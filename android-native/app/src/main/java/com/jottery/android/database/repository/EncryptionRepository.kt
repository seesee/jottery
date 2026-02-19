package com.jottery.android.database.repository

import com.jottery.android.database.dao.EncryptionMetadataDao
import com.jottery.android.model.EncryptionMetadata

/**
 * Repository for encryption metadata (salt, iterations, verification token).
 */
class EncryptionRepository(
    private val dao: EncryptionMetadataDao,
) {
    suspend fun get(): EncryptionMetadata? = dao.get()

    suspend fun store(metadata: EncryptionMetadata) = dao.insertOrReplace(metadata)
}
