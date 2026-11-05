package com.jottery.android.database.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.jottery.android.model.NoteVersionRecord

/** See [INLINE_CONTENT_LIMIT]: a snapshot can be as large as its note. */
private const val VERSION_COLUMNS = """version_key, note_id, version, created_at, synced_at,
    CASE WHEN length(content) > $INLINE_CONTENT_LIMIT THEN '' ELSE content END AS content,
    tags, attachments, syntax_language, word_wrap, show_preview, color, reason,
    content_hash, parent_hash"""

@Dao
interface NoteVersionDao {

    @Query("SELECT $VERSION_COLUMNS FROM note_versions WHERE note_id = :noteId ORDER BY version DESC")
    suspend fun getVersions(noteId: String): List<NoteVersionRecord>

    @Query("SELECT $VERSION_COLUMNS FROM note_versions")
    suspend fun listAll(): List<NoteVersionRecord>

    @Query("SELECT $VERSION_COLUMNS FROM note_versions WHERE version_key = :versionKey")
    suspend fun getByKey(versionKey: String): NoteVersionRecord?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertOrReplace(version: NoteVersionRecord)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertOrReplaceAll(versions: List<NoteVersionRecord>)

    @Query("DELETE FROM note_versions WHERE note_id = :noteId")
    suspend fun deleteByNoteId(noteId: String)

    @Query("SELECT attachments FROM note_versions WHERE note_id = :noteId AND attachments != '[]'")
    suspend fun listAttachments(noteId: String): List<String>

    @Query("SELECT attachments FROM note_versions WHERE attachments != '[]'")
    suspend fun listAllAttachments(): List<String>

    @Query("SELECT length(content) FROM note_versions WHERE version_key = :versionKey")
    suspend fun contentLength(versionKey: String): Int?

    /** [start] counts from 1, as SQLite's substr does. */
    @Query("SELECT substr(content, :start, :length) FROM note_versions WHERE version_key = :versionKey")
    suspend fun contentChunk(versionKey: String, start: Int, length: Int): String?
}
