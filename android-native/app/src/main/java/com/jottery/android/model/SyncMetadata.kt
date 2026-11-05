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

    /**
     * This device has opened or uploaded the account's wrapped key, so that key
     * isn't in the old account-password format. If it later stops opening, the
     * password changed on another device, and it must not be overwritten (D1).
     */
    @ColumnInfo(name = "wrapped_key_opened", defaultValue = "0")
    val wrappedKeyOpened: Boolean = false,

    /**
     * The account's key rewrapped for a password change here that hasn't reached
     * the server yet. Wrapped like the server's copy, so it's safe on disk. It's
     * kept until uploaded, because only this device can tell that the server's
     * copy is out of date (jottery-rkg9).
     */
    @ColumnInfo(name = "pending_wrapped_key")
    val pendingWrappedKey: String? = null,

    /** The server's opaque position in this account's changes. Never parsed. */
    @ColumnInfo(name = "pull_cursor")
    val pullCursor: String? = null,

    /**
     * The one-off full pull has been done since this server started offering a
     * cursor. Timestamp pulls can skip changes silently, and that repairs it.
     */
    @ColumnInfo(name = "cursor_adopted", defaultValue = "0")
    val cursorAdopted: Boolean = false,
)
