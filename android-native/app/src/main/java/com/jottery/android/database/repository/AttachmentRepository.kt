package com.jottery.android.database.repository

import com.jottery.android.database.dao.AttachmentDao
import com.jottery.android.model.AttachmentRecord

/**
 * Repository for encrypted attachment blob storage.
 */
class AttachmentRepository(
    private val dao: AttachmentDao,
) {
    suspend fun get(id: String): AttachmentRecord? = dao.getById(id)

    suspend fun getBlob(id: String): ByteArray? = dao.getBlob(id)

    suspend fun insertOrReplace(attachment: AttachmentRecord) = dao.insertOrReplace(attachment)

    suspend fun delete(id: String) = dao.delete(id)

    suspend fun listBlobIds(): List<String> = dao.listBlobIds()

    suspend fun updateBlobData(id: String, data: ByteArray) = dao.updateBlobData(id, data)
}
