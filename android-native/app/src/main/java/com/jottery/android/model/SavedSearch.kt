package com.jottery.android.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

/**
 * Encrypted saved search query.
 */
@Entity(tableName = "saved_searches")
data class SavedSearch(
    @PrimaryKey
    val id: String,

    // Encrypted
    val name: String,

    // Encrypted
    val query: String,

    @ColumnInfo(name = "display_order")
    val displayOrder: Int = 0,

    @ColumnInfo(name = "created_at")
    val createdAt: String,

    @ColumnInfo(name = "modified_at")
    val modifiedAt: String,

    @ColumnInfo(name = "synced_at")
    val syncedAt: String? = null,

    val deleted: Boolean = false,

    @ColumnInfo(name = "deleted_at")
    val deletedAt: String? = null,

    val version: Int = 1,

    @ColumnInfo(name = "needs_sync")
    val needsSync: Boolean = false,
)
