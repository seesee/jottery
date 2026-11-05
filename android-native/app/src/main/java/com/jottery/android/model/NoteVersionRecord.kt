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
        private const val KEY_HASH_LENGTH = 16

        /**
         * A snapshot of an accepted state. Clients count versions differently, so two
         * devices can both reach version 5 with different text, and the server keeps
         * only the first it hears of. The hash keeps them apart, while the same text
         * still gets the same key. Nothing parses a key, so older clients just store it.
         */
        fun from(note: NoteRecord, reason: String): NoteVersionRecord {
            val hash = note.contentHash?.take(KEY_HASH_LENGTH)
            return from(note, reason, listOfNotNull(note.id, note.version, hash).joinToString(":"))
        }

        /**
         * A local safety snapshot taken before an overwrite. It has its own key,
         * so it can never replace another snapshot of the same version.
         */
        fun safetySnapshot(note: NoteRecord, reason: String): NoteVersionRecord =
            from(note, reason, "${note.id}:${note.version}:${System.currentTimeMillis()}")

        private fun from(note: NoteRecord, reason: String, key: String): NoteVersionRecord {
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
