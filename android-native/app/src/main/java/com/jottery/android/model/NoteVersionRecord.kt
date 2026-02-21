package com.jottery.android.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.Index
import androidx.room.PrimaryKey
import com.jottery.android.util.DateUtils

/**
 * Version snapshot of a note. Primary key is composite "noteId:version".
 */
@Entity(
    tableName = "note_versions",
    indices = [
        Index(value = ["note_id"]),
        Index(value = ["note_id", "version"]),
    ]
)
data class NoteVersionRecord(
    @PrimaryKey
    @ColumnInfo(name = "version_key")
    val versionKey: String,

    @ColumnInfo(name = "note_id")
    val noteId: String,

    val version: Int,

    @ColumnInfo(name = "created_at")
    val createdAt: String,

    @ColumnInfo(name = "synced_at")
    val syncedAt: String,

    // Encrypted JSON
    val content: String,

    // Encrypted JSON
    val tags: String,

    // JSON array
    val attachments: String,

    @ColumnInfo(name = "syntax_language")
    val syntaxLanguage: String? = null,

    @ColumnInfo(name = "word_wrap")
    val wordWrap: Boolean? = null,

    @ColumnInfo(name = "show_preview")
    val showPreview: Boolean? = null,

    val color: String? = null,

    val reason: String,

    @ColumnInfo(name = "content_hash")
    val contentHash: String? = null,

    @ColumnInfo(name = "parent_hash")
    val parentHash: String? = null,
) {
    companion object {
        fun from(note: NoteRecord, reason: String): NoteVersionRecord {
            val key = "${note.id}:${note.version}"
            return NoteVersionRecord(
                versionKey = key,
                noteId = note.id,
                version = note.version,
                createdAt = note.modifiedAt,
                syncedAt = DateUtils.nowISO8601(),
                content = note.content,
                tags = note.tags,
                attachments = note.attachments,
                syntaxLanguage = note.syntaxLanguage,
                wordWrap = note.wordWrap,
                showPreview = note.showPreview,
                color = note.color,
                reason = reason,
                contentHash = note.contentHash,
                parentHash = note.parentHash,
            )
        }
    }
}
