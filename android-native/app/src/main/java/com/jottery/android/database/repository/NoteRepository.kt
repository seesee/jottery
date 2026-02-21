package com.jottery.android.database.repository

import android.util.Log
import com.jottery.android.crypto.CryptoService
import com.jottery.android.database.dao.NoteDao
import com.jottery.android.database.dao.NoteVersionDao
import com.jottery.android.model.AttachmentRef
import com.jottery.android.model.DecryptedNote
import com.jottery.android.model.NoteRecord
import com.jottery.android.model.NoteVersionRecord
import com.jottery.android.model.SyncableNoteFields
import com.jottery.android.util.DateUtils
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import java.time.Duration
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
) {
    private val json = Json { ignoreUnknownKeys = true }

    companion object {
        private const val TAG = "NoteRepo"
        private val DEFAULT_PURGE_INTERVAL: Duration = Duration.ofDays(30)
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
    suspend fun listActiveWithStats(key: SecretKey): ListResult {
        val records = noteDao.listActive()
        var failCount = 0
        val decrypted = mutableListOf<DecryptedNote>()
        for (record in records) {
            try {
                decrypted.add(decrypt(record, key))
            } catch (e: Exception) {
                failCount++
                if (failCount <= 3) {
                    Log.w(TAG, "Decrypt failed for note ${record.id}: ${e.javaClass.simpleName}: ${e.message}")
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
        val records = noteDao.listDeleted()
        return records.mapNotNull { record ->
            try {
                decrypt(record, key)
            } catch (_: Exception) {
                null
            }
        }
    }

    /** Fetch all archived notes, decrypted. */
    suspend fun listArchived(key: SecretKey): List<DecryptedNote> {
        val records = noteDao.listArchived()
        return records.mapNotNull { record ->
            try {
                decrypt(record, key)
            } catch (_: Exception) {
                null
            }
        }
    }

    /** Fetch a single note by ID, decrypted. */
    suspend fun get(id: String, key: SecretKey): DecryptedNote? {
        val record = noteDao.getById(id) ?: return null
        return decrypt(record, key)
    }

    /** Fetch a raw (encrypted) record by ID. */
    suspend fun getRaw(id: String): NoteRecord? {
        return noteDao.getById(id)
    }

    /** Fetch all records that need syncing. */
    suspend fun listNeedingSync(): List<NoteRecord> {
        return noteDao.listNeedingSync()
    }

    /** Fetch all raw records (for backup). */
    suspend fun listAllRaw(): List<NoteRecord> {
        return noteDao.listAll()
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
    ): DecryptedNote {
        val encContent = CryptoService.encryptText(content, key)
        val encTags = CryptoService.encryptStringArray(tags, key)

        val contentJSON = CryptoService.serializeEncryptedJSON(encContent)
        val tagsJSON = CryptoService.serializeEncryptedJSON(encTags)

        var record = NoteRecord.new(
            encryptedContent = contentJSON,
            encryptedTags = tagsJSON,
        )

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

    // MARK: - Update

    /** Update a decrypted note's content and tags back to the database. */
    suspend fun update(note: DecryptedNote, key: SecretKey) {
        val encContent = CryptoService.encryptText(note.content, key)
        val encTags = CryptoService.encryptStringArray(note.tags, key)

        val contentJSON = CryptoService.serializeEncryptedJSON(encContent)
        val tagsJSON = CryptoService.serializeEncryptedJSON(encTags)

        val now = DateUtils.nowISO8601()
        val attachmentsString = json.encodeToString(note.attachments)

        // Fetch the current DB record so we can read the previous hash and version
        val currentRecord = getRaw(note.id)
        val previousHash = currentRecord?.contentHash
        val previousChain: List<String> = if (currentRecord?.hashChain != null) {
            try {
                json.decodeFromString(currentRecord.hashChain!!)
            } catch (_: Exception) {
                emptyList()
            }
        } else {
            emptyList()
        }

        // Compute content hash using sync-format tags
        val syncTags = encryptTagsToSyncFormat(note.tags, key)
        val contentHash = CryptoService.computeContentHash(
            SyncableNoteFields(
                content = contentJSON,
                tags = syncTags,
                attachmentIds = note.attachments.map { it.id }.sorted(),
                pinned = note.pinned,
                archived = note.archived,
                locked = note.locked,
                syntaxLanguage = note.syntaxLanguage,
                wordWrap = note.wordWrap,
                showPreview = note.showPreview,
                color = note.color,
            )
        )
        val parentHash = previousHash
        val hashChain = CryptoService.updateHashChain(contentHash, previousChain)
        val hashChainStr = json.encodeToString(hashChain)

        // First-edit snapshot: when version is 0, create a snapshot of the original state
        // then advance version to 1
        val currentVersion = currentRecord?.version ?: note.version
        val newVersion: Int
        if (currentVersion == 0 && currentRecord != null) {
            val versionSnapshot = NoteVersionRecord.from(currentRecord, "manual-sync")
            noteVersionDao.insertOrReplace(versionSnapshot)
            newVersion = 1
        } else {
            newVersion = currentVersion
        }

        // Build the updated record
        val updatedRecord = (currentRecord ?: NoteRecord(
            id = note.id,
            createdAt = DateUtils.formatISO8601(note.createdAt),
            modifiedAt = now,
            content = contentJSON,
            tags = tagsJSON,
        )).copy(
            content = contentJSON,
            tags = tagsJSON,
            modifiedAt = now,
            attachments = attachmentsString,
            pinned = note.pinned,
            archived = note.archived,
            archivedAt = note.archivedAt?.let { DateUtils.formatISO8601(it) },
            locked = note.locked,
            lockedAt = note.lockedAt?.let { DateUtils.formatISO8601(it) },
            deleted = note.deleted,
            deletedAt = note.deletedAt?.let { DateUtils.formatISO8601(it) },
            version = newVersion,
            wordWrap = note.wordWrap,
            syntaxLanguage = note.syntaxLanguage,
            showPreview = note.showPreview,
            color = note.color,
            contentHash = contentHash,
            parentHash = parentHash,
            hashChain = hashChainStr,
            needsSync = true,
        )

        noteDao.update(updatedRecord)
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

    /** Hard-delete a note (permanent). */
    suspend fun hardDelete(id: String) {
        noteDao.hardDelete(id)
    }

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
        val record = noteDao.getById(id) ?: return
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
        val record = noteDao.getById(id) ?: return
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

    /** Mark note as synced, optionally updating version from server. */
    suspend fun markSynced(id: String, syncedAt: String, serverVersion: Int? = null) {
        if (serverVersion != null) {
            noteDao.markSyncedWithVersion(id, syncedAt, serverVersion)
        } else {
            noteDao.markSynced(id, syncedAt)
        }
    }

    // MARK: - Attachment Management

    /** Add an attachment reference to a note's attachments JSON array. */
    suspend fun addAttachment(noteId: String, ref: AttachmentRef) {
        val record = noteDao.getById(noteId) ?: return
        val attachments: MutableList<AttachmentRef> = try {
            json.decodeFromString<List<AttachmentRef>>(record.attachments).toMutableList()
        } catch (_: Exception) {
            mutableListOf()
        }
        attachments.add(ref)
        val updated = record.copy(
            attachments = json.encodeToString(attachments),
            modifiedAt = DateUtils.nowISO8601(),
            needsSync = true,
        )
        noteDao.update(updated)
    }

    /** Remove an attachment reference from a note's attachments JSON array. */
    suspend fun removeAttachment(noteId: String, attachmentId: String) {
        val record = noteDao.getById(noteId) ?: return
        val attachments: MutableList<AttachmentRef> = try {
            json.decodeFromString<List<AttachmentRef>>(record.attachments).toMutableList()
        } catch (_: Exception) {
            mutableListOf()
        }
        attachments.removeAll { it.id == attachmentId }
        val updated = record.copy(
            attachments = json.encodeToString(attachments),
            modifiedAt = DateUtils.nowISO8601(),
            needsSync = true,
        )
        noteDao.update(updated)
    }

    // MARK: - Insert Raw (for sync pull)

    /** Insert or replace a raw record (from server). */
    suspend fun insertOrReplace(record: NoteRecord) {
        noteDao.insertOrReplace(record)
    }

    // MARK: - Purge

    /**
     * Permanently delete notes that have been in the recycle bin longer than the given interval.
     * Returns the number of notes purged.
     */
    suspend fun purgeOldDeletedNotes(
        olderThan: Duration = DEFAULT_PURGE_INTERVAL,
    ): Int {
        val cutoff = Instant.now().minus(olderThan)
        val cutoffStr = DateUtils.formatISO8601(cutoff)
        return noteDao.purgeOldDeleted(cutoffStr)
    }

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

        // Decrypt attachment filenames (stored as encrypted JSON strings)
        val attachments = rawAttachments.map { ref ->
            try {
                val enc = CryptoService.parseEncryptedJSON(ref.filename)
                val decryptedFilename = CryptoService.decryptText(enc, key)
                ref.copy(filename = decryptedFilename)
            } catch (_: Exception) {
                ref
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
            decryptedAt = Instant.now(),
        )
    }
}
