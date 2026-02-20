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
    private val key: SecretKey,
    private val scope: CoroutineScope,
) {
    companion object {
        private const val TAG = "SyncService"
    }

    private val json = Json { ignoreUnknownKeys = true; encodeDefaults = true }

    var onSyncStatusChanged: ((String?) -> Unit)? = null
    var onConflictDetected: ((ConflictInfo) -> Unit)? = null
    var onSyncComplete: ((Int) -> Unit)? = null

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
            val notesToSync = noteRepo.listNeedingSync()
            if (notesToSync.isEmpty()) {
                val deletions = syncRepo.listDeletions()
                val savedSearches = savedSearchRepo.listNeedingSync()
                if (deletions.isEmpty() && savedSearches.isEmpty()) return null
            }

            // Build sync notes
            val syncNotes = notesToSync.map { record ->
                val attachmentRefs: List<AttachmentRef> = try {
                    json.decodeFromString(record.attachments)
                } catch (_: Exception) { emptyList() }

                val hashChain: List<String>? = record.hashChain?.let {
                    try { json.decodeFromString(it) } catch (_: Exception) { null }
                }

                // Encrypt tags individually for sync format
                val encTags = try {
                    val enc = CryptoService.parseEncryptedJSON(record.tags)
                    val plainText = CryptoService.decryptText(enc, key)
                    val plainTags: List<String> = if (plainText.startsWith("[")) {
                        json.decodeFromString(plainText)
                    } else {
                        plainText.split(",").map { it.trim() }.filter { it.isNotEmpty() }
                    }
                    plainTags.map { tag ->
                        val tagJson = json.encodeToString(tag)
                        val encrypted = CryptoService.encryptText(tagJson, key)
                        CryptoService.serializeEncryptedJSON(encrypted)
                    }
                } catch (_: Exception) {
                    listOf(record.tags)
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
                    val blob = attachmentRepo.getBlob(ref.data) ?: continue
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

            // Process accepted
            val now = DateUtils.nowISO8601()
            for (accepted in response.accepted) {
                noteRepo.markSynced(accepted.id, accepted.syncedAt ?: now, accepted.serverVersion)
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
        onSyncStatusChanged?.invoke("Pulling\u2026")
        try {
            val syncMeta = syncRepo.getSyncMetadata()
            val knownPairs = noteRepo.listIdsAndAttachmentBlobIds()
            val knownNoteIds = knownPairs.map { it.first }.toMutableList()
            val knownBlobIds = knownPairs.flatMap { it.second }.toMutableList()

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

                // Process notes
                for (syncNote in response.notes) {
                    val existing = noteRepo.getRaw(syncNote.id)
                    if (existing == null) {
                        totalNewCount++
                    }

                    // Convert sync tags back to single encrypted blob
                    val tagsBlob = if (syncNote.tags.size == 1 && syncNote.tags[0].contains("ciphertext")) {
                        syncNote.tags[0]
                    } else {
                        // Re-encrypt tags as a single blob
                        val plainTags = syncNote.tags.mapNotNull { tagStr ->
                            try {
                                val enc = CryptoService.parseEncryptedJSON(tagStr)
                                val plain = CryptoService.decryptText(enc, key)
                                json.decodeFromString<String>(plain)
                            } catch (_: Exception) { null }
                        }
                        val encTags = CryptoService.encryptStringArray(plainTags, key)
                        CryptoService.serializeEncryptedJSON(encTags)
                    }

                    val record = NoteRecord(
                        id = syncNote.id,
                        createdAt = syncNote.createdAt,
                        modifiedAt = syncNote.modifiedAt,
                        syncedAt = response.syncedAt,
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
                    noteRepo.insertOrReplace(record)
                    knownNoteIds.add(syncNote.id)
                }

                // Process attachments
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

                // Process versions
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

                // Process deletions
                response.deletions?.forEach { deletion ->
                    noteRepo.hardDelete(deletion.id)
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

    // MARK: - Full Sync Cycle

    suspend fun fullSync(): Int {
        val pushResponse = push()
        val newCount = pull()

        onSyncStatusChanged?.invoke("Finishing\u2026")

        // Handle rejections (conflicts)
        pushResponse?.rejected?.forEach { rejected ->
            handleRejection(rejected)
        }

        onSyncStatusChanged?.invoke(null)
        onSyncComplete?.invoke(newCount)
        return newCount
    }

    private suspend fun handleRejection(rejected: SyncRejected) {
        try {
            val localNote = noteRepo.get(rejected.id, key) ?: return

            // Decrypt server content for comparison
            val serverContent = try {
                val enc = CryptoService.parseEncryptedJSON(rejected.serverContent)
                CryptoService.decryptText(enc, key)
            } catch (_: Exception) { rejected.serverContent }

            val serverTags = rejected.serverTags.mapNotNull { tagStr ->
                try {
                    val enc = CryptoService.parseEncryptedJSON(tagStr)
                    val plain = CryptoService.decryptText(enc, key)
                    json.decodeFromString<String>(plain)
                } catch (_: Exception) { null }
            }

            val conflict = ConflictInfo(
                id = rejected.id,
                localContent = localNote.content,
                localTags = localNote.tags,
                localModifiedAt = localNote.modifiedAt.toString(),
                serverContent = serverContent,
                serverTags = serverTags,
                serverModifiedAt = rejected.serverModifiedAt,
                serverEncryptedContent = rejected.serverContent,
                serverEncryptedTags = rejected.serverTags,
                serverVersion = rejected.serverVersion,
                serverAttachments = rejected.serverAttachments,
                serverPinned = rejected.serverPinned,
                serverSyntaxLanguage = rejected.serverSyntaxLanguage,
                serverWordWrap = rejected.serverWordWrap,
                serverShowPreview = rejected.serverShowPreview,
                serverContentHash = rejected.serverContentHash,
                serverParentHash = rejected.serverParentHash,
                serverHashChain = rejected.serverHashChain,
            )

            onConflictDetected?.invoke(conflict)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to handle rejection for ${rejected.id}: ${e.message}")
        }
    }

    /**
     * Resolve a sync conflict using the specified strategy.
     * Ported from iOS SyncService.resolveConflict().
     */
    suspend fun resolveConflict(conflict: ConflictInfo, strategy: ConflictResolutionStrategy) {
        val localRecord = noteRepo.getRaw(conflict.id) ?: return

        when (strategy) {
            ConflictResolutionStrategy.KEEP_LOCAL -> {
                // Rebase local hash chain onto server's for fast-forward push
                val localChain: List<String> = localRecord.hashChain?.let {
                    try { json.decodeFromString(it) } catch (_: Exception) { emptyList() }
                } ?: emptyList()
                val serverChain = conflict.serverHashChain ?: emptyList()

                // Merge chains: server chain + local chain, deduplicated, max 50
                val merged = (serverChain + localChain).distinct().takeLast(50)

                val updated = localRecord.copy(
                    version = conflict.serverVersion + 1,
                    parentHash = conflict.serverContentHash,
                    hashChain = json.encodeToString(merged),
                    needsSync = true,
                    modifiedAt = DateUtils.nowISO8601(),
                )
                noteRepo.updateRaw(updated)
            }

            ConflictResolutionStrategy.KEEP_SERVER -> {
                // Snapshot current state before overwriting
                versionRepo.insertOrReplace(
                    NoteVersionRecord.from(localRecord, "pre-conflict-resolution")
                )

                // Convert server sync tags to storage format (single encrypted blob)
                val tagsBlob = syncTagsToStorageTags(conflict.serverEncryptedTags)

                val updated = localRecord.copy(
                    content = conflict.serverEncryptedContent,
                    tags = tagsBlob,
                    attachments = json.encodeToString(conflict.serverAttachments),
                    pinned = conflict.serverPinned,
                    syntaxLanguage = conflict.serverSyntaxLanguage ?: localRecord.syntaxLanguage,
                    wordWrap = conflict.serverWordWrap ?: localRecord.wordWrap,
                    showPreview = conflict.serverShowPreview ?: localRecord.showPreview,
                    version = conflict.serverVersion,
                    contentHash = conflict.serverContentHash,
                    parentHash = conflict.serverParentHash,
                    hashChain = conflict.serverHashChain?.let { json.encodeToString(it) },
                    needsSync = false,
                    syncedAt = DateUtils.nowISO8601(),
                    modifiedAt = conflict.serverModifiedAt,
                )
                noteRepo.updateRaw(updated)
            }

            ConflictResolutionStrategy.KEEP_BOTH -> {
                // Accept server version for the existing note
                val tagsBlob = syncTagsToStorageTags(conflict.serverEncryptedTags)

                val serverRecord = localRecord.copy(
                    content = conflict.serverEncryptedContent,
                    tags = tagsBlob,
                    attachments = json.encodeToString(conflict.serverAttachments),
                    pinned = conflict.serverPinned,
                    syntaxLanguage = conflict.serverSyntaxLanguage ?: localRecord.syntaxLanguage,
                    wordWrap = conflict.serverWordWrap ?: localRecord.wordWrap,
                    showPreview = conflict.serverShowPreview ?: localRecord.showPreview,
                    version = conflict.serverVersion,
                    contentHash = conflict.serverContentHash,
                    parentHash = conflict.serverParentHash,
                    hashChain = conflict.serverHashChain?.let { json.encodeToString(it) },
                    needsSync = false,
                    syncedAt = DateUtils.nowISO8601(),
                    modifiedAt = conflict.serverModifiedAt,
                )
                noteRepo.updateRaw(serverRecord)

                // Create a new note with the local content
                noteRepo.create(
                    content = conflict.localContent,
                    tags = conflict.localTags,
                    key = key,
                )
            }
        }

        Log.d(TAG, "Conflict resolved for ${conflict.id} with strategy $strategy")
    }

    /**
     * Convert sync-format tags (individually encrypted) to storage format (single encrypted blob).
     */
    private fun syncTagsToStorageTags(syncTags: List<String>): String {
        return try {
            if (syncTags.size == 1 && syncTags[0].contains("ciphertext")) {
                // Already in storage format
                syncTags[0]
            } else {
                // Decrypt individual sync tags and re-encrypt as single blob
                val plainTags = syncTags.mapNotNull { tagStr ->
                    try {
                        val enc = CryptoService.parseEncryptedJSON(tagStr)
                        val plain = CryptoService.decryptText(enc, key)
                        json.decodeFromString<String>(plain)
                    } catch (_: Exception) { null }
                }
                val encTags = CryptoService.encryptStringArray(plainTags, key)
                CryptoService.serializeEncryptedJSON(encTags)
            }
        } catch (_: Exception) {
            syncTags.firstOrNull() ?: "[]"
        }
    }
}
