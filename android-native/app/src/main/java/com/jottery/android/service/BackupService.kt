package com.jottery.android.service

import android.util.Base64
import android.util.Log
import com.jottery.android.crypto.CryptoService
import com.jottery.android.database.repository.AttachmentRepository
import com.jottery.android.database.repository.NoteRepository
import com.jottery.android.database.repository.SavedSearchRepository
import com.jottery.android.database.repository.VersionRepository
import com.jottery.android.model.AttachmentRef
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import javax.crypto.SecretKey

/**
 * JWE v3 compatible backup/restore.
 *
 * Backup format:
 * - Version 3 JSON structure with encrypted notes, versions, attachments, and saved searches
 * - Compatible with iOS backup format for cross-platform restore
 */
class BackupService(
    private val noteRepo: NoteRepository,
    private val versionRepo: VersionRepository,
    private val attachmentRepo: AttachmentRepository,
    private val savedSearchRepo: SavedSearchRepository?,
) {
    companion object {
        private const val TAG = "BackupService"
        private const val BACKUP_VERSION = 3
    }

    private val json = Json { ignoreUnknownKeys = true; encodeDefaults = true; prettyPrint = true }

    @Serializable
    data class BackupPayload(
        val version: Int = BACKUP_VERSION,
        val createdAt: String,
        val notes: List<BackupNote>,
        val versions: List<BackupVersion> = emptyList(),
        val attachments: List<BackupAttachment> = emptyList(),
        val savedSearches: List<BackupSavedSearch> = emptyList(),
    )

    @Serializable
    data class BackupNote(
        val id: String,
        val createdAt: String,
        val modifiedAt: String,
        val content: String,
        val tags: String,
        val attachments: String,
        val pinned: Boolean,
        val archived: Boolean,
        val archivedAt: String? = null,
        val locked: Boolean = false,
        val lockedAt: String? = null,
        val deleted: Boolean,
        val deletedAt: String? = null,
        val version: Int,
        val wordWrap: Boolean = true,
        val syntaxLanguage: String = "markdown",
        val showPreview: Boolean = false,
        val color: String? = null,
        val contentHash: String? = null,
        val parentHash: String? = null,
        val hashChain: String? = null,
    )

    @Serializable
    data class BackupVersion(
        val versionKey: String,
        val noteId: String,
        val version: Int,
        val createdAt: String,
        val syncedAt: String? = null,
        val content: String,
        val tags: String,
        val attachments: String,
        val syntaxLanguage: String? = null,
        val wordWrap: Boolean? = null,
        val showPreview: Boolean? = null,
        val color: String? = null,
        val reason: String? = null,
        val contentHash: String? = null,
        val parentHash: String? = null,
    )

    @Serializable
    data class BackupAttachment(
        val id: String,
        val data: String, // Base64-encoded encrypted blob
    )

    @Serializable
    data class BackupSavedSearch(
        val id: String,
        val name: String,
        val query: String,
        val createdAt: String,
    )

    /**
     * Create a backup containing all notes, versions, and attachments.
     * The data stays encrypted (no decryption needed).
     * Returns the JSON string.
     */
    suspend fun createBackup(): String {
        val allNotes = noteRepo.listAllRaw()
        val backupNotes = allNotes.map { record ->
            BackupNote(
                id = record.id,
                createdAt = record.createdAt,
                modifiedAt = record.modifiedAt,
                content = record.content,
                tags = record.tags,
                attachments = record.attachments,
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
                hashChain = record.hashChain,
            )
        }

        val allVersions = mutableListOf<BackupVersion>()
        for (note in allNotes) {
            val versions = versionRepo.getVersions(note.id)
            for (v in versions) {
                allVersions.add(
                    BackupVersion(
                        versionKey = v.versionKey,
                        noteId = v.noteId,
                        version = v.version,
                        createdAt = v.createdAt,
                        syncedAt = v.syncedAt,
                        content = v.content,
                        tags = v.tags,
                        attachments = v.attachments,
                        syntaxLanguage = v.syntaxLanguage,
                        wordWrap = v.wordWrap,
                        showPreview = v.showPreview,
                        color = v.color,
                        reason = v.reason,
                        contentHash = v.contentHash,
                        parentHash = v.parentHash,
                    )
                )
            }
        }

        val backupAttachments = mutableListOf<BackupAttachment>()
        val blobIds = attachmentRepo.listBlobIds()
        for (id in blobIds) {
            val blob = attachmentRepo.getBlob(id)
            if (blob != null) {
                backupAttachments.add(
                    BackupAttachment(
                        id = id,
                        data = Base64.encodeToString(blob, Base64.NO_WRAP),
                    )
                )
            }
        }

        val backupSearches = mutableListOf<BackupSavedSearch>()
        savedSearchRepo?.let { repo ->
            val searches = repo.listAllRaw()
            for (s in searches) {
                backupSearches.add(
                    BackupSavedSearch(
                        id = s.id,
                        name = s.name,
                        query = s.query,
                        createdAt = s.createdAt,
                    )
                )
            }
        }

        val payload = BackupPayload(
            createdAt = com.jottery.android.util.DateUtils.nowISO8601(),
            notes = backupNotes,
            versions = allVersions,
            attachments = backupAttachments,
            savedSearches = backupSearches,
        )

        return json.encodeToString(payload)
    }

    /**
     * Restore a backup. Inserts all notes, versions, and attachments.
     * Returns the number of notes restored.
     */
    suspend fun restoreBackup(backupJson: String): Int {
        val payload: BackupPayload = json.decodeFromString(backupJson)

        Log.d(TAG, "Restoring backup v${payload.version}: ${payload.notes.size} notes, " +
            "${payload.versions.size} versions, ${payload.attachments.size} attachments")

        // Restore notes
        for (backupNote in payload.notes) {
            val record = com.jottery.android.model.NoteRecord(
                id = backupNote.id,
                createdAt = backupNote.createdAt,
                modifiedAt = backupNote.modifiedAt,
                content = backupNote.content,
                tags = backupNote.tags,
                attachments = backupNote.attachments,
                pinned = backupNote.pinned,
                archived = backupNote.archived,
                archivedAt = backupNote.archivedAt,
                locked = backupNote.locked,
                lockedAt = backupNote.lockedAt,
                deleted = backupNote.deleted,
                deletedAt = backupNote.deletedAt,
                version = backupNote.version,
                wordWrap = backupNote.wordWrap,
                syntaxLanguage = backupNote.syntaxLanguage,
                showPreview = backupNote.showPreview,
                color = backupNote.color,
                contentHash = backupNote.contentHash,
                parentHash = backupNote.parentHash,
                hashChain = backupNote.hashChain,
                needsSync = false,
            )
            noteRepo.insertOrReplace(record)
        }

        // Restore versions
        for (backupVer in payload.versions) {
            val record = com.jottery.android.model.NoteVersionRecord(
                versionKey = backupVer.versionKey,
                noteId = backupVer.noteId,
                version = backupVer.version,
                createdAt = backupVer.createdAt,
                syncedAt = backupVer.syncedAt,
                content = backupVer.content,
                tags = backupVer.tags,
                attachments = backupVer.attachments,
                syntaxLanguage = backupVer.syntaxLanguage,
                wordWrap = backupVer.wordWrap,
                showPreview = backupVer.showPreview,
                color = backupVer.color,
                reason = backupVer.reason,
                contentHash = backupVer.contentHash,
                parentHash = backupVer.parentHash,
            )
            versionRepo.insertOrReplace(record)
        }

        // Restore attachments
        for (backupAtt in payload.attachments) {
            val data = Base64.decode(backupAtt.data, Base64.NO_WRAP)
            attachmentRepo.insertOrReplace(
                com.jottery.android.model.AttachmentRecord(
                    id = backupAtt.id,
                    filename = "",
                    mimeType = "",
                    size = data.size,
                    data = data,
                )
            )
        }

        // Restore saved searches
        savedSearchRepo?.let { repo ->
            for (backupSearch in payload.savedSearches) {
                val record = com.jottery.android.model.SavedSearch(
                    id = backupSearch.id,
                    name = backupSearch.name,
                    query = backupSearch.query,
                    createdAt = backupSearch.createdAt,
                    needsSync = false,
                )
                repo.insertOrReplace(record)
            }
        }

        return payload.notes.size
    }
}
