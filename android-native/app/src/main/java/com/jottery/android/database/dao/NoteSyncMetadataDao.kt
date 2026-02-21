package com.jottery.android.database.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.jottery.android.model.NoteSyncMetadata

@Dao
interface NoteSyncMetadataDao {

    @Query("SELECT * FROM note_sync_metadata WHERE note_id = :noteId")
    suspend fun getByNoteId(noteId: String): NoteSyncMetadata?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertOrReplace(metadata: NoteSyncMetadata)

    @Query("DELETE FROM note_sync_metadata WHERE note_id = :noteId")
    suspend fun deleteByNoteId(noteId: String)

    @Query("SELECT * FROM note_sync_metadata WHERE last_sync_status = 'conflict'")
    suspend fun listConflicts(): List<NoteSyncMetadata>
}
