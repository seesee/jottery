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

    val filename: String,

    @ColumnInfo(name = "mime_type")
    val mimeType: String,

    val size: Int,

    // Encrypted blob
    val data: ByteArray,

    @ColumnInfo(name = "thumbnail_data")
    val thumbnailData: ByteArray? = null,
) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is AttachmentRecord) return false
        return id == other.id
    }

    override fun hashCode(): Int = id.hashCode()
}
