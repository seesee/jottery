package com.jottery.android.database.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.jottery.android.model.NoteDeletion

@Dao
interface NoteDeletionDao {

    /**
     * Deletions still to send. A permanent deletion removes the note, so one whose
     * note exists again is stale: the note came back, from a backup or an import.
     * Sending it would delete the restored copy, which is older, on every device.
     */
    @Query("SELECT * FROM note_deletions WHERE id NOT IN (SELECT id FROM notes)")
    suspend fun listPending(): List<NoteDeletion>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertOrReplace(deletion: NoteDeletion)

    @Query("DELETE FROM note_deletions WHERE id = :id")
    suspend fun delete(id: String)

    @Query("DELETE FROM note_deletions")
    suspend fun deleteAll()
}
