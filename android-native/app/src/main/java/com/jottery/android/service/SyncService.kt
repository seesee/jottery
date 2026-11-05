package com.jottery.android.service

import android.util.Log
import com.jottery.android.crypto.CryptoService
import com.jottery.android.database.repository.AttachmentRepository
import com.jottery.android.database.repository.NoteRepository
import com.jottery.android.database.repository.SavedSearchRepository
import com.jottery.android.database.repository.SyncRepository
import com.jottery.android.database.repository.VersionRepository
import com.jottery.android.model.*
import com.jottery.android.network.SSEClient
import com.jottery.android.network.SyncClient
import com.jottery.android.util.DateUtils
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonPrimitive
import javax.crypto.SecretKey

/**
 * Sync orchestrator — push local changes, pull remote changes, handle conflicts.
 */
class SyncService(
    private val syncClient: SyncClient,
    private val sseClient: SSEClient,
    private val noteRepo: NoteRepository,
    private val versionRepo: VersionRepository,
    private val attachmentRepo: AttachmentRepository,
    private val syncRepo: SyncRepository,
    private val savedSearchRepo: SavedSearchRepository,
    /** Read on every use, so a key swap or a lock takes effect at once. */
    private val keyProvider: () -> SecretKey?,
    private val scope: CoroutineScope,
    /** Runs a block in one database transaction. */
    private val transaction: suspend (suspend () -> Unit) -> Unit = { it() },
) {
    companion object {
        private const val TAG = "SyncService"

        /**
         * Build the attachments-only repair payload for server attachment
         * warnings: every warned blob the lookup can supply, deduplicated by
         * id. Pure logic split out for testability.
         */
        fun collectRepairAttachments(
            warnings: List<AttachmentWarning>,
            blobLookup: (String) -> ByteArray?,
        ): List<SyncAttachment> {
            val repairs = mutableListOf<SyncAttachment>()
            val seen = mutableSetOf<String>()
            for (warning in warnings) {
                for (attachmentId in warning.attachmentIds) {
                    if (!seen.add(attachmentId)) continue
                    val blob = blobLookup(attachmentId) ?: continue
                    repairs.add(SyncAttachment(
                        id = attachmentId,
                        data = java.util.Base64.getEncoder().encodeToString(blob),
                    ))
                }
            }
            return repairs
        }
    }

    private val json = Json { ignoreUnknownKeys = true; encodeDefaults = true }

    var onSyncStatusChanged: ((String?) -> Unit)? = null
    var onConflictDetected: ((ConflictInfo) -> Unit)? = null
    var onSyncComplete: ((Int) -> Unit)? = null

    private fun requireKey(): SecretKey =
        keyProvider() ?: throw IllegalStateException("Vault is locked")

    // MARK: - SSE

    suspend fun startSSE() {
        try {
            val tokenResponse = withContext(Dispatchers.IO) {
                syncClient.getSSEToken()
            }
            sseClient.start(tokenResponse.token)
            Log.d(TAG, "SSE started with token")
        } catch (e: Exception) {
            Log.w(TAG, "Failed to start SSE: ${e.message}")
        }
    }

    fun stopSSE() {
        sseClient.stop()
    }

    // MARK: - Push

    suspend fun push(): SyncPushResponse? {
        onSyncStatusChanged?.invoke("Pushing\u2026")
        try {
            val key = requireKey()
            // Notes waiting on a conflict decision would only be rejected again
            val inConflict = syncRepo.listStoredConflicts().map { it.id }.toSet()
            val notesToSync = noteRepo.listNeedingSync().filter { it.id !in inConflict }
            if (notesToSync.isEmpty()) {
                val deletions = syncRepo.listDeletions()
                val savedSearches = savedSearchRepo.listNeedingSync()
                if (deletions.isEmpty() && savedSearches.isEmpty()) return null
            }

            // Guards the accepted notes below: markSynced only clears the flag if
            // modified_at still matches the snapshot that was actually pushed.
            val pushedModifiedAt = notesToSync.associate { it.id to it.modifiedAt }

            // Build sync notes
            val syncNotes = notesToSync.mapNotNull { record ->
                val attachmentRefs: List<AttachmentRef> = try {
                    json.decodeFromString(record.attachments)
                } catch (_: Exception) { emptyList() }

                val hashChain: List<String>? = record.hashChain?.let {
                    try { json.decodeFromString(it) } catch (_: Exception) { null }
                }

                // Encrypt tags individually for sync format. Tags that can't be
                // read must not go out as they are, so the note stays queued.
                val encTags = try {
                    storageTagsToSyncTags(record.tags, key)
                } catch (e: Exception) {
                    Log.w(TAG, "Skipping push of note ${record.id}: tags unreadable")
                    return@mapNotNull null
                }

                SyncNote(
                    id = record.id,
                    createdAt = record.createdAt,
                    modifiedAt = record.modifiedAt,
                    content = record.content,
                    tags = encTags,
                    attachments = attachmentRefs,
                    pinned = record.pinned,
                    archived = record.archived,
                    archivedAt = record.archivedAt,
                    locked = record.locked,
                    lockedAt = record.lockedAt,
                    deleted = record.deleted,
                    deletedAt = record.deletedAt,
                    version = record.version,
                    wordWrap = record.wordWrap,
                    syntaxLanguage = record.syntaxLanguage,
                    showPreview = record.showPreview,
                    color = record.color,
                    contentHash = record.contentHash,
                    parentHash = record.parentHash,
                    hashChain = hashChain,
                )
            }

            // Build sync versions
            val versionsToSync = versionRepo.listNeedingSync()
            val syncVersions = versionsToSync.map { ver ->
                val attachmentRefs: List<AttachmentRef> = try {
                    json.decodeFromString(ver.attachments)
                } catch (_: Exception) { emptyList() }

                SyncNoteVersion(
                    versionKey = ver.versionKey,
                    noteId = ver.noteId,
                    version = ver.version,
                    createdAt = ver.createdAt,
                    syncedAt = ver.syncedAt,
                    content = ver.content,
                    tags = listOf(ver.tags), // Tags stored as single encrypted blob
                    attachments = attachmentRefs,
                    syntaxLanguage = ver.syntaxLanguage,
                    wordWrap = ver.wordWrap,
                    showPreview = ver.showPreview,
                    color = ver.color,
                    reason = ver.reason,
                    contentHash = ver.contentHash,
                    parentHash = ver.parentHash,
                )
            }

            // Build sync attachments
            val syncAttachments = mutableListOf<SyncAttachment>()
            for (note in notesToSync) {
                val refs: List<AttachmentRef> = try {
                    json.decodeFromString(note.attachments)
                } catch (_: Exception) { continue }
                for (ref in refs) {
                    val blob = attachmentRepo.getBlob(ref.data)
                    if (blob == null) {
                        // Never skip silently: the note still pushes without the
                        // blob and the server warns back if it has no copy either
                        Log.w(TAG, "Attachment ${ref.id} (note ${note.id}) has no local blob — pushing note without it")
                        continue
                    }
                    syncAttachments.add(SyncAttachment(
                        id = ref.data,
                        data = android.util.Base64.encodeToString(blob, android.util.Base64.NO_WRAP),
                    ))
                }
            }

            // Build deletions
            val syncDeletions = syncRepo.listDeletions().map {
                SyncDeletion(id = it.id, deletedAt = it.deletedAt)
            }

            val request = SyncPushRequest(
                notes = syncNotes,
                attachments = syncAttachments,
                versions = syncVersions,
                deletions = syncDeletions,
            )

            val response = syncClient.push(request)

            // React to server attachment warnings: re-upload blobs this device
            // holds (one attachments-only push, no in-run retry), log the rest
            val warnings = response.attachmentWarnings.orEmpty()
            if (warnings.isNotEmpty()) {
                // getBlob is a suspend function, so gather held blobs first and
                // hand the pure helper a plain lookup
                val heldBlobs = mutableMapOf<String, ByteArray>()
                for (warning in warnings) {
                    for (attachmentId in warning.attachmentIds) {
                        if (attachmentId !in heldBlobs) {
                            attachmentRepo.getBlob(attachmentId)?.let { heldBlobs[attachmentId] = it }
                        }
                    }
                }
                val repairs = collectRepairAttachments(warnings) { id -> heldBlobs[id] }
                val healableIds = repairs.map { it.id }.toSet()
                for (warning in warnings) {
                    for (attachmentId in warning.attachmentIds) {
                        if (attachmentId !in healableIds) {
                            Log.w(TAG, "Server has no data for attachment $attachmentId (note ${warning.noteId}) and neither does this device")
                        }
                    }
                }
                if (repairs.isNotEmpty()) {
                    Log.w(TAG, "Re-uploading ${repairs.size} attachment(s) the server is missing")
                    try {
                        syncClient.push(SyncPushRequest(notes = emptyList(), attachments = repairs, versions = emptyList(), deletions = emptyList()))
                    } catch (e: Exception) {
                        Log.e(TAG, "Attachment repair push failed: ${e.message}")
                    }
                }
            }

            // Rejections first: the conflict is built from the local row while
            // that row still holds the edit, before pull can touch anything.
            for (rejected in response.rejected) {
                handleRejection(rejected)
            }

            // Process accepted
            val now = DateUtils.nowISO8601()
            for (accepted in response.accepted) {
                val modifiedAt = pushedModifiedAt[accepted.id] ?: continue
                val cleared = noteRepo.markSynced(
                    id = accepted.id,
                    syncedAt = accepted.syncedAt ?: now,
                    serverVersion = accepted.serverVersion,
                    pushedModifiedAt = modifiedAt,
                )
                if (cleared) {
                    noteRepo.getRaw(accepted.id)?.let {
                        versionRepo.insertOrReplace(NoteVersionRecord.from(it, "sync"))
                    }
                }
            }

            // Clear processed deletions
            if (syncDeletions.isNotEmpty()) {
                syncRepo.clearDeletions()
            }

            syncRepo.updateLastPushAt(now)

            return response
        } catch (e: Exception) {
            Log.e(TAG, "Push failed: ${e.message}")
            throw e
        }
    }

    // MARK: - Pull

    suspend fun pull(): Int {
        onSyncStatusChanged?.invoke("Pulling…")
        try {
            val key = requireKey()
            val syncMeta = syncRepo.getSyncMetadata()
            val knownPairs = noteRepo.listIdsAndAttachmentBlobIds()
            val knownNoteIds = knownPairs.map { it.first }.toMutableList()
            val knownBlobIds = knownPairs.flatMap { it.second }.toMutableList()
            val conflictsById = syncRepo.listStoredConflicts().associateBy { it.id }.toMutableMap()

            var offset = 0
            var hasMore = true
            var totalNewCount = 0

            while (hasMore) {
                val request = SyncPullRequest(
                    lastSyncAt = syncMeta?.lastSyncAt,
                    knownNoteIds = knownNoteIds,
                    knownAttachmentIds = knownBlobIds,
                    limit = 100,
                    offset = offset,
                )

                val response = syncClient.pull(request)

                Log.d(TAG, "Pull page: offset=$offset, notes=${response.notes.size}, " +
                    "attachments=${response.attachments.size}, hasMore=${response.hasMore}")

                val raised = mutableListOf<StoredConflict>()

                // One transaction per page: a note never lands without its blobs
                transaction {
                    for (syncNote in response.notes) {
                        if (processNote(syncNote, response.syncedAt, key, conflictsById, raised)) {
                            totalNewCount++
                        }
                        knownNoteIds.add(syncNote.id)
                    }

                    for (syncAtt in response.attachments) {
                        val data = android.util.Base64.decode(syncAtt.data, android.util.Base64.NO_WRAP)
                        attachmentRepo.insertOrReplace(
                            com.jottery.android.model.AttachmentRecord(
                                id = syncAtt.id,
                                filename = "",
                                mimeType = "",
                                size = data.size,
                                data = data,
                            )
                        )
                        knownBlobIds.add(syncAtt.id)
                    }

                    for (syncVer in response.versions) {
                        val ver = NoteVersionRecord(
                            versionKey = syncVer.versionKey,
                            noteId = syncVer.noteId,
                            version = syncVer.version,
                            createdAt = syncVer.createdAt,
                            syncedAt = syncVer.syncedAt,
                            content = syncVer.content,
                            tags = syncVer.tags.firstOrNull() ?: "[]",
                            attachments = json.encodeToString(syncVer.attachments),
                            syntaxLanguage = syncVer.syntaxLanguage,
                            wordWrap = syncVer.wordWrap,
                            showPreview = syncVer.showPreview,
                            color = syncVer.color,
                            reason = syncVer.reason,
                            contentHash = syncVer.contentHash,
                            parentHash = syncVer.parentHash,
                        )
                        versionRepo.insertOrReplace(ver)
                    }

                    response.deletions?.forEach { deletion ->
                        // A note with unsynced edits outranks the remote deletion
                        if (!noteRepo.hardDeleteIfSynced(deletion.id)) {
                            Log.d(TAG, "Kept note ${deletion.id}: local edits outrank the remote deletion")
                        }
                    }
                }

                // Only announce conflicts once the page has committed
                for (stored in raised) {
                    onConflictDetected?.invoke(describe(stored, key))
                }

                hasMore = response.hasMore ?: false
                offset += response.notes.size

                // Safety: avoid infinite loop if server returns hasMore but no notes
                if (hasMore && response.notes.isEmpty()) {
                    Log.w(TAG, "Server says hasMore but returned 0 notes — breaking")
                    break
                }
            }

            val now = DateUtils.nowISO8601()
            syncRepo.updateLastPullAt(now)
            syncRepo.updateLastSyncAt(now)

            Log.d(TAG, "Pull complete: $totalNewCount new notes across ${offset} total")
            return totalNewCount
        } catch (e: Exception) {
            Log.e(TAG, "Pull failed: ${e.message}")
            throw e
        }
    }

    /**
     * Apply one pulled note. Returns true if it is new to this device.
     *
     * Nothing here may destroy a local edit: see [PullDecision].
     */
    private suspend fun processNote(
        syncNote: SyncNote,
        syncedAt: String?,
        key: SecretKey,
        conflictsById: MutableMap<String, StoredConflict>,
        raised: MutableList<StoredConflict>,
    ): Boolean {
        // A note awaiting a decision stays as it is. The stored server side is
        // kept current, so choosing it can't adopt a version already overtaken.
        conflictsById[syncNote.id]?.let { existing ->
            val updated = conflictFrom(syncNote, existing)
            syncRepo.saveConflict(updated)
            conflictsById[syncNote.id] = updated
            return false
        }

        val local = noteRepo.getRaw(syncNote.id)
        if (local == null) {
            noteRepo.insertOrReplace(buildRecord(syncNote, syncedAt, key) ?: return false)
            return true
        }

        val action = PullDecision.decide(
            local = PullDecision.Side(
                contentHash = local.contentHash,
                parentHash = local.parentHash,
                hashChain = decodeChain(local.hashChain),
                modifiedAt = local.modifiedAt,
            ),
            remote = PullDecision.Side(
                contentHash = syncNote.contentHash,
                parentHash = syncNote.parentHash,
                hashChain = syncNote.hashChain.orEmpty(),
                modifiedAt = syncNote.modifiedAt,
            ),
            localNeedsSync = local.needsSync,
            flagsDiffer = flagsDiffer(local, syncNote),
        )

        when (action) {
            PullAction.SKIP -> Unit
            PullAction.FAST_FORWARD -> {
                noteRepo.insertOrReplace(buildRecord(syncNote, syncedAt, key) ?: return false)
            }
            PullAction.FAST_FORWARD_KEEP_FLAGS -> {
                val remote = buildRecord(syncNote, syncedAt, key) ?: return false
                noteRepo.insertOrReplace(
                    remote.copy(
                        pinned = local.pinned,
                        archived = local.archived,
                        archivedAt = local.archivedAt,
                        locked = local.locked,
                        lockedAt = local.lockedAt,
                        deleted = local.deleted,
                        deletedAt = local.deletedAt,
                        needsSync = true,
                    )
                )
            }
            PullAction.REPLACE_WITH_SNAPSHOT -> {
                val remote = buildRecord(syncNote, syncedAt, key) ?: return false
                versionRepo.insertOrReplace(NoteVersionRecord.safetySnapshot(local, "pre-sync-overwrite"))
                noteRepo.insertOrReplace(remote)
            }
            PullAction.CONFLICT -> {
                val stored = conflictFrom(syncNote, null)
                syncRepo.saveConflict(stored)
                conflictsById[syncNote.id] = stored
                raised.add(stored)
            }
        }
        return false
    }

    private fun flagsDiffer(local: NoteRecord, remote: SyncNote): Boolean =
        local.pinned != remote.pinned ||
            local.archived != remote.archived ||
            local.locked != (remote.locked ?: false) ||
            local.deleted != remote.deleted ||
            local.color != remote.color ||
            local.wordWrap != (remote.wordWrap ?: local.wordWrap) ||
            local.syntaxLanguage != (remote.syntaxLanguage ?: local.syntaxLanguage) ||
            local.showPreview != (remote.showPreview ?: local.showPreview)

    /**
     * Build a local row from a pulled note, or null to quarantine it: a note that
     * can't be read under this key must not replace anything or be stored with
     * its tags missing. Force Full Sync fetches it again.
     */
    private fun buildRecord(syncNote: SyncNote, syncedAt: String?, key: SecretKey): NoteRecord? {
        val tagsBlob = try {
            CryptoService.decrypt(CryptoService.parseEncryptedJSON(syncNote.content), key)
            syncTagsToStorageTags(syncNote.tags, key)
        } catch (e: Exception) {
            Log.w(TAG, "Skipping pulled note ${syncNote.id}: it can't be decrypted with this key")
            return null
        }

        return NoteRecord(
            id = syncNote.id,
            createdAt = syncNote.createdAt,
            modifiedAt = syncNote.modifiedAt,
            syncedAt = syncedAt,
            content = syncNote.content,
            tags = tagsBlob,
            attachments = json.encodeToString(syncNote.attachments),
            pinned = syncNote.pinned,
            archived = syncNote.archived,
            archivedAt = syncNote.archivedAt,
            locked = syncNote.locked ?: false,
            lockedAt = syncNote.lockedAt,
            deleted = syncNote.deleted,
            deletedAt = syncNote.deletedAt,
            version = syncNote.version,
            wordWrap = syncNote.wordWrap ?: true,
            syntaxLanguage = syncNote.syntaxLanguage ?: "markdown",
            showPreview = syncNote.showPreview ?: false,
            color = syncNote.color,
            contentHash = syncNote.contentHash,
            parentHash = syncNote.parentHash,
            hashChain = syncNote.hashChain?.let { json.encodeToString(it) },
            needsSync = false,
        )
    }

    private fun decodeChain(chain: String?): List<String> = chain?.let {
        try { json.decodeFromString<List<String>>(it) } catch (_: Exception) { emptyList() }
    } ?: emptyList()

    // MARK: - Full Sync Cycle

    suspend fun fullSync(): Int {
        push()
        val newCount = pull()

        onSyncStatusChanged?.invoke(null)
        onSyncComplete?.invoke(newCount)
        return newCount
    }

    // MARK: - Conflicts

    private suspend fun handleRejection(rejected: SyncRejected) {
        try {
            val stored = StoredConflict(
                id = rejected.id,
                serverModifiedAt = rejected.serverModifiedAt,
                serverEncryptedContent = rejected.serverContent,
                serverEncryptedTags = rejected.serverTags,
                serverVersion = rejected.serverVersion,
                serverAttachments = rejected.serverAttachments,
                serverPinned = rejected.serverPinned,
                serverArchived = rejected.serverArchived,
                serverArchivedAt = rejected.serverArchivedAt,
                serverLocked = rejected.serverLocked,
                serverLockedAt = rejected.serverLockedAt,
                serverColor = rejected.serverColor,
                serverSyntaxLanguage = rejected.serverSyntaxLanguage,
                serverWordWrap = rejected.serverWordWrap,
                serverShowPreview = rejected.serverShowPreview,
                serverContentHash = rejected.serverContentHash,
                serverParentHash = rejected.serverParentHash,
                serverHashChain = rejected.serverHashChain,
                ancestorContent = rejected.ancestorContent,
                ancestorTags = rejected.ancestorTags,
            )
            syncRepo.saveConflict(stored)
            onConflictDetected?.invoke(describe(stored, requireKey()))
        } catch (e: Exception) {
            Log.e(TAG, "Failed to handle rejection for ${rejected.id}: ${e.message}")
        }
    }

    private fun conflictFrom(note: SyncNote, previous: StoredConflict?) = StoredConflict(
        id = note.id,
        serverModifiedAt = note.modifiedAt,
        serverEncryptedContent = note.content,
        serverEncryptedTags = note.tags,
        serverVersion = note.version,
        serverAttachments = note.attachments,
        serverPinned = note.pinned,
        serverArchived = note.archived,
        serverArchivedAt = note.archivedAt,
        serverLocked = note.locked ?: false,
        serverLockedAt = note.lockedAt,
        serverDeleted = note.deleted,
        serverDeletedAt = note.deletedAt,
        serverColor = note.color,
        serverSyntaxLanguage = note.syntaxLanguage,
        serverWordWrap = note.wordWrap,
        serverShowPreview = note.showPreview,
        serverContentHash = note.contentHash,
        serverParentHash = note.parentHash,
        serverHashChain = note.hashChain,
        ancestorContent = previous?.ancestorContent,
        ancestorTags = previous?.ancestorTags,
    )

    /** Decrypt both sides of a conflict for display. */
    private suspend fun describe(stored: StoredConflict, key: SecretKey): ConflictInfo {
        val local = try {
            noteRepo.get(stored.id, key)
        } catch (e: Exception) {
            Log.w(TAG, "Local copy of conflicted note ${stored.id} is unreadable")
            null
        }

        val serverContent = try {
            CryptoService.decryptText(CryptoService.parseEncryptedJSON(stored.serverEncryptedContent), key)
        } catch (_: Exception) {
            null // Never fall back to the raw value
        }
        val serverTags = try {
            stored.serverEncryptedTags.flatMap { decodeSyncTag(it, key) }
        } catch (_: Exception) {
            emptyList()
        }
        val ancestor = stored.ancestorContent?.let {
            try {
                CryptoService.decryptText(CryptoService.parseEncryptedJSON(it), key)
            } catch (_: Exception) {
                null
            }
        }

        return ConflictInfo(
            stored = stored,
            localContent = local?.content ?: "",
            localTags = local?.tags ?: emptyList(),
            localModifiedAt = local?.modifiedAt?.toString() ?: "",
            serverContent = serverContent,
            serverTags = serverTags,
            ancestorContent = ancestor,
        )
    }

    /** Every conflict awaiting a decision, decrypted for display. */
    suspend fun loadConflicts(): List<ConflictInfo> {
        val key = requireKey()
        return syncRepo.listStoredConflicts().map { describe(it, key) }
    }

    /**
     * Resolve a sync conflict using the specified strategy. Ported from iOS
     * SyncService.resolveConflict(). Throws on failure, leaving the conflict in
     * place so nothing is lost and the user can try again.
     */
    suspend fun resolveConflict(conflict: ConflictInfo, strategy: ConflictResolutionStrategy) {
        val key = requireKey()
        val stored = conflict.stored
        val localRecord = noteRepo.getRaw(conflict.id)
            ?: throw IllegalStateException("The note no longer exists")
        if (strategy != ConflictResolutionStrategy.KEEP_LOCAL && conflict.serverUnreadable) {
            throw IllegalStateException("The server's version can't be decrypted on this device")
        }

        // Decrypts every server tag, so a failure stops here with nothing written
        val serverTagsBlob =
            if (strategy == ConflictResolutionStrategy.KEEP_LOCAL) null
            else syncTagsToStorageTags(stored.serverEncryptedTags, key)

        transaction {
            when (strategy) {
                ConflictResolutionStrategy.KEEP_LOCAL -> {
                    // Rebase onto the server's head so the next push fast-forwards:
                    // local hash, server hash, then both histories, newest first.
                    val merged = (
                        listOfNotNull(localRecord.contentHash, stored.serverContentHash) +
                            decodeChain(localRecord.hashChain) +
                            stored.serverHashChain.orEmpty()
                        ).distinct().take(CryptoService.MAX_HASH_CHAIN_LENGTH)

                    noteRepo.updateRaw(
                        localRecord.copy(
                            version = stored.serverVersion + 1,
                            parentHash = stored.serverContentHash,
                            hashChain = json.encodeToString(merged),
                            needsSync = true,
                            modifiedAt = DateUtils.nowISO8601(),
                        )
                    )
                }

                ConflictResolutionStrategy.KEEP_SERVER -> {
                    versionRepo.insertOrReplace(
                        NoteVersionRecord.safetySnapshot(localRecord, "pre-conflict-resolution")
                    )
                    noteRepo.updateRaw(adoptServer(localRecord, stored, serverTagsBlob!!))
                }

                ConflictResolutionStrategy.KEEP_BOTH -> {
                    noteRepo.updateRaw(adoptServer(localRecord, stored, serverTagsBlob!!))

                    // The local text moves to a new note with everything that
                    // went with it. Blobs are copied under new ids, because
                    // removing an attachment deletes its blob by id.
                    val copy = noteRepo.create(
                        content = conflict.localContent,
                        tags = conflict.localTags,
                        key = key,
                    )
                    val refs = decodeRefs(localRecord.attachments).map { copyAttachment(it) }
                    val copyRecord = noteRepo.getRaw(copy.id)!!
                    noteRepo.updateRaw(
                        copyRecord.copy(
                            attachments = json.encodeToString(refs),
                            pinned = localRecord.pinned,
                            syntaxLanguage = localRecord.syntaxLanguage,
                            wordWrap = localRecord.wordWrap,
                            showPreview = localRecord.showPreview,
                            color = localRecord.color,
                        )
                    )
                }
            }
            syncRepo.clearConflict(conflict.id)
        }

        Log.d(TAG, "Conflict resolved for ${conflict.id} with strategy $strategy")
    }

    private fun adoptServer(local: NoteRecord, stored: StoredConflict, tagsBlob: String) = local.copy(
        content = stored.serverEncryptedContent,
        tags = tagsBlob,
        attachments = json.encodeToString(stored.serverAttachments),
        pinned = stored.serverPinned,
        archived = stored.serverArchived,
        archivedAt = stored.serverArchivedAt,
        locked = stored.serverLocked,
        lockedAt = stored.serverLockedAt,
        deleted = stored.serverDeleted,
        deletedAt = stored.serverDeletedAt,
        color = stored.serverColor,
        syntaxLanguage = stored.serverSyntaxLanguage ?: local.syntaxLanguage,
        wordWrap = stored.serverWordWrap ?: local.wordWrap,
        showPreview = stored.serverShowPreview ?: local.showPreview,
        version = stored.serverVersion,
        contentHash = stored.serverContentHash,
        parentHash = stored.serverParentHash,
        hashChain = stored.serverHashChain?.let { json.encodeToString(it) },
        needsSync = false,
        syncedAt = DateUtils.nowISO8601(),
        modifiedAt = stored.serverModifiedAt,
    )

    private fun decodeRefs(attachments: String): List<AttachmentRef> = try {
        json.decodeFromString(attachments)
    } catch (_: Exception) {
        emptyList()
    }

    private suspend fun copyAttachment(ref: AttachmentRef): AttachmentRef {
        val newId = CryptoService.generateUUID()
        val blob = attachmentRepo.getBlob(ref.data)
        if (blob != null) {
            attachmentRepo.insertOrReplace(
                com.jottery.android.model.AttachmentRecord(
                    id = newId,
                    filename = "",
                    mimeType = ref.mimeType,
                    size = ref.size,
                    data = blob,
                )
            )
        }
        return ref.copy(id = newId, data = newId)
    }

    // MARK: - Tag Format

    /**
     * Convert sync-format tags (individually encrypted) to storage format (one
     * encrypted blob). Throws if any tag can't be read, because storing a shorter
     * list would erase the rest on every device at the next edit.
     */
    private fun syncTagsToStorageTags(syncTags: List<String>, key: SecretKey): String {
        val plainTags = syncTags.flatMap { decodeSyncTag(it, key) }
        return CryptoService.serializeEncryptedJSON(CryptoService.encryptStringArray(plainTags, key))
    }

    /** Convert storage format to sync format: each tag JSON-encoded, then encrypted. */
    private fun storageTagsToSyncTags(storageTags: String, key: SecretKey): List<String> {
        if (storageTags == "[]") return emptyList() // Written by older builds
        return decodeSyncTag(storageTags, key).map { tag ->
            CryptoService.serializeEncryptedJSON(CryptoService.encryptText(json.encodeToString(tag), key))
        }
    }

    /**
     * One encrypted value to its tags. Clients have written three shapes: a JSON
     * string, a JSON array holding every tag, and plain text.
     */
    private fun decodeSyncTag(encrypted: String, key: SecretKey): List<String> {
        val text = CryptoService.decryptText(CryptoService.parseEncryptedJSON(encrypted), key)
        val element = try {
            json.parseToJsonElement(text)
        } catch (_: Exception) {
            null
        }
        return when {
            element is JsonArray -> element.map { (it as JsonPrimitive).content }
            element is JsonPrimitive && element.isString -> listOf(element.content)
            text.isEmpty() -> emptyList()
            else -> listOf(text)
        }
    }
}
