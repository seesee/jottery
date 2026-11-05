package com.jottery.android.database.repository

import com.jottery.android.database.dao.NoteDeletionDao
import com.jottery.android.database.dao.NoteSyncMetadataDao
import com.jottery.android.database.dao.SyncMetadataDao
import com.jottery.android.model.NoteDeletion
import com.jottery.android.model.NoteSyncMetadata
import com.jottery.android.model.StoredConflict
import com.jottery.android.model.SyncMetadata
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

private const val CONFLICT_STATUS = "conflict"
private val conflictJson = Json { ignoreUnknownKeys = true; encodeDefaults = true }

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

    suspend fun savePullCursor(cursor: String) = syncMetadataDao.savePullCursor(cursor)

    suspend fun clearPullCursor() = syncMetadataDao.clearPullCursor()

    suspend fun markWrappedKeyOpened() = syncMetadataDao.markWrappedKeyOpened()

    suspend fun setPendingWrappedKey(blob: String?) = syncMetadataDao.setPendingWrappedKey(blob)

    suspend fun clearPendingWrappedKey(blob: String) = syncMetadataDao.clearPendingWrappedKey(blob)

    // MARK: - Note Sync Metadata (per-note)

    suspend fun getNoteSyncMetadata(noteId: String): NoteSyncMetadata? =
        noteSyncMetadataDao.getByNoteId(noteId)

    suspend fun saveNoteSyncMetadata(metadata: NoteSyncMetadata) =
        noteSyncMetadataDao.insertOrReplace(metadata)

    suspend fun deleteNoteSyncMetadata(noteId: String) =
        noteSyncMetadataDao.deleteByNoteId(noteId)

    suspend fun listConflicts(): List<NoteSyncMetadata> =
        noteSyncMetadataDao.listConflicts()

    // MARK: - Conflicts

    /** Conflicts awaiting a decision. Unreadable rows are dropped, not fatal. */
    suspend fun listStoredConflicts(): List<StoredConflict> =
        noteSyncMetadataDao.listConflicts().mapNotNull { row ->
            row.conflictData?.let {
                try {
                    conflictJson.decodeFromString<StoredConflict>(it)
                } catch (_: Exception) {
                    null
                }
            }
        }

    suspend fun saveConflict(conflict: StoredConflict) {
        noteSyncMetadataDao.insertOrReplace(
            NoteSyncMetadata(
                noteId = conflict.id,
                syncedAt = conflict.serverModifiedAt,
                syncHash = conflict.serverContentHash ?: "",
                serverVersion = conflict.serverVersion,
                lastSyncStatus = CONFLICT_STATUS,
                conflictData = conflictJson.encodeToString(conflict),
            )
        )
    }

    suspend fun clearConflict(noteId: String) = noteSyncMetadataDao.deleteByNoteId(noteId)

    // MARK: - Note Deletions

    suspend fun listDeletions(): List<NoteDeletion> = noteDeletionDao.listPending()

    suspend fun addDeletion(deletion: NoteDeletion) = noteDeletionDao.insertOrReplace(deletion)

    suspend fun removeDeletion(id: String) = noteDeletionDao.delete(id)

    suspend fun clearDeletions() = noteDeletionDao.deleteAll()
}
