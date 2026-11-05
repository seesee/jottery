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

    @Query("UPDATE sync_metadata SET pull_cursor = :cursor, cursor_adopted = 1 WHERE id = 1")
    suspend fun savePullCursor(cursor: String)

    @Query("UPDATE sync_metadata SET pull_cursor = NULL WHERE id = 1")
    suspend fun clearPullCursor()

    @Query("UPDATE sync_metadata SET wrapped_key_opened = 1 WHERE id = 1")
    suspend fun markWrappedKeyOpened()

    @Query("UPDATE sync_metadata SET pending_wrapped_key = :blob WHERE id = 1")
    suspend fun setPendingWrappedKey(blob: String?)

    /** Only if nothing newer has been queued while the upload was in flight. */
    @Query("UPDATE sync_metadata SET pending_wrapped_key = NULL WHERE id = 1 AND pending_wrapped_key = :blob")
    suspend fun clearPendingWrappedKey(blob: String)
}
