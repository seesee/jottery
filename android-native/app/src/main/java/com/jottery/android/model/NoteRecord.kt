package com.jottery.android.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey
import com.jottery.android.util.DateUtils

/**
 * On-disk note record with encrypted fields. Maps directly to the `notes` SQLite table.
 * Decryption happens in NoteRepository, not here.
 */
@Entity(tableName = "notes")
data class NoteRecord(
    @PrimaryKey
    val id: String,

    @ColumnInfo(name = "created_at")
    val createdAt: String,

    @ColumnInfo(name = "modified_at")
    var modifiedAt: String,

    @ColumnInfo(name = "synced_at")
    var syncedAt: String? = null,

    // Encrypted JSON: {"ciphertext":"...","iv":"..."}
    var content: String,

    // Encrypted JSON: {"ciphertext":"...","iv":"..."}
    var tags: String,

    // JSON array of attachment references
    var attachments: String = "[]",

    var pinned: Boolean = false,

    var archived: Boolean = false,

    @ColumnInfo(name = "archived_at")
    var archivedAt: String? = null,

    var locked: Boolean = false,

    @ColumnInfo(name = "locked_at")
    var lockedAt: String? = null,

    var deleted: Boolean = false,

    @ColumnInfo(name = "deleted_at")
    var deletedAt: String? = null,

    var version: Int = 1,

    @ColumnInfo(name = "word_wrap")
    var wordWrap: Boolean = true,

    @ColumnInfo(name = "syntax_language")
    var syntaxLanguage: String = "markdown",

    @ColumnInfo(name = "show_preview")
    var showPreview: Boolean = false,

    var color: String? = null,

    @ColumnInfo(name = "content_hash")
    var contentHash: String? = null,

    @ColumnInfo(name = "parent_hash")
    var parentHash: String? = null,

    @ColumnInfo(name = "hash_chain")
    var hashChain: String? = null,

    @ColumnInfo(name = "needs_sync")
    var needsSync: Boolean = true,

    @ColumnInfo(name = "sync_hash")
    var syncHash: String? = null,
) {
    companion object {
        fun new(encryptedContent: String, encryptedTags: String): NoteRecord {
            val now = DateUtils.nowISO8601()
            return NoteRecord(
                id = java.util.UUID.randomUUID().toString().lowercase(),
                createdAt = now,
                modifiedAt = now,
                content = encryptedContent,
                tags = encryptedTags,
                attachments = "[]",
                pinned = false,
                archived = false,
                locked = false,
                deleted = false,
                version = 0,
                wordWrap = true,
                syntaxLanguage = "markdown",
                showPreview = false,
                needsSync = true,
            )
        }
    }
}
