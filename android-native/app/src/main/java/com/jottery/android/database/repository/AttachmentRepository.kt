package com.jottery.android.database.repository

import com.jottery.android.database.dao.AttachmentDao
import com.jottery.android.model.AttachmentRecord

private const val READ_CHUNK = 1024 * 1024

/**
 * Repository for encrypted attachment blob storage.
 */
class AttachmentRepository(
    private val dao: AttachmentDao,
) {
    suspend fun getBlob(id: String): ByteArray? {
        val length = dao.blobLength(id) ?: return null
        val blob = ByteArray(length)
        var offset = 0
        while (offset < length) {
            val chunk = dao.blobChunk(id, offset + 1, READ_CHUNK) ?: return null
            if (chunk.isEmpty()) return null
            chunk.copyInto(blob, offset)
            offset += chunk.size
        }
        return blob
    }

    suspend fun insertOrReplace(attachment: AttachmentRecord) = dao.insertOrReplace(attachment)

    suspend fun delete(id: String) = dao.delete(id)

    suspend fun listBlobIds(): List<String> = dao.listBlobIds()

    suspend fun updateBlobData(id: String, data: ByteArray) = dao.updateBlobData(id, data)

    suspend fun listUploadedIds(): Set<String> = dao.listUploadedIds().toSet()

    suspend fun markUploaded(id: String) = dao.markUploaded(id)

    /** A new server holds none of this device's blobs. */
    suspend fun markAllNotUploaded() = dao.markAllNotUploaded()

    suspend fun clearOldFilenames(limit: Int): Int = dao.clearOldFilenames(limit)
}
