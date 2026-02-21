package com.jottery.android.database.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.jottery.android.model.SyncMetadata

@Dao
interface SyncMetadataDao {

    @Query("SELECT * FROM sync_metadata WHERE id = 1")
    suspend fun get(): SyncMetadata?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertOrReplace(metadata: SyncMetadata)

    @Query("UPDATE sync_metadata SET last_sync_at = :syncedAt WHERE id = 1")
    suspend fun updateLastSyncAt(syncedAt: String)

    @Query("UPDATE sync_metadata SET last_push_at = :pushedAt WHERE id = 1")
    suspend fun updateLastPushAt(pushedAt: String)

    @Query("UPDATE sync_metadata SET last_pull_at = :pulledAt WHERE id = 1")
    suspend fun updateLastPullAt(pulledAt: String)
}
