package com.jottery.android.database.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.jottery.android.model.NoteDeletion

@Dao
interface NoteDeletionDao {

    @Query("SELECT * FROM note_deletions")
    suspend fun listAll(): List<NoteDeletion>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertOrReplace(deletion: NoteDeletion)

    @Query("DELETE FROM note_deletions WHERE id = :id")
    suspend fun delete(id: String)

    @Query("DELETE FROM note_deletions")
    suspend fun deleteAll()
}
