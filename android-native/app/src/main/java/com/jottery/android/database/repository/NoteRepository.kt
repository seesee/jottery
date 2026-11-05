package com.jottery.android.database.repository

import android.util.Log
import com.jottery.android.crypto.CryptoService
import com.jottery.android.database.dao.ExpiredNote
import com.jottery.android.database.dao.INLINE_CONTENT_LIMIT
import com.jottery.android.database.dao.NoteDao
import com.jottery.android.database.dao.NoteVersionDao
import com.jottery.android.model.AttachmentRef
import com.jottery.android.model.DecryptedNote
import com.jottery.android.model.NoteRecord
import com.jottery.android.model.NoteVersionRecord
import com.jottery.android.model.SyncableNoteFields
import com.jottery.android.service.ExportService
import com.jottery.android.util.DateUtils
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import java.time.Instant
import javax.crypto.SecretKey

/**
 * CRUD operations on notes with automatic encrypt/decrypt.
 * All public methods are suspend functions that delegate to Room DAOs.
 * CryptoService calls are synchronous (no suspend).
 */
class NoteRepository(
    private val noteDao: NoteDao,
    private val noteVersionDao: NoteVersionDao,
    /** Runs a block in one database transaction. */
    private val runInTransaction: suspend (suspend () -> Unit) -> Unit = { it() },
) {
    private val json = Json { ignoreUnknownKeys = true }

    companion object {
        private const val TAG = "NoteRepo"
        private const val READ_CHUNK = 512 * 1024
    }

    // MARK: - Read

    /** Result of listing active notes with decryption stats. */
    data class ListResult(
        val notes: List<DecryptedNote>,
        val totalRecords: Int,
        val failedCount: Int,
    )

    /** Fetch all active (non-deleted, non-archived) notes, decrypted. */
    suspend fun listActive(key: SecretKey): List<DecryptedNote> {
        return listActiveWithStats(key).notes
    }

    /** Fetch all active notes with decryption stats (total records, failures). */
    suspend fun listActiveWithStats(
        key: SecretKey,
        known: Map<String, DecryptedNote> = emptyMap(),
    ): ListResult {
        val records = noteDao.listActive()
        var failCount = 0
        val decrypted = mutableListOf<DecryptedNote>()
        for (record in records) {
            val unchanged = known[record.id]?.takeIf { isSameAs(record, it) }
            if (unchanged != null) {
                decrypted.add(unchanged)
                continue
            }
            try {
                decrypted.add(decrypt(hydrate(record), key))
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (e: Exception) {
                failCount++
                if (failCount <= 3) {
                    Log.w(TAG, "Decrypt failed for note ${record.id}: ${e.javaClass.simpleName}")
                }
            }
        }
        if (failCount > 0) {
            Log.w(TAG, "$failCount/${records.size} notes failed to decrypt")
        }
        return ListResult(decrypted, records.size, failCount)
    }

    /** Fetch all deleted notes, decrypted (recycle bin). */
    suspend fun listDeleted(key: SecretKey): List<DecryptedNote> {
        return decryptEach(noteDao.listDeleted(), key)
    }

    /** Fetch all archived notes, decrypted. */
    suspend fun listArchived(
        key: SecretKey,
        known: Map<String, DecryptedNote> = emptyMap(),
    ): List<DecryptedNote> {
        return decryptEach(noteDao.listArchived(), key, known)
    }

    /**
     * Whether a row is the note already in memory, so it needn't be decrypted
     * again. Rows from before hash chains can't show that, so they always are.
     */
    private fun isSameAs(record: NoteRecord, note: DecryptedNote): Boolean =
        record.contentHash != null &&
            record.contentHash == note.contentHash &&
            record.version == note.version &&
            record.needsSync == note.needsSync &&
            record.pinned == note.pinned &&
            record.archived == note.archived &&
            record.locked == note.locked &&
            record.deleted == note.deleted &&
            record.wordWrap == note.wordWrap &&
            record.syntaxLanguage == note.syntaxLanguage &&
            record.showPreview == note.showPreview &&
            record.color == note.color &&
            DateUtils.parseISO8601(record.modifiedAt) == note.modifiedAt &&
            record.syncedAt?.let(DateUtils::parseISO8601) == note.syncedAt

    /** One note that can't be read must not stop the others loading. */
    private suspend fun decryptEach(
        records: List<NoteRecord>,
        key: SecretKey,
        known: Map<String, DecryptedNote> = emptyMap(),
    ): List<DecryptedNote> =
        records.mapNotNull { record ->
            known[record.id]?.takeIf { isSameAs(record, it) }?.let { return@mapNotNull it }
            try {
                decrypt(hydrate(record), key)
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (_: Exception) {
                null
            }
        }

    /** Fill in content too large to read with the row: see [INLINE_CONTENT_LIMIT]. */
    private suspend fun hydrate(record: NoteRecord): NoteRecord {
        if (record.content.isNotEmpty()) return record
        val length = noteDao.contentLength(record.id) ?: return record
        val content = StringBuilder(length)
        var start = 1
        while (start <= length) {
            content.append(noteDao.contentChunk(record.id, start, READ_CHUNK) ?: break)
            start += READ_CHUNK
        }
        return record.copy(content = content.toString())
    }

    /** Fetch a single note by ID, decrypted. */
    suspend fun get(id: String, key: SecretKey): DecryptedNote? {
        return decrypt(getRaw(id) ?: return null, key)
    }

    /** Fetch a raw (encrypted) record by ID. */
    suspend fun getRaw(id: String): NoteRecord? {
        return noteDao.getById(id)?.let { hydrate(it) }
    }

    /** Fetch all records that need syncing. */
    suspend fun listNeedingSync(): List<NoteRecord> {
        return noteDao.listNeedingSync().map { hydrate(it) }
    }

    /** Fetch all raw records (for backup). */
    suspend fun listAllRaw(): List<NoteRecord> {
        return noteDao.listAll().map { hydrate(it) }
    }

    /**
     * Return note IDs and their referenced attachment blob IDs for all non-deleted notes.
     * Uses raw DB access — no decryption required.
     */
    suspend fun listIdsAndAttachmentBlobIds(): List<Pair<String, List<String>>> {
        val rows = noteDao.listIdsAndAttachments()
        return rows.map { row ->
            val blobIds = if (row.attachments != "[]") {
                try {
                    val refs: List<AttachmentRef> = json.decodeFromString(row.attachments)
                    refs.map { it.data }
                } catch (_: Exception) {
                    emptyList()
                }
            } else {
                emptyList()
            }
            Pair(row.id, blobIds)
        }
    }

    /** The blobs one note refers to. */
    suspend fun blobIdsOf(id: String): Set<String> =
        getRaw(id)?.let { refs(it.attachments).mapTo(mutableSetOf()) { ref -> ref.data } } ?: emptySet()

    /** The blobs any note refers to, in or out of the bin. */
    suspend fun allBlobIds(): Set<String> =
        listIdsAndAttachmentBlobIds().flatMapTo(mutableSetOf()) { it.second }

    private fun refs(attachments: String): List<AttachmentRef> = try {
        json.decodeFromString(attachments)
    } catch (_: Exception) {
        emptyList()
    }

    /** Count of active notes. */
    suspend fun countActive(): Int {
        return noteDao.countActive()
    }

    // MARK: - Create

    /** Create a new note with the given content. */
    suspend fun create(
        content: String,
        tags: List<String> = emptyList(),
        key: SecretKey,
        syntaxLanguage: String = "markdown",
        wordWrap: Boolean = true,
        showPreview: Boolean = false,
        color: String? = null,
    ): DecryptedNote {
        val encContent = CryptoService.encryptText(content, key)
        val encTags = CryptoService.encryptStringArray(tags, key)

        val contentJSON = CryptoService.serializeEncryptedJSON(encContent)
        val tagsJSON = CryptoService.serializeEncryptedJSON(encTags)

        var record = NoteRecord.new(
            encryptedContent = contentJSON,
            encryptedTags = tagsJSON,
        ).copy(syntaxLanguage = syntaxLanguage, wordWrap = wordWrap, showPreview = showPreview, color = color)

        // Compute content hash using sync-format tags
        val syncTags = encryptTagsToSyncFormat(tags, key)
        val contentHash = CryptoService.computeContentHash(
            SyncableNoteFields(
                content = contentJSON,
                tags = syncTags,
                attachmentIds = emptyList(),
                pinned = record.pinned,
                archived = record.archived,
                locked = record.locked,
                syntaxLanguage = record.syntaxLanguage,
                wordWrap = record.wordWrap,
                showPreview = record.showPreview,
                color = record.color,
            )
        )
        record = record.copy(contentHash = contentHash, parentHash = null)
        val hashChain = CryptoService.updateHashChain(contentHash, emptyList())
        record = record.copy(hashChain = json.encodeToString(hashChain))

        noteDao.insertOrReplace(record)

        return decrypt(record, key)
    }

    /** Store a note from an export file with everything the file carried. */
    suspend fun createImported(
        id: String,
        note: ExportService.ExportNote,
        attachments: List<AttachmentRef>,
        key: SecretKey,
    ) {
        val contentJSON = CryptoService.serializeEncryptedJSON(CryptoService.encryptText(note.content, key))
        val contentHash = CryptoService.computeContentHash(
            SyncableNoteFields(
                content = contentJSON,
                tags = encryptTagsToSyncFormat(note.tags, key),
                attachmentIds = attachments.map { it.id },
                pinned = note.pinned,
                archived = note.archived,
                locked = note.locked,
                syntaxLanguage = note.syntaxLanguage,
                wordWrap = note.wordWrap,
                showPreview = note.showPreview,
                color = note.color,
            )
        )
        val now = DateUtils.nowISO8601()
        val createdAt = DateUtils.parseISO8601(note.createdAt)?.let(DateUtils::formatISO8601) ?: now
        val modifiedAt = DateUtils.parseISO8601(note.modifiedAt)?.let(DateUtils::formatISO8601) ?: now
        noteDao.insertOrReplace(
            NoteRecord(
                id = id,
                createdAt = createdAt,
                modifiedAt = modifiedAt,
                content = contentJSON,
                tags = CryptoService.serializeEncryptedJSON(CryptoService.encryptStringArray(note.tags, key)),
                attachments = json.encodeToString(attachments),
                pinned = note.pinned,
                archived = note.archived,
                archivedAt = if (note.archived) modifiedAt else null,
                locked = note.locked,
                lockedAt = if (note.locked) modifiedAt else null,
                version = 0,
                wordWrap = note.wordWrap,
                syntaxLanguage = note.syntaxLanguage,
                showPreview = note.showPreview,
                color = note.color,
                contentHash = contentHash,
                parentHash = null,
                hashChain = json.encodeToString(CryptoService.updateHashChain(contentHash, emptyList())),
                needsSync = true,
            )
        )
    }

    // MARK: - Update

    /**
     * Save what the editor owns: content, tags, language, word wrap, colour and
     * preview. Everything else comes from the current row, so an editor holding
     * an older copy can't undo a delete, archive, lock or pin, drop attachments
     * added elsewhere, or write decrypted filenames back to the database.
     *
     * Returns false, writing nothing, if the note is gone or in the bin.
     */
    suspend fun update(note: DecryptedNote, key: SecretKey): Boolean {
        var saved = false
        runInTransaction {
            val current = getRaw(note.id)
            if (current != null && !current.deleted) {
                write(current, note, key)
                saved = true
            }
        }
        return saved
    }

    private suspend fun write(current: NoteRecord, note: DecryptedNote, key: SecretKey) {
        val contentJSON = CryptoService.serializeEncryptedJSON(CryptoService.encryptText(note.content, key))
        val tagsJSON = CryptoService.serializeEncryptedJSON(CryptoService.encryptStringArray(note.tags, key))

        val previousChain: List<String> = current.hashChain?.let {
            try {
                json.decodeFromString(it)
            } catch (_: Exception) {
                emptyList()
            }
        } ?: emptyList()

        // Compute content hash using sync-format tags
        val attachmentIds = try {
            json.decodeFromString<List<AttachmentRef>>(current.attachments).map { it.id }.sorted()
        } catch (_: Exception) {
            emptyList()
        }
        val contentHash = CryptoService.computeContentHash(
            SyncableNoteFields(
                content = contentJSON,
                tags = encryptTagsToSyncFormat(note.tags, key),
                attachmentIds = attachmentIds,
                pinned = current.pinned,
                archived = current.archived,
                locked = current.locked,
                syntaxLanguage = note.syntaxLanguage,
                wordWrap = note.wordWrap,
                showPreview = note.showPreview,
                color = note.color,
            )
        )
        // What the server holds: this row itself, if nothing is waiting to go
        val serverHash = current.syncHash ?: current.contentHash.takeIf { !current.needsSync }
        val hashChainStr = json.encodeToString(
            CryptoService.updateHashChain(contentHash, previousChain, keep = serverHash)
        )

        // First-edit snapshot: when version is 0, create a snapshot of the original state
        // then advance version to 1
        val newVersion = if (current.version == 0) {
            noteVersionDao.insertOrReplace(NoteVersionRecord.from(current, "manual-sync"))
            1
        } else {
            current.version
        }

        noteDao.update(
            current.copy(
                content = contentJSON,
                tags = tagsJSON,
                modifiedAt = DateUtils.nowISO8601(),
                version = newVersion,
                wordWrap = note.wordWrap,
                syntaxLanguage = note.syntaxLanguage,
                showPreview = note.showPreview,
                color = note.color,
                contentHash = contentHash,
                parentHash = current.contentHash,
                hashChain = hashChainStr,
                needsSync = true,
                syncHash = serverHash,
            )
        )
    }

    /** Update a raw record (used during sync). */
    suspend fun updateRaw(record: NoteRecord) {
        noteDao.update(record)
    }

    /** Soft-delete a note. */
    suspend fun softDelete(id: String) {
        val now = DateUtils.nowISO8601()
        noteDao.softDelete(id, deletedAt = now, modifiedAt = now)
    }

    /** Restore a soft-deleted note. */
    suspend fun restore(id: String) {
        val now = DateUtils.nowISO8601()
        noteDao.restore(id, modifiedAt = now)
    }

    /** Remove the row. Anything else needs NotePurger, which calls this. */
    suspend fun hardDelete(id: String) {
        noteDao.hardDelete(id)
    }

    /**
     * Honour a deletion from the server by moving the note to the bin, where it
     * can still be restored. A server that lied can't then wipe this device.
     */
    suspend fun moveToBinForServer(id: String, deletedAt: String) {
        noteDao.moveToBinForServer(id, deletedAt, DateUtils.nowISO8601())
    }

    /** Returns false if the note has moved on since [pushedModifiedAt], and was left alone. */
    suspend fun moveToBinIfUnchanged(id: String, pushedModifiedAt: String): Boolean =
        noteDao.moveToBinIfUnchanged(id, pushedModifiedAt, DateUtils.nowISO8601()) > 0

    /** Archive a note. */
    suspend fun archive(id: String) {
        val now = DateUtils.nowISO8601()
        noteDao.archive(id, archivedAt = now, modifiedAt = now)
    }

    /** Unarchive a note. */
    suspend fun unarchive(id: String) {
        val now = DateUtils.nowISO8601()
        noteDao.unarchive(id, modifiedAt = now)
    }

    /** Toggle pin status. */
    suspend fun togglePin(id: String) {
        val record = getRaw(id) ?: return
        val now = DateUtils.nowISO8601()
        val updated = record.copy(
            pinned = !record.pinned,
            modifiedAt = now,
            needsSync = true,
        )
        noteDao.update(updated)
    }

    /** Toggle lock status. */
    suspend fun toggleLock(id: String) {
        val record = getRaw(id) ?: return
        val now = DateUtils.nowISO8601()
        val updated = if (record.locked) {
            record.copy(
                locked = false,
                lockedAt = null,
                modifiedAt = now,
                needsSync = true,
            )
        } else {
            record.copy(
                locked = true,
                lockedAt = now,
                modifiedAt = now,
                needsSync = true,
            )
        }
        noteDao.update(updated)
    }

    /**
     * Mark a pushed note as synced. Returns true if the flag was cleared, false
     * if the note changed after [pushedModifiedAt] was read, in which case it
     * stays queued for the next push.
     */
    suspend fun markSynced(
        id: String,
        syncedAt: String,
        serverVersion: Int?,
        pushedModifiedAt: String,
        pushedHash: String?,
    ): Boolean {
        // True even if the note has moved on: this is now what the server holds
        pushedHash?.let { noteDao.setSyncHash(id, it) }
        val updated = if (serverVersion != null) {
            noteDao.markSyncedWithVersionIfUnchanged(id, syncedAt, serverVersion, pushedModifiedAt)
        } else {
            noteDao.markSyncedIfUnchanged(id, syncedAt, pushedModifiedAt)
        }
        return updated > 0
    }

    /** Send a note again without changing it. */
    suspend fun requeue(id: String) = noteDao.requeue(id)

    // MARK: - Attachment Management

    /** Add or remove tags on one note, as an edit. */
    suspend fun changeTags(id: String, key: SecretKey, change: (List<String>) -> List<String>) {
        val note = get(id, key) ?: return
        val tags = change(note.tags)
        if (tags != note.tags) update(note.copy(tags = tags), key)
    }

    /**
     * Add an attachment reference to a note. This is an edit like any other, so it
     * advances the hash: otherwise a remote edit pulled before the next push would
     * look like a descendant and replace the list.
     */
    suspend fun addAttachment(noteId: String, ref: AttachmentRef, key: SecretKey) {
        changeAttachments(noteId, key) { it + ref }
    }

    /** The references as stored, with filenames still encrypted. */
    suspend fun rawAttachmentRefs(noteId: String): List<AttachmentRef> =
        getRaw(noteId)?.let { refs(it.attachments) } ?: emptyList()

    /** Replace a note's references. */
    suspend fun setAttachments(noteId: String, refs: List<AttachmentRef>, key: SecretKey) {
        changeAttachments(noteId, key) { refs }
    }

    /** Remove an attachment reference from a note. */
    suspend fun removeAttachment(noteId: String, attachmentId: String, key: SecretKey) {
        changeAttachments(noteId, key) { refs -> refs.filter { it.id != attachmentId } }
    }

    private suspend fun changeAttachments(
        noteId: String,
        key: SecretKey,
        change: (List<AttachmentRef>) -> List<AttachmentRef>,
    ) {
        runInTransaction {
            val record = getRaw(noteId) ?: return@runInTransaction
            val changed = record.copy(attachments = json.encodeToString(change(refs(record.attachments))))
            write(changed, decrypt(changed, key), key)
        }
    }

    // MARK: - Insert Raw (for sync pull)

    /** Insert or replace a raw record (from server). */
    suspend fun insertOrReplace(record: NoteRecord) {
        noteDao.insertOrReplace(record)
    }

    // MARK: - Repair

    /**
     * Earlier builds wrote decrypted filenames into the attachments JSON and
     * pushed them. Encrypt any still there and queue the note, so the server's
     * copy is replaced as well. Returns the number of notes repaired.
     */
    suspend fun repairPlaintextFilenames(key: SecretKey): Int {
        var repaired = 0
        for (row in noteDao.listIdsAndAttachments()) {
            if (row.attachments == "[]") continue
            val refs: List<AttachmentRef> = try {
                json.decodeFromString(row.attachments)
            } catch (_: Exception) {
                continue
            }
            val fixed = refs.map { ref ->
                try {
                    CryptoService.parseEncryptedJSON(ref.filename)
                    ref
                } catch (_: Exception) {
                    ref.copy(
                        filename = CryptoService.serializeEncryptedJSON(CryptoService.encryptText(ref.filename, key))
                    )
                }
            }
            if (fixed != refs) {
                runInTransaction {
                    getRaw(row.id)?.let {
                        noteDao.update(it.copy(attachments = json.encodeToString(fixed), needsSync = true))
                    }
                }
                repaired++
            }
        }
        return repaired
    }

    /** Whether every note, in or out of the bin, opens with [key]. */
    suspend fun allNotesOpenWith(key: SecretKey): Boolean =
        noteDao.listAll().all { record ->
            try {
                decrypt(hydrate(record), key)
                true
            } catch (_: Exception) {
                false
            }
        }

    // MARK: - Purge

    /** Notes whose time in the bin ran out before [cutoff]. See NotePurger. */
    suspend fun listExpiredDeleted(cutoff: String): List<ExpiredNote> =
        noteDao.listExpiredDeleted(cutoff)

    suspend fun startBinClocks() = noteDao.startBinClocks(DateUtils.nowISO8601())

    // MARK: - Private

    /**
     * Encrypt plaintext tags to sync format (each tag JSON-encoded then individually encrypted).
     * This is needed for content hash computation to match the web client.
     */
    private fun encryptTagsToSyncFormat(tags: List<String>, key: SecretKey): List<String> {
        return tags.map { tag ->
            val tagJSON = json.encodeToString(tag)
            val encrypted = CryptoService.encryptText(tagJSON, key)
            CryptoService.serializeEncryptedJSON(encrypted)
        }
    }

    /**
     * Strip accumulated JSON quote escaping from a tag value.
     * A sync format mismatch could store `"tag"` instead of `tag`, and each
     * sync round-trip would add another layer of escaping. This peels them off.
     */
    private fun stripAccumulatedQuotes(tag: String): String {
        var t = tag
        while (t.length >= 2 && t.startsWith('"') && t.endsWith('"')) {
            val unescaped = t.substring(1, t.length - 1).replace("\\\"", "\"")
            if (unescaped == t) break
            t = unescaped
        }
        return t
    }

    /** Decrypt a NoteRecord to a DecryptedNote. */
    private fun decrypt(record: NoteRecord, key: SecretKey): DecryptedNote {
        val encContent = CryptoService.parseEncryptedJSON(record.content)
        val content = CryptoService.decryptText(encContent, key)

        val encTags = CryptoService.parseEncryptedJSON(record.tags)
        val tags: List<String>
        val decryptedTagsText = CryptoService.decryptText(encTags, key)
        tags = if (decryptedTagsText.isEmpty()) {
            emptyList()
        } else if (decryptedTagsText.startsWith("[")) {
            try {
                val decoded: List<String> = json.decodeFromString(decryptedTagsText)
                // Strip any accumulated JSON quote escaping from sync format mismatch
                decoded.map { tag -> stripAccumulatedQuotes(tag) }
            } catch (_: Exception) {
                emptyList()
            }
        } else if (decryptedTagsText.startsWith("\"")) {
            // Single JSON-encoded string (mismatched sync/storage format) — decode it
            try {
                val single: String = json.decodeFromString(decryptedTagsText)
                listOf(stripAccumulatedQuotes(single))
            } catch (_: Exception) {
                listOf(decryptedTagsText.trim('"'))
            }
        } else {
            // Plain string — single tag or comma-separated
            decryptedTagsText.split(",")
                .map { it.trim() }
                .filter { it.isNotEmpty() }
        }

        val rawAttachments: List<AttachmentRef> = try {
            json.decodeFromString(record.attachments)
        } catch (_: Exception) {
            emptyList()
        }

        // Decrypt attachment filenames (stored as encrypted JSON strings). A name
        // that won't open is blanked, never shown as raw ciphertext; one that
        // isn't ciphertext at all is a plaintext name from an older build.
        val attachments = rawAttachments.map { ref ->
            val encrypted = try {
                CryptoService.parseEncryptedJSON(ref.filename)
            } catch (_: Exception) {
                return@map ref
            }
            try {
                ref.copy(filename = CryptoService.decryptText(encrypted, key))
            } catch (_: Exception) {
                ref.copy(filename = "")
            }
        }

        val hashChain: List<String> = if (record.hashChain != null) {
            try {
                json.decodeFromString(record.hashChain!!)
            } catch (_: Exception) {
                emptyList()
            }
        } else {
            emptyList()
        }

        return DecryptedNote(
            id = record.id,
            createdAt = DateUtils.parseISO8601(record.createdAt) ?: Instant.now(),
            modifiedAt = DateUtils.parseISO8601(record.modifiedAt) ?: Instant.now(),
            syncedAt = record.syncedAt?.let { DateUtils.parseISO8601(it) },
            content = content,
            tags = tags,
            attachments = attachments,
            pinned = record.pinned,
            archived = record.archived,
            archivedAt = record.archivedAt?.let { DateUtils.parseISO8601(it) },
            locked = record.locked,
            lockedAt = record.lockedAt?.let { DateUtils.parseISO8601(it) },
            deleted = record.deleted,
            deletedAt = record.deletedAt?.let { DateUtils.parseISO8601(it) },
            version = record.version,
            wordWrap = record.wordWrap,
            syntaxLanguage = record.syntaxLanguage,
            showPreview = record.showPreview,
            color = record.color,
            contentHash = record.contentHash,
            parentHash = record.parentHash,
            hashChain = hashChain,
            needsSync = record.needsSync,
        )
    }
}
