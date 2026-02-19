package com.jottery.android.database.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Update
import com.jottery.android.model.NoteRecord

@Dao
interface NoteDao {

    @Query("SELECT * FROM notes WHERE deleted = 0 AND archived = 0 ORDER BY modified_at DESC")
    suspend fun listActive(): List<NoteRecord>

    @Query("SELECT * FROM notes WHERE deleted = 1 ORDER BY deleted_at DESC")
    suspend fun listDeleted(): List<NoteRecord>

    @Query("SELECT * FROM notes WHERE archived = 1 AND deleted = 0 ORDER BY modified_at DESC")
    suspend fun listArchived(): List<NoteRecord>

    @Query("SELECT * FROM notes WHERE needs_sync = 1")
    suspend fun listNeedingSync(): List<NoteRecord>

    @Query("SELECT * FROM notes WHERE id = :id")
    suspend fun getById(id: String): NoteRecord?

    @Query("SELECT COUNT(*) FROM notes WHERE deleted = 0 AND archived = 0")
    suspend fun countActive(): Int

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertOrReplace(note: NoteRecord)

    @Update
    suspend fun update(note: NoteRecord)

    @Query("DELETE FROM notes WHERE id = :id")
    suspend fun hardDelete(id: String)

    @Query("""
        UPDATE notes SET deleted = 1, deleted_at = :deletedAt,
        modified_at = :modifiedAt, needs_sync = 1 WHERE id = :id
    """)
    suspend fun softDelete(id: String, deletedAt: String, modifiedAt: String)

    @Query("""
        UPDATE notes SET deleted = 0, deleted_at = NULL,
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

    @Query("""
        UPDATE notes SET needs_sync = 0, synced_at = :syncedAt WHERE id = :id
    """)
    suspend fun markSynced(id: String, syncedAt: String)

    @Query("""
        UPDATE notes SET needs_sync = 0, synced_at = :syncedAt, version = :version WHERE id = :id
    """)
    suspend fun markSyncedWithVersion(id: String, syncedAt: String, version: Int)

    @Query("SELECT id, attachments FROM notes")
    suspend fun listIdsAndAttachments(): List<NoteIdAttachments>

    @Query("DELETE FROM notes WHERE deleted = 1 AND deleted_at < :cutoff")
    suspend fun purgeOldDeleted(cutoff: String): Int
}

data class NoteIdAttachments(
    val id: String,
    val attachments: String,
)
