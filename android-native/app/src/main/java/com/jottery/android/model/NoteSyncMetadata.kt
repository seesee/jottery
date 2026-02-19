package com.jottery.android.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.ForeignKey
import androidx.room.PrimaryKey

/**
 * Per-note sync tracking metadata.
 */
@Entity(
    tableName = "note_sync_metadata",
    foreignKeys = [
        ForeignKey(
            entity = NoteRecord::class,
            parentColumns = ["id"],
            childColumns = ["note_id"],
            onDelete = ForeignKey.CASCADE,
        )
    ]
)
data class NoteSyncMetadata(
    @PrimaryKey
    @ColumnInfo(name = "note_id")
    val noteId: String,

    @ColumnInfo(name = "synced_at")
    val syncedAt: String,

    @ColumnInfo(name = "sync_hash")
    val syncHash: String,

    @ColumnInfo(name = "server_version")
    val serverVersion: Int,

    @ColumnInfo(name = "last_sync_status")
    val lastSyncStatus: String = "pending",

    @ColumnInfo(name = "error_message")
    val errorMessage: String? = null,

    @ColumnInfo(name = "conflict_data")
    val conflictData: String? = null,
)
