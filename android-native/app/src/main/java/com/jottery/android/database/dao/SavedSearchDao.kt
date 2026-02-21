package com.jottery.android.database.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.jottery.android.model.SavedSearch

@Dao
interface SavedSearchDao {

    @Query("SELECT * FROM saved_searches WHERE deleted = 0 ORDER BY display_order ASC")
    suspend fun listActive(): List<SavedSearch>

    @Query("SELECT * FROM saved_searches ORDER BY display_order ASC")
    suspend fun listAll(): List<SavedSearch>

    @Query("SELECT * FROM saved_searches WHERE id = :id")
    suspend fun getById(id: String): SavedSearch?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertOrReplace(savedSearch: SavedSearch)

    @Query("DELETE FROM saved_searches WHERE id = :id")
    suspend fun delete(id: String)

    @Query("""
        UPDATE saved_searches SET name = :name, query = :query, needs_sync = 1
        WHERE id = :id
    """)
    suspend fun updateEncrypted(id: String, name: String, query: String)

    @Query("SELECT * FROM saved_searches WHERE needs_sync = 1")
    suspend fun listNeedingSync(): List<SavedSearch>
}
