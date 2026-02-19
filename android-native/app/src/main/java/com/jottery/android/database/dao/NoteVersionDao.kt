package com.jottery.android.database.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.jottery.android.model.NoteVersionRecord

@Dao
interface NoteVersionDao {

    @Query("SELECT * FROM note_versions WHERE note_id = :noteId ORDER BY version DESC")
    suspend fun getVersions(noteId: String): List<NoteVersionRecord>

    @Query("SELECT * FROM note_versions WHERE version_key = :versionKey")
    suspend fun getByKey(versionKey: String): NoteVersionRecord?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertOrReplace(version: NoteVersionRecord)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertOrReplaceAll(versions: List<NoteVersionRecord>)

    @Query("DELETE FROM note_versions WHERE note_id = :noteId")
    suspend fun deleteByNoteId(noteId: String)

    @Query("SELECT * FROM note_versions WHERE note_id IN (SELECT id FROM notes WHERE needs_sync = 1)")
    suspend fun listNeedingSync(): List<NoteVersionRecord>
}
