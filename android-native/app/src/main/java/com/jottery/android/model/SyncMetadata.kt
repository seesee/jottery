package com.jottery.android.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

/**
 * Sync state metadata. Singleton table (id=1).
 */
@Entity(tableName = "sync_metadata")
data class SyncMetadata(
    @PrimaryKey
    val id: Int = 1,

    @ColumnInfo(name = "last_sync_at")
    val lastSyncAt: String? = null,

    @ColumnInfo(name = "last_push_at")
    val lastPushAt: String? = null,

    @ColumnInfo(name = "last_pull_at")
    val lastPullAt: String? = null,

    @ColumnInfo(name = "api_key")
    val apiKey: String? = null,

    @ColumnInfo(name = "client_id")
    val clientId: String? = null,

    @ColumnInfo(name = "sync_enabled")
    val syncEnabled: Boolean = false,

    @ColumnInfo(name = "sync_endpoint")
    val syncEndpoint: String? = null,

    @ColumnInfo(name = "auto_sync_interval")
    val autoSyncInterval: Int? = 5,

    @ColumnInfo(name = "user_email")
    val userEmail: String? = null,

    @ColumnInfo(name = "pending_registration_email")
    val pendingRegistrationEmail: String? = null,

    @ColumnInfo(name = "pending_device_name")
    val pendingDeviceName: String? = null,

    @ColumnInfo(name = "user_id")
    val userId: String? = null,
)
