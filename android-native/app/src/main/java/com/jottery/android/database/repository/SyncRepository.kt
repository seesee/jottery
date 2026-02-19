package com.jottery.android.database.repository

import com.jottery.android.database.dao.NoteDeletionDao
import com.jottery.android.database.dao.NoteSyncMetadataDao
import com.jottery.android.database.dao.SyncMetadataDao
import com.jottery.android.model.NoteDeletion
import com.jottery.android.model.NoteSyncMetadata
import com.jottery.android.model.SyncMetadata

/**
 * Repository for sync state metadata.
 */
class SyncRepository(
    private val syncMetadataDao: SyncMetadataDao,
    private val noteSyncMetadataDao: NoteSyncMetadataDao,
    private val noteDeletionDao: NoteDeletionDao,
) {
    // MARK: - Sync Metadata (global)

    suspend fun getSyncMetadata(): SyncMetadata? = syncMetadataDao.get()

    suspend fun saveSyncMetadata(metadata: SyncMetadata) = syncMetadataDao.insertOrReplace(metadata)

    suspend fun updateLastSyncAt(syncedAt: String) = syncMetadataDao.updateLastSyncAt(syncedAt)

    suspend fun updateLastPushAt(pushedAt: String) = syncMetadataDao.updateLastPushAt(pushedAt)

    suspend fun updateLastPullAt(pulledAt: String) = syncMetadataDao.updateLastPullAt(pulledAt)

    // MARK: - Note Sync Metadata (per-note)

    suspend fun getNoteSyncMetadata(noteId: String): NoteSyncMetadata? =
        noteSyncMetadataDao.getByNoteId(noteId)

    suspend fun saveNoteSyncMetadata(metadata: NoteSyncMetadata) =
        noteSyncMetadataDao.insertOrReplace(metadata)

    suspend fun deleteNoteSyncMetadata(noteId: String) =
        noteSyncMetadataDao.deleteByNoteId(noteId)

    suspend fun listConflicts(): List<NoteSyncMetadata> =
        noteSyncMetadataDao.listConflicts()

    // MARK: - Note Deletions

    suspend fun listDeletions(): List<NoteDeletion> = noteDeletionDao.listAll()

    suspend fun addDeletion(deletion: NoteDeletion) = noteDeletionDao.insertOrReplace(deletion)

    suspend fun removeDeletion(id: String) = noteDeletionDao.delete(id)

    suspend fun clearDeletions() = noteDeletionDao.deleteAll()
}
