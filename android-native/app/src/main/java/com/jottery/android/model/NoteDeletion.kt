package com.jottery.android.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

/**
 * Tracks note deletions for sync. When a note is permanently deleted,
 * a record here tells the sync system to propagate the deletion.
 */
@Entity(tableName = "note_deletions")
data class NoteDeletion(
    @PrimaryKey
    val id: String,

    @ColumnInfo(name = "deleted_at")
    val deletedAt: String,
)
