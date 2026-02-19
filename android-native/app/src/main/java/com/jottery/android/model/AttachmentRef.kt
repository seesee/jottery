package com.jottery.android.model

import kotlinx.serialization.Serializable

/**
 * Reference to an encrypted attachment blob. Stored as JSON within notes/versions.
 * Not a Room entity — embedded in notes.attachments JSON column.
 */
@Serializable
data class AttachmentRef(
    val id: String,
    var filename: String,
    val mimeType: String,
    val size: Int,
    val data: String,
    val thumbnailData: String? = null,
)
