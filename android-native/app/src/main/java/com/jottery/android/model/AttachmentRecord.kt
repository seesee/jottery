package com.jottery.android.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

/**
 * Encrypted attachment blob storage.
 */
@Entity(tableName = "attachments")
data class AttachmentRecord(
    @PrimaryKey
    val id: String,

    // Always empty: the name lives, encrypted, in the note's attachment reference
    val filename: String,

    @ColumnInfo(name = "mime_type")
    val mimeType: String,

    val size: Int,

    // Encrypted blob
    val data: ByteArray,

    @ColumnInfo(name = "thumbnail_data")
    val thumbnailData: ByteArray? = null,

    // The server holds this blob, so a push needn't send it again
    @ColumnInfo(defaultValue = "0")
    val uploaded: Boolean = false,
) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is AttachmentRecord) return false
        return id == other.id
    }

    override fun hashCode(): Int = id.hashCode()
}
