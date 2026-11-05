package com.jottery.android.database.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Update
import com.jottery.android.model.NoteRecord

/**
 * Android reads rows through a 2 MB CursorWindow, and one row larger than that
 * makes the whole query throw. Content that could approach the limit comes back
 * empty from these queries and is read separately, in chunks, by the repository.
 * Real content is never empty: it is at least the JSON of an encrypted value.
 */
const val INLINE_CONTENT_LIMIT = 512 * 1024

private const val NOTE_COLUMNS = """id, created_at, modified_at, synced_at,
    CASE WHEN length(content) > $INLINE_CONTENT_LIMIT THEN '' ELSE content END AS content,
    tags, attachments, pinned, archived, archived_at, locked, locked_at, deleted, deleted_at,
    version, word_wrap, syntax_language, show_preview, color, content_hash, parent_hash,
    hash_chain, needs_sync, sync_hash, local_deleted_at"""

@Dao
interface NoteDao {

    @Query("SELECT $NOTE_COLUMNS FROM notes WHERE deleted = 0 AND archived = 0 ORDER BY modified_at DESC")
    suspend fun listActive(): List<NoteRecord>

    @Query("SELECT $NOTE_COLUMNS FROM notes WHERE deleted = 1 ORDER BY deleted_at DESC")
    suspend fun listDeleted(): List<NoteRecord>

    @Query("SELECT $NOTE_COLUMNS FROM notes WHERE archived = 1 AND deleted = 0 ORDER BY modified_at DESC")
    suspend fun listArchived(): List<NoteRecord>

    @Query("SELECT $NOTE_COLUMNS FROM notes WHERE needs_sync = 1")
    suspend fun listNeedingSync(): List<NoteRecord>

    @Query("SELECT $NOTE_COLUMNS FROM notes WHERE id = :id")
    suspend fun getById(id: String): NoteRecord?

    @Query("SELECT length(content) FROM notes WHERE id = :id")
    suspend fun contentLength(id: String): Int?

    /** [start] counts from 1, as SQLite's substr does. */
    @Query("SELECT substr(content, :start, :length) FROM notes WHERE id = :id")
    suspend fun contentChunk(id: String, start: Int, length: Int): String?

    @Query("SELECT COUNT(*) FROM notes WHERE deleted = 0 AND archived = 0")
    suspend fun countActive(): Int

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertOrReplace(note: NoteRecord)

    @Update
    suspend fun update(note: NoteRecord)

    @Query("DELETE FROM notes WHERE id = :id")
    suspend fun hardDelete(id: String)

    @Query("""
        UPDATE notes SET deleted = 1, deleted_at = :deletedAt, local_deleted_at = :deletedAt,
        modified_at = :modifiedAt, needs_sync = 1 WHERE id = :id
    """)
    suspend fun softDelete(id: String, deletedAt: String, modifiedAt: String)

    /** A deletion from the server: into the bin, with nothing left to send. */
    @Query("""
        UPDATE notes SET deleted = 1, deleted_at = :deletedAt, local_deleted_at = :localDeletedAt,
        needs_sync = 0 WHERE id = :id
    """)
    suspend fun moveToBinForServer(id: String, deletedAt: String, localDeletedAt: String)

    /**
     * As above, unless the note has changed since it was pushed. A note already in the
     * bin keeps its dates. A live one starts afresh, as a date left on it from an
     * earlier stay would let a server send it through the bin at once.
     */
    @Query("""
        UPDATE notes SET
        deleted_at = CASE WHEN deleted = 1 THEN COALESCE(deleted_at, :now) ELSE :now END,
        local_deleted_at = CASE WHEN deleted = 1 THEN COALESCE(local_deleted_at, :now) ELSE :now END,
        deleted = 1, needs_sync = 0
        WHERE id = :id AND modified_at = :pushedModifiedAt
    """)
    suspend fun moveToBinIfUnchanged(id: String, pushedModifiedAt: String, now: String): Int

    @Query("""
        UPDATE notes SET deleted = 0, deleted_at = NULL, local_deleted_at = NULL,
        modified_at = :modifiedAt, needs_sync = 1 WHERE id = :id
    """)
    suspend fun restore(id: String, modifiedAt: String)

    @Query("""
        UPDATE notes SET archived = 1, archived_at = :archivedAt,
        modified_at = :modifiedAt, needs_sync = 1 WHERE id = :id
    """)
    suspend fun archive(id: String, archivedAt: String, modifiedAt: String)

    @Query("""
        UPDATE notes SET archived = 0, archived_at = NULL,
        modified_at = :modifiedAt, needs_sync = 1 WHERE id = :id
    """)
    suspend fun unarchive(id: String, modifiedAt: String)

    @Query("UPDATE notes SET needs_sync = 1 WHERE id = :id")
    suspend fun requeue(id: String)

    @Query("UPDATE notes SET sync_hash = :hash WHERE id = :id")
    suspend fun setSyncHash(id: String, hash: String)

    /**
     * Clear needs_sync only if the row still has the modified_at that was pushed.
     * An edit saved while the request was in flight has a newer modified_at, so
     * it keeps its flag and goes out on the next push.
     */
    @Query("""
        UPDATE notes SET needs_sync = 0, synced_at = :syncedAt
        WHERE id = :id AND modified_at = :pushedModifiedAt
    """)
    suspend fun markSyncedIfUnchanged(id: String, syncedAt: String, pushedModifiedAt: String): Int

    @Query("""
        UPDATE notes SET needs_sync = 0, synced_at = :syncedAt, version = :version
        WHERE id = :id AND modified_at = :pushedModifiedAt
    """)
    suspend fun markSyncedWithVersionIfUnchanged(
        id: String,
        syncedAt: String,
        version: Int,
        pushedModifiedAt: String,
    ): Int

    @Query("SELECT id, attachments FROM notes")
    suspend fun listIdsAndAttachments(): List<NoteIdAttachments>

    @Query("SELECT $NOTE_COLUMNS FROM notes")
    suspend fun listAll(): List<NoteRecord>

    @Query("""
        SELECT id, deleted_at AS deletedAt FROM notes
        WHERE deleted = 1 AND local_deleted_at < :cutoff
    """)
    suspend fun listExpiredDeleted(cutoff: String): List<ExpiredNote>

    /** A note in the bin with no date of this device's own gets one. deleted_at can be the server's. */
    @Query("UPDATE notes SET local_deleted_at = :now WHERE deleted = 1 AND local_deleted_at IS NULL")
    suspend fun startBinClocks(now: String)
}

data class ExpiredNote(
    val id: String,
    val deletedAt: String?,
)

data class NoteIdAttachments(
    val id: String,
    val attachments: String,
)
