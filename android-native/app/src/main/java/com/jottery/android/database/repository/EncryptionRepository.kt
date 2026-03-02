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

    /** Save envelope encryption metadata, clearing legacy salt/iterations. */
    suspend fun saveEnvelope(
        envelopeVersion: Int,
        deviceSalt: String,
        localWrappedMaster: String,
        wrappingKdfVersion: Int,
    ) {
        val existing = get() ?: return
        val updated = existing.copy(
            salt = null,
            iterations = null,
            envelopeVersion = envelopeVersion,
            deviceSalt = deviceSalt,
            localWrappedMaster = localWrappedMaster,
            wrappingKdfVersion = wrappingKdfVersion,
        )
        dao.insertOrReplace(updated)
    }
}
