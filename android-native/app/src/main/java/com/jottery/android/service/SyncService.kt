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
import com.jottery.android.network.SyncClientException
import com.jottery.android.util.DateUtils
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonPrimitive
import java.time.Duration
import java.time.Instant
import javax.crypto.SecretKey

/** What a sync is doing, for the UI to word. */
enum class SyncPhase { PUSHING, PULLING }

/**
 * Sync orchestrator — push local changes, pull remote changes, handle conflicts.
 */
/** What came of asking the server for a blob. */
enum class BlobRepair { REPAIRED, NOT_ON_SERVER, FAILED }

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
        private const val HTTP_NOT_FOUND = 404
        private const val HTTP_PAYLOAD_TOO_LARGE = 413
        private const val HTTP_STORAGE_FULL = 507

        /** Comfortably under the server's default 5 MB limit per request. */
        internal const val PUSH_CHUNK_BYTES = 3L * 1024 * 1024
        internal const val PUSH_CHUNK_NOTES = 50

        internal const val PULL_PAGE_SIZE = 100

        /**
         * A page holding this much base64 makes the next page smaller. A current
         * server stops a page near 16 MB itself, so this is for older ones.
         */
        internal const val PULL_PAGE_BYTES = 32L * 1024 * 1024

        /** Stops a server that always says there's more. A million changes at full size. */
        private const val MAX_PULL_PAGES = 10_000
        private const val MAX_CURSOR_LENGTH = 1024

        /** Blobs fetched one at a time per sync. The rest wait for the next one. */
        internal const val MAX_BLOB_FETCHES = 20

        /** This many notes binned in one pull is worth telling the user about. */
        internal const val NOTABLE_BIN_COUNT = 10

        /** The server skips anything dated further than this ahead of its clock. */
        private val CLOCK_TOLERANCE = Duration.ofHours(24)
        private val CLOCK_NOTICE_INTERVAL = Duration.ofHours(1)

        /**
         * The attachments-only repair payload for server attachment warnings:
         * every warned blob the lookup can supply, deduplicated by id.
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

    var onSyncPhaseChanged: ((SyncPhase?) -> Unit)? = null
    var onConflictDetected: ((ConflictInfo) -> Unit)? = null
    var onSyncComplete: ((Int) -> Unit)? = null

    /** Notes from the server that were dropped because they couldn't be read. */
    var onNotesQuarantined: ((Int) -> Unit)? = null
    private var quarantined = 0

    /** Notes the server moved to the bin in one pull, when it was a lot of them. */
    var onManyNotesBinned: ((Int) -> Unit)? = null

    /** Something was skipped for being dated too far ahead of the server. */
    var onClockLooksWrong: (() -> Unit)? = null
    private var binned = 0

    // Found by a push, and acted on when the pull after it has the server's time
    private var binnedByPush = 0
    private val unansweredTimes = mutableListOf<String>()
    private var clockNoticeAt: Instant = Instant.EPOCH

    /** Blobs the server has said it doesn't hold, so they aren't asked for again. */
    private val unavailableBlobs = mutableSetOf<String>()

    private val purger = NotePurger(noteRepo, versionRepo, attachmentRepo, syncRepo, transaction)

    private fun requireKey(): SecretKey =
        keyProvider() ?: throw IllegalStateException("Vault is locked")

    // MARK: - SSE

    fun startSSE() {
        sseClient.tokenProvider = { withContext(Dispatchers.IO) { syncClient.getSSEToken().token } }
        sseClient.start()
    }

    fun stopSSE() {
        sseClient.stop()
    }

    // MARK: - Push

    /**
     * Send local changes, in sequential chunks sized by bytes as well as count: a
     * server limits each request (5 MB by default), so a single request holding a
     * whole vault would be refused forever. Each chunk is marked synced before the
     * next. A note too large for the server is skipped and the rest still go.
     */
    suspend fun push() {
        onSyncPhaseChanged?.invoke(SyncPhase.PUSHING)
        try {
            val key = requireKey()
            binnedByPush = 0
            unansweredTimes.clear()
            // Notes waiting on a conflict decision would only be rejected again
            val inConflict = syncRepo.listStoredConflicts().map { it.id }.toSet()
            val notesToSync = noteRepo.listNeedingSync().filter { it.id !in inConflict }
            val deletions = syncRepo.listDeletions()
            val searches = savedSearchRepo.listNeedingSync()
            if (notesToSync.isEmpty() && deletions.isEmpty() && searches.isEmpty()) return

            val chunks = chunk(notesToSync)
            var tooLarge: SyncClientException? = null
            var storageFull: SyncClientException? = null

            for (records in chunks) {
                try {
                    pushChunk(records, key)
                } catch (e: SyncClientException) {
                    when (e.statusCode) {
                        HTTP_PAYLOAD_TOO_LARGE -> {
                            Log.w(TAG, "Server refused a chunk of ${records.size} note(s) as too large")
                            tooLarge = e
                        }
                        HTTP_STORAGE_FULL -> {
                            storageFull = e
                            break
                        }
                        else -> throw e
                    }
                }
            }

            if (storageFull != null) pushBinnedNotes(inConflict, key)
            pushOthers(deletions, searches)

            syncRepo.updateLastPushAt(DateUtils.nowISO8601())
            storageFull?.let { throw it }
            tooLarge?.let { throw it }
        } catch (e: Exception) {
            Log.e(TAG, "Push failed: ${e.javaClass.simpleName}")
            throw e
        }
    }

    /**
     * A full account refuses anything that adds to it, and a chunk fails as a whole,
     * so notes the user has binned to make room would go down with the others. A
     * note going to the bin doesn't count, as long as nothing else rides along, so
     * those go by themselves. Without this a full account could never be emptied.
     */
    private suspend fun pushBinnedNotes(inConflict: Set<String>, key: SecretKey) {
        val binned = noteRepo.listNeedingSync().filter { it.deleted && it.id !in inConflict }
        for (records in chunk(binned)) pushChunk(records, key, includeVersions = false)
    }

    /** Split notes into consecutive chunks of at most [PUSH_CHUNK_NOTES] and [PUSH_CHUNK_BYTES]. */
    private fun chunk(records: List<NoteRecord>): List<List<NoteRecord>> {
        val chunks = mutableListOf<List<NoteRecord>>()
        var current = mutableListOf<NoteRecord>()
        var bytes = 0L
        for (record in records) {
            val size = (record.content.length + record.tags.length + record.attachments.length).toLong()
            if (current.isNotEmpty() && (current.size >= PUSH_CHUNK_NOTES || bytes + size > PUSH_CHUNK_BYTES)) {
                chunks += current
                current = mutableListOf()
                bytes = 0
            }
            current += record
            bytes += size
        }
        if (current.isNotEmpty()) chunks += current
        return chunks
    }

    private suspend fun pushChunk(records: List<NoteRecord>, key: SecretKey, includeVersions: Boolean = true) {
        // Guards the accepted notes below: markSynced only clears the flag if
        // modified_at still matches the snapshot that was actually pushed.
        val pushedModifiedAt = records.associate { it.id to it.modifiedAt }
        val pushedHashes = records.associate { it.id to it.contentHash }

        val syncNotes = records.mapNotNull { record -> toSyncNote(record, key) }
        if (syncNotes.isEmpty()) return

        val noteBytes = syncNotes.sumOf { it.content.length.toLong() }
        val syncVersions =
            if (includeVersions) buildSyncVersions(records, key, PUSH_CHUNK_BYTES - noteBytes) else emptyList()

        // Notes go first, without their blobs: the server only stores a blob
        // whose note it knows, and a blob can be most of a request by itself.
        val response = syncClient.push(
            SyncPushRequest(
                notes = syncNotes,
                attachments = emptyList(),
                versions = syncVersions,
                attachmentsAuthoritative = records.all { refsAreReadable(it.attachments) },
            )
        )

        // Rejections first: the conflict is built from the local row while that
        // row still holds the edit, before pull can touch anything.
        for (rejected in response.rejected) {
            handleRejection(rejected)
        }

        val now = DateUtils.nowISO8601()
        for (accepted in response.accepted) {
            val modifiedAt = pushedModifiedAt[accepted.id] ?: continue
            val cleared = noteRepo.markSynced(
                id = accepted.id,
                syncedAt = accepted.syncedAt?.takeIf { DateUtils.parseISO8601(it) != null } ?: now,
                serverVersion = accepted.serverVersion,
                pushedModifiedAt = modifiedAt,
                pushedHash = pushedHashes[accepted.id],
            )
            if (cleared) {
                noteRepo.getRaw(accepted.id)?.let {
                    versionRepo.insertOrReplace(NoteVersionRecord.from(it, "sync"))
                }
            }
        }

        recordSkipped(response, syncNotes)

        // Send what the server lacks: blobs never uploaded from here, plus any it
        // reports missing. Older servers don't report, so the flag covers those.
        val acceptedIds = response.accepted.map { it.id }.toSet()
        val uploaded = attachmentRepo.listUploadedIds()
        val needed = syncNotes.filter { it.id in acceptedIds }
            .flatMap { it.attachments }
            .filter { it.data !in uploaded }
            .map { it.id to it.data }
        val warned = response.attachmentWarnings.orEmpty().flatMap { it.attachmentIds }.map { it to it }
        try {
            uploadBlobs((needed + warned).distinctBy { it.first })
        } catch (e: Exception) {
            // The notes are marked synced, and only pushing one sends its blobs, so
            // without this they'd never go. A repeat push of the note changes nothing.
            val waiting = needed.map { it.second }.toSet()
            syncNotes.filter { note -> note.id in acceptedIds && note.attachments.any { it.data in waiting } }
                .forEach { noteRepo.requeue(it.id) }
            throw e
        }
    }

    /**
     * Deal with the notes the server took but didn't store. Only ones this push sent
     * count, so a server can't name any others here.
     *
     * One deleted after this edit goes to the bin, as a pulled deletion would. This
     * can't wait for the pull, which won't repeat a deletion it's already past. An edit
     * made since it was sent stays, because that edit is later than the deletion.
     *
     * One it doesn't answer for at all has a time it wouldn't accept. It says so only
     * in `errors`, which aren't for parsing, so the times are kept to check against
     * the server's clock. The note stays queued for when the device's clock is right.
     */
    private suspend fun recordSkipped(response: SyncPushResponse, pushed: List<SyncNote>) {
        val deletedElsewhere = response.skippedDeleted.orEmpty().toSet()
        val answered = (response.accepted.map { it.id } + response.rejected.map { it.id }).toSet()
        for (note in pushed) {
            if (note.id in deletedElsewhere) {
                val wasLive = noteRepo.getRaw(note.id)?.deleted == false
                if (noteRepo.moveToBinIfUnchanged(note.id, note.modifiedAt) && wasLive) binnedByPush++
            } else if (note.id !in answered) {
                unansweredTimes += listOfNotNull(note.createdAt, note.modifiedAt, note.deletedAt, note.archivedAt, note.lockedAt)
            }
        }
    }

    /** Judged by the server's clock, because this device's is the one in doubt. */
    private fun warnIfClockLooksWrong(serverTime: String?) {
        val reference = serverTime?.let { DateUtils.parseISO8601(it) } ?: Instant.now()
        val limit = reference.plus(CLOCK_TOLERANCE)
        val tooFarAhead = unansweredTimes.any { DateUtils.parseISO8601(it)?.isAfter(limit) == true }
        unansweredTimes.clear()

        val now = Instant.now()
        if (tooFarAhead && Duration.between(clockNoticeAt, now) >= CLOCK_NOTICE_INTERVAL) {
            clockNoticeAt = now
            onClockLooksWrong?.invoke()
        }
    }

    private fun toSyncNote(record: NoteRecord, key: SecretKey): SyncNote? {
        // Encrypt tags individually for sync format. Tags that can't be read must
        // not go out as they are, so the note stays queued.
        val encTags = try {
            storageTagsToSyncTags(record.tags, key)
        } catch (e: Exception) {
            Log.w(TAG, "Skipping push of note ${record.id}: tags unreadable")
            return null
        }
        return SyncNote(
            id = record.id,
            createdAt = record.createdAt,
            modifiedAt = record.modifiedAt,
            content = record.content,
            tags = encTags,
            attachments = decodeRefs(record.attachments),
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
            hashChain = decodeChain(record.hashChain).ifEmpty { null },
        )
    }

    /**
     * Snapshots of these notes, newest first, up to [budget] bytes. Older ones have
     * been offered before, and they are history, so they can wait for a later push.
     */
    private suspend fun buildSyncVersions(
        records: List<NoteRecord>,
        key: SecretKey,
        budget: Long,
    ): List<SyncNoteVersion> {
        var remaining = budget
        val versions = mutableListOf<SyncNoteVersion>()
        for (record in records) {
            for (ver in versionRepo.getVersions(record.id)) {
                remaining -= ver.content.length
                if (remaining < 0) return versions
                val tags = try {
                    storageTagsToSyncTags(ver.tags, key)
                } catch (_: Exception) {
                    continue
                }
                versions += SyncNoteVersion(
                    versionKey = ver.versionKey,
                    noteId = ver.noteId,
                    version = ver.version,
                    createdAt = ver.createdAt,
                    syncedAt = ver.syncedAt,
                    content = ver.content,
                    tags = tags,
                    attachments = decodeRefs(ver.attachments),
                    syntaxLanguage = ver.syntaxLanguage,
                    wordWrap = ver.wordWrap,
                    showPreview = ver.showPreview,
                    color = ver.color,
                    reason = ver.reason,
                    contentHash = ver.contentHash,
                    parentHash = ver.parentHash,
                )
            }
        }
        return versions
    }

    /**
     * Upload blobs as attachments-only pushes, as many per request as fit. One
     * that can't be read or is too large for the server is left for the server's
     * warning to report, and must not stop the rest of the sync.
     */
    private suspend fun uploadBlobs(ids: List<Pair<String, String>>) {
        val batch = mutableListOf<SyncAttachment>()
        val blobIds = mutableListOf<String>()
        var bytes = 0L

        suspend fun flush() {
            if (batch.isEmpty()) return
            try {
                syncClient.push(
                    SyncPushRequest(notes = emptyList(), attachments = batch.toList(), versions = emptyList())
                )
                blobIds.forEach { attachmentRepo.markUploaded(it) }
            } catch (e: SyncClientException) {
                if (e.statusCode != HTTP_PAYLOAD_TOO_LARGE) throw e
                Log.w(TAG, "Server refused ${batch.size} attachment(s) as too large")
            }
            batch.clear()
            blobIds.clear()
            bytes = 0
        }

        for ((attachmentId, blobId) in ids) {
            val blob = try {
                attachmentRepo.getBlob(blobId)
            } catch (e: Exception) {
                Log.w(TAG, "Attachment $attachmentId can't be read: ${e.javaClass.simpleName}")
                null
            }
            if (blob == null) {
                Log.w(TAG, "No local data for attachment $attachmentId: the server will warn if it has none either")
                continue
            }
            val encoded = java.util.Base64.getEncoder().encodeToString(blob)
            if (batch.isNotEmpty() && bytes + encoded.length > PUSH_CHUNK_BYTES) flush()
            batch += SyncAttachment(id = attachmentId, data = encoded)
            blobIds += blobId
            bytes += encoded.length
        }
        flush()
    }

    /** Deletions and saved searches go in one small request after the notes. */
    private suspend fun pushOthers(deletions: List<NoteDeletion>, searches: List<SavedSearch>) {
        if (deletions.isEmpty() && searches.isEmpty()) return
        val response = syncClient.push(
            SyncPushRequest(
                notes = emptyList(),
                attachments = emptyList(),
                versions = emptyList(),
                deletions = deletions.map { SyncDeletion(id = it.id, deletedAt = it.deletedAt) },
                savedSearches = searches.map {
                    SyncSavedSearch(
                        id = it.id,
                        name = it.name,
                        query = it.query,
                        order = it.displayOrder,
                        createdAt = it.createdAt,
                        modifiedAt = it.modifiedAt,
                        deleted = it.deleted,
                        deletedAt = it.deletedAt,
                        version = it.version,
                    )
                },
            )
        )
        // A refused deletion is dropped like an accepted one. Sending it again would
        // only re-announce the note to every device, and it returns through the pull.
        response.rejectedDeletions?.takeIf { it.isNotEmpty() }?.let {
            Log.i(TAG, "${it.size} deletion(s) refused: those notes have newer edits")
        }
        deletions.forEach { syncRepo.removeDeletion(it.id) }

        // A search has no accepted list, so an error may mean one was skipped. Nothing
        // says which, and sending them all again is harmless.
        if (!response.errors.isNullOrEmpty()) {
            unansweredTimes += searches.map { it.modifiedAt }
            return
        }
        val now = DateUtils.nowISO8601()
        searches.forEach { savedSearchRepo.markSynced(it.id, now, it.modifiedAt) }
    }

    // MARK: - Pull

    /** What applying one page needs, and what it leaves for afterwards. */
    private class PullPage(
        val syncedAt: String?,
        val key: SecretKey,
        val conflictsById: MutableMap<String, StoredConflict>,
        val pendingDeletions: Map<String, NoteDeletion>,
    ) {
        val raised = mutableListOf<StoredConflict>()
        val droppedBlobs = mutableSetOf<String>()
    }

    /**
     * Fetch remote changes, following "Moving a client to the cursor" in
     * docs/sync-protocol.md. Each page is checked at the boundary and written in one
     * transaction, along with the cursor, so a crash repeats at most one page. Against
     * a server with no cursor the same loop pages by offset, and the watermark moves
     * only once every page has succeeded. The watermark is always the server's clock,
     * because the server filters on its own.
     *
     * @param fullPull ignore the cursor and watermark and fetch everything again.
     */
    suspend fun pull(fullPull: Boolean = false): Int {
        onSyncPhaseChanged?.invoke(SyncPhase.PULLING)
        try {
            val key = requireKey()
            quarantined = 0
            binned = binnedByPush
            binnedByPush = 0
            val syncMeta = syncRepo.getSyncMetadata()
            var cursor = if (fullPull) "" else syncMeta?.pullCursor.orEmpty()
            var lastSyncAt = if (fullPull) null else syncMeta?.lastSyncAt
            // Nothing to repair if this already fetches everything, or it's been done
            var fullPullDone = lastSyncAt == null || syncMeta?.cursorAdopted == true
            // Blobs actually held, not ones merely referenced: a note can arrive
            // before its blob, and the server skips whatever is listed here.
            val knownBlobIds = attachmentRepo.listBlobIds().toMutableList()
            val pendingDeletions = syncRepo.listDeletions().associateBy { it.id }
            var watermark: String? = null
            val conflictsById = syncRepo.listStoredConflicts().associateBy { it.id }.toMutableMap()

            var offset = 0
            var limit = PULL_PAGE_SIZE
            var hasMore = true
            var finished = true
            var pages = 0
            var totalNewCount = 0

            while (hasMore) {
                if (++pages > MAX_PULL_PAGES) {
                    Log.w(TAG, "Stopped a pull after $MAX_PULL_PAGES pages")
                    finished = false
                    break
                }
                val request = SyncPullRequest(
                    cursor = cursor,
                    lastSyncAt = lastSyncAt,
                    knownNoteIds = emptyList(),
                    knownAttachmentIds = knownBlobIds,
                    limit = limit,
                    offset = offset,
                )

                val response = try {
                    syncClient.pull(request)
                } catch (e: OutOfMemoryError) {
                    // A page of large blobs. Fewer changes per page is the only way on
                    if (limit == 1) throw java.io.IOException("A single change is too large to pull")
                    limit = (limit / 4).coerceAtLeast(1)
                    Log.w(TAG, "Pull page too large for memory, trying $limit at a time")
                    continue
                }
                val nextCursor = response.nextCursor?.takeIf(::isUsableCursor)

                // First sight of a cursor. Timestamp pulls can have skipped changes
                // silently, so this once start again from nothing. Sending that any
                // earlier would make an older server re-send everything on every sync.
                if (nextCursor != null && !fullPullDone) {
                    fullPullDone = true
                    cursor = ""
                    lastSyncAt = null
                    continue
                }

                Log.d(TAG, "Pull page: notes=${response.notes.size}, " +
                    "attachments=${response.attachments.size}, hasMore=${response.hasMore}")

                // The first page's time: later pages can't have missed anything
                // that a sync starting from it wouldn't fetch again.
                if (watermark == null) {
                    watermark = response.syncedAt?.takeIf { DateUtils.parseISO8601(it) != null }
                }

                val page = PullPage(response.syncedAt, key, conflictsById, pendingDeletions)

                // One transaction per page: a note never lands without its blobs,
                // and the cursor never moves past something that wasn't applied.
                transaction {
                    for (syncNote in response.notes) {
                        if (!SyncValidator.isValidNote(syncNote)) {
                            Log.w(TAG, "Dropped a malformed note from the server")
                            quarantined++
                            continue
                        }
                        if (processNote(SyncValidator.sanitised(syncNote), page)) {
                            totalNewCount++
                        }
                    }

                    for (syncAtt in response.attachments) {
                        val data = SyncValidator.decodeBlob(syncAtt)
                        if (data == null) {
                            Log.w(TAG, "Dropped a malformed attachment from the server")
                            continue
                        }
                        attachmentRepo.insertOrReplace(
                            com.jottery.android.model.AttachmentRecord(
                                id = syncAtt.id,
                                filename = "",
                                mimeType = "",
                                size = data.size,
                                data = data,
                                uploaded = true,
                            )
                        )
                        knownBlobIds.add(syncAtt.id)
                    }

                    for (syncVer in response.versions) {
                        if (!SyncValidator.isValidVersion(syncVer)) continue
                        val tags = try {
                            syncTagsToStorageTags(syncVer.tags, key)
                        } catch (_: Exception) {
                            Log.w(TAG, "Skipping a version whose tags can't be read")
                            continue
                        }
                        val ver = NoteVersionRecord(
                            versionKey = syncVer.versionKey,
                            noteId = syncVer.noteId,
                            version = syncVer.version,
                            createdAt = syncVer.createdAt,
                            syncedAt = syncVer.syncedAt,
                            content = syncVer.content,
                            tags = tags,
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

                    response.savedSearches?.forEach { applySavedSearch(it) }

                    response.deletions?.forEach { applyDeletion(it, page) }

                    purger.deleteUnreferencedBlobs(page.droppedBlobs)

                    if (nextCursor != null) {
                        syncRepo.savePullCursor(nextCursor)
                    } else {
                        // An older server, or one that lost support: rely on the watermark
                        syncRepo.clearPullCursor()
                    }
                }

                // Only announce conflicts once the page has committed
                for (stored in page.raised) {
                    onConflictDetected?.invoke(describe(stored, key))
                }

                hasMore = response.hasMore ?: false
                val madeNoProgress = if (nextCursor != null) {
                    // A page can hold no notes and still have more behind it, so
                    // the cursor is the only sign of progress.
                    nextCursor == cursor
                } else {
                    response.notes.isEmpty()
                }
                if (nextCursor != null) cursor = nextCursor else offset += response.notes.size

                val pageBytes = response.attachments.sumOf { it.data.length.toLong() }
                limit = when {
                    pageBytes > PULL_PAGE_BYTES -> (limit / 2).coerceAtLeast(1)
                    pageBytes < PULL_PAGE_BYTES / 4 -> (limit * 2).coerceAtMost(PULL_PAGE_SIZE)
                    else -> limit
                }

                if (hasMore && madeNoProgress) {
                    Log.w(TAG, "Server says there's more but made no progress: stopping")
                    finished = false
                    break
                }
            }

            syncRepo.updateLastPullAt(DateUtils.nowISO8601())
            if (finished) watermark?.let { syncRepo.updateLastSyncAt(it) }

            fetchMissingBlobs()

            Log.d(TAG, "Pull complete: $totalNewCount new notes")
            if (quarantined > 0) onNotesQuarantined?.invoke(quarantined)
            if (binned >= NOTABLE_BIN_COUNT) onManyNotesBinned?.invoke(binned)
            warnIfClockLooksWrong(watermark)
            return totalNewCount
        } catch (e: Exception) {
            Log.e(TAG, "Pull failed: ${e.javaClass.simpleName}")
            throw e
        }
    }

    private fun isUsableCursor(cursor: String): Boolean =
        cursor.length <= MAX_CURSOR_LENGTH && cursor.all { it.code in 0x20..0x7E }

    /**
     * Fetch blobs that notes refer to but this device doesn't hold, such as a
     * reference adopted from a conflict: a pull only sends blobs for the notes in
     * it, so nothing else would ever bring them. Never fails the sync.
     */
    private suspend fun fetchMissingBlobs() {
        val missing = noteRepo.allBlobIds() - attachmentRepo.listBlobIds().toSet() - unavailableBlobs
        for (id in missing.take(MAX_BLOB_FETCHES)) {
            try {
                if (!SyncValidator.isUuid(id)) {
                    unavailableBlobs += id
                    continue
                }
                val attachment = syncClient.getAttachment(id)
                val data = SyncValidator.decodeBlob(attachment)?.takeIf { attachment.id == id }
                if (data == null) {
                    unavailableBlobs += id
                    continue
                }
                attachmentRepo.insertOrReplace(
                    com.jottery.android.model.AttachmentRecord(
                        id = id, filename = "", mimeType = "", size = data.size, data = data, uploaded = true,
                    )
                )
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (e: SyncClientException) {
                if (e.statusCode == HTTP_NOT_FOUND) unavailableBlobs += id else return
            } catch (e: Exception) {
                Log.w(TAG, "Couldn't fetch a blob: ${e.javaClass.simpleName}")
                return
            }
        }
    }

    /** Fetch one blob on request. Never throws, because the result is shown to the user. */
    suspend fun repairBlob(id: String): BlobRepair {
        if (!SyncValidator.isUuid(id)) return BlobRepair.NOT_ON_SERVER
        return try {
            val attachment = syncClient.getAttachment(id)
            val data = SyncValidator.decodeBlob(attachment)?.takeIf { attachment.id == id }
                ?: return BlobRepair.NOT_ON_SERVER
            attachmentRepo.insertOrReplace(
                com.jottery.android.model.AttachmentRecord(
                    id = id, filename = "", mimeType = "", size = data.size, data = data, uploaded = true,
                )
            )
            unavailableBlobs -= id
            BlobRepair.REPAIRED
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: SyncClientException) {
            if (e.statusCode == HTTP_NOT_FOUND) BlobRepair.NOT_ON_SERVER else BlobRepair.FAILED
        } catch (e: Exception) {
            Log.w(TAG, "Couldn't fetch a blob: ${e.javaClass.simpleName}")
            BlobRepair.FAILED
        }
    }

    /**
     * Apply a deletion from the server, which isn't trusted: it could list every
     * note. So the note goes to the bin rather than away, where it stays for the
     * usual time by this device's clock. The same rule as the server's applies to a
     * local edit: one made after the deletion wins, and goes back up with the next
     * push. One made before it loses, but the bin still keeps it.
     */
    private suspend fun applyDeletion(deletion: SyncDeletion, page: PullPage) {
        if (!SyncValidator.isUuid(deletion.id) || deletion.id in page.conflictsById) return
        val deletedAt = DateUtils.parseISO8601(deletion.deletedAt) ?: return // Keeping can be undone
        val local = noteRepo.getRaw(deletion.id) ?: return
        if (local.needsSync) {
            val editedAt = DateUtils.parseISO8601(local.modifiedAt)
            if (editedAt == null || editedAt.isAfter(deletedAt)) {
                Log.d(TAG, "Kept note ${deletion.id}: an edit here is newer than its deletion")
                return
            }
        }
        if (local.deleted && !local.needsSync) return
        noteRepo.moveToBinForServer(deletion.id, local.deletedAt ?: deletion.deletedAt)
        if (!local.deleted) binned++
    }

    /**
     * Apply one pulled note. Returns true if it is new to this device.
     *
     * Nothing here may destroy a local edit: see [PullDecision].
     */
    private suspend fun processNote(syncNote: SyncNote, page: PullPage): Boolean {
        val key = page.key

        // Deleted here and not yet pushed. A later edit from elsewhere still comes
        // back, as it would on the server.
        page.pendingDeletions[syncNote.id]?.let { deletion ->
            val editedAt = DateUtils.parseISO8601(syncNote.modifiedAt)
            val deletedAt = DateUtils.parseISO8601(deletion.deletedAt)
            if (editedAt != null && deletedAt != null && !editedAt.isAfter(deletedAt)) return false
        }

        // A note awaiting a decision stays as it is. The stored server side is
        // kept current, so choosing it can't adopt a version already overtaken.
        page.conflictsById[syncNote.id]?.let { existing ->
            val updated = conflictFrom(syncNote, existing)
            syncRepo.saveConflict(updated)
            page.conflictsById[syncNote.id] = updated
            return false
        }

        val local = noteRepo.getRaw(syncNote.id)
        if (local == null) {
            noteRepo.insertOrReplace(buildRecord(syncNote, page.syncedAt, key, null) ?: return false)
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
                val remote = buildRecord(syncNote, page.syncedAt, key, local) ?: return false
                noteRepo.insertOrReplace(remote)
                recordReplaced(local, remote, page)
            }
            PullAction.FAST_FORWARD_KEEP_FLAGS -> {
                val remote = buildRecord(syncNote, page.syncedAt, key, local) ?: return false
                val merged = remote.copy(
                    pinned = local.pinned,
                    archived = local.archived,
                    archivedAt = local.archivedAt,
                    locked = local.locked,
                    lockedAt = local.lockedAt,
                    deleted = local.deleted,
                    deletedAt = local.deletedAt,
                    localDeletedAt = local.localDeletedAt,
                    needsSync = true,
                )
                noteRepo.insertOrReplace(merged)
                recordReplaced(local, merged, page)
            }
            PullAction.REPLACE_WITH_SNAPSHOT -> {
                val remote = buildRecord(syncNote, page.syncedAt, key, local) ?: return false
                versionRepo.insertOrReplace(NoteVersionRecord.safetySnapshot(local, "pre-sync-overwrite"))
                noteRepo.insertOrReplace(remote)
                recordReplaced(local, remote, page)
            }
            PullAction.CONFLICT -> {
                val stored = conflictFrom(syncNote, null)
                syncRepo.saveConflict(stored)
                page.conflictsById[syncNote.id] = stored
                page.raised.add(stored)
            }
        }
        return false
    }

    /** Note what a replacement left behind: blobs to clear, and any move to the bin. */
    private fun recordReplaced(before: NoteRecord, after: NoteRecord, page: PullPage) {
        page.droppedBlobs += decodeRefs(before.attachments).map { it.data } -
            decodeRefs(after.attachments).map { it.data }.toSet()
        if (after.deleted && !before.deleted) binned++
    }

    private fun flagsDiffer(local: NoteRecord, remote: SyncNote): Boolean =
        local.pinned != remote.pinned ||
            local.archived != remote.archived ||
            local.locked != (remote.locked ?: false) ||
            local.deleted != remote.deleted ||
            local.color != remote.color ||
            local.wordWrap != (remote.wordWrap ?: local.wordWrap) ||
            local.syntaxLanguage != (remote.syntaxLanguage ?: local.syntaxLanguage) ||
            local.showPreview != (remote.showPreview ?: local.showPreview) ||
            // Clients aren't consistent about advancing the hash for these either
            decodeRefs(local.attachments).map { it.id }.toSet() != remote.attachments.map { it.id }.toSet()

    /**
     * Build a local row from a pulled note, or null to quarantine it: a note that
     * can't be read under this key must not replace anything or be stored with
     * its tags missing. Force Full Sync fetches it again.
     */
    private fun buildRecord(
        syncNote: SyncNote,
        syncedAt: String?,
        key: SecretKey,
        local: NoteRecord?,
    ): NoteRecord? {
        val tagsBlob = try {
            CryptoService.decrypt(CryptoService.parseEncryptedJSON(syncNote.content), key)
            syncTagsToStorageTags(syncNote.tags, key)
        } catch (e: Exception) {
            Log.w(TAG, "Skipping pulled note ${syncNote.id}: it can't be decrypted with this key")
            quarantined++
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
            syncHash = syncNote.contentHash,
            // Its time in the bin runs from when this device saw it go there
            localDeletedAt = when {
                !syncNote.deleted -> null
                local?.deleted == true -> local.localDeletedAt ?: DateUtils.nowISO8601()
                else -> DateUtils.nowISO8601()
            },
        )
    }

    /** Last write wins, as on the server. A search with local changes keeps them. */
    private suspend fun applySavedSearch(remote: SyncSavedSearch) {
        if (!SyncValidator.isValidSavedSearch(remote)) return
        val local = savedSearchRepo.getRaw(remote.id)
        if (local != null && local.needsSync) {
            val localTime = DateUtils.parseISO8601(local.modifiedAt)
            val remoteTime = DateUtils.parseISO8601(remote.modifiedAt)
            if (localTime != null && remoteTime != null && !remoteTime.isAfter(localTime)) return
        }
        savedSearchRepo.insertOrReplace(
            SavedSearch(
                id = remote.id,
                name = remote.name,
                query = remote.query,
                displayOrder = remote.order,
                createdAt = remote.createdAt,
                modifiedAt = remote.modifiedAt,
                syncedAt = DateUtils.nowISO8601(),
                deleted = remote.deleted,
                deletedAt = remote.deletedAt,
                version = remote.version,
                needsSync = false,
            )
        )
    }

    private fun decodeChain(chain: String?): List<String> = chain?.let {
        try { json.decodeFromString<List<String>>(it) } catch (_: Exception) { emptyList() }
    } ?: emptyList()

    // MARK: - Full Sync Cycle

    /**
     * Push, then pull. A failed push doesn't stop the pull, which can't overwrite
     * anything queued: changes from other devices keep arriving, and the push
     * error is reported afterwards.
     */
    suspend fun fullSync(fullPull: Boolean = false): Int {
        var pushFailure: Exception? = null
        try {
            push()
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: Exception) {
            pushFailure = e
        }
        val newCount = pull(fullPull)

        onSyncPhaseChanged?.invoke(null)
        onSyncComplete?.invoke(newCount)
        pushFailure?.let { throw it }
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
                serverDeleted = rejected.serverDeleted,
                serverDeletedAt = rejected.serverDeletedAt?.takeIf { DateUtils.parseISO8601(it) != null },
                serverContentHash = rejected.serverContentHash,
                serverParentHash = rejected.serverParentHash,
                serverHashChain = rejected.serverHashChain,
                ancestorContent = rejected.ancestorContent,
                ancestorTags = rejected.ancestorTags,
            )
            syncRepo.saveConflict(stored)
            onConflictDetected?.invoke(describe(stored, requireKey()))
        } catch (e: Exception) {
            Log.e(TAG, "Failed to handle rejection for ${rejected.id}: ${e.javaClass.simpleName}")
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
        // What's on screen may be behind: a pull can move the server's side on,
        // and the user can edit the note while it waits for a decision.
        val stored = syncRepo.listStoredConflicts().firstOrNull { it.id == conflict.id } ?: conflict.stored
        val current = describe(stored, key)
        val localRecord = noteRepo.getRaw(conflict.id)
            ?: throw IllegalStateException("The note no longer exists")
        if (strategy != ConflictResolutionStrategy.KEEP_LOCAL && current.serverUnreadable) {
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

                    // This push will be authoritative, and keeping the local text
                    // says nothing about attachments another device has added.
                    val localRefs = decodeRefs(localRecord.attachments)
                    val refs = localRefs + stored.serverAttachments.filter { server ->
                        localRefs.none { it.id == server.id }
                    }

                    noteRepo.updateRaw(
                        localRecord.copy(
                            attachments = json.encodeToString(refs),
                            version = stored.serverVersion + 1,
                            parentHash = stored.serverContentHash,
                            hashChain = json.encodeToString(merged),
                            syncHash = stored.serverContentHash,
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
                        content = current.localContent,
                        tags = current.localTags,
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
        deletedAt = stored.serverDeletedAt ?: DateUtils.nowISO8601().takeIf { stored.serverDeleted },
        // Its time in the bin runs from when this device saw it go there
        localDeletedAt = if (stored.serverDeleted) local.localDeletedAt ?: DateUtils.nowISO8601() else null,
        color = stored.serverColor,
        syntaxLanguage = stored.serverSyntaxLanguage ?: local.syntaxLanguage,
        wordWrap = stored.serverWordWrap ?: local.wordWrap,
        showPreview = stored.serverShowPreview ?: local.showPreview,
        version = stored.serverVersion,
        contentHash = stored.serverContentHash,
        parentHash = stored.serverParentHash,
        hashChain = stored.serverHashChain?.let { json.encodeToString(it) },
        needsSync = false,
        syncHash = stored.serverContentHash,
        syncedAt = DateUtils.nowISO8601(),
        modifiedAt = stored.serverModifiedAt,
    )

    private fun decodeRefs(attachments: String): List<AttachmentRef> = try {
        json.decodeFromString(attachments)
    } catch (_: Exception) {
        emptyList()
    }

    /** A list that can't be read goes out empty, so it can't be called complete. */
    private fun refsAreReadable(attachments: String): Boolean = try {
        json.decodeFromString<List<AttachmentRef>>(attachments)
        true
    } catch (_: Exception) {
        false
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
