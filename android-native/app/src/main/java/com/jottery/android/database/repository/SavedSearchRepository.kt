package com.jottery.android.database.repository

import com.jottery.android.crypto.CryptoService
import com.jottery.android.database.dao.SavedSearchDao
import com.jottery.android.model.DecryptedSavedSearch
import com.jottery.android.model.SavedSearch
import com.jottery.android.util.DateUtils
import javax.crypto.SecretKey

/**
 * Repository for encrypted saved search queries.
 */
class SavedSearchRepository(
    private val dao: SavedSearchDao,
) {
    /** List all non-deleted saved searches, decrypted. */
    suspend fun listActive(key: SecretKey): List<DecryptedSavedSearch> {
        return dao.listActive().mapNotNull { record ->
            try {
                decrypt(record, key)
            } catch (_: Exception) {
                null
            }
        }
    }

    /** List all saved searches (including deleted), decrypted. */
    suspend fun listAll(key: SecretKey): List<DecryptedSavedSearch> {
        return dao.listAll().mapNotNull { record ->
            try {
                decrypt(record, key)
            } catch (_: Exception) {
                null
            }
        }
    }

    /** Create a new saved search. */
    suspend fun create(name: String, query: String, key: SecretKey): DecryptedSavedSearch {
        val id = CryptoService.generateUUID()
        val now = DateUtils.nowISO8601()
        val encName = CryptoService.encryptText(name, key)
        val encQuery = CryptoService.encryptText(query, key)

        val record = SavedSearch(
            id = id,
            name = CryptoService.serializeEncryptedJSON(encName),
            query = CryptoService.serializeEncryptedJSON(encQuery),
            displayOrder = 0,
            createdAt = now,
            modifiedAt = now,
            needsSync = true,
        )
        dao.insertOrReplace(record)

        return DecryptedSavedSearch(
            id = id,
            name = name,
            query = query,
            displayOrder = 0,
            createdAt = now,
            modifiedAt = now,
        )
    }

    /** Soft-delete a saved search. */
    suspend fun delete(id: String) {
        val record = dao.getById(id) ?: return
        dao.insertOrReplace(record.copy(
            deleted = true,
            deletedAt = DateUtils.nowISO8601(),
            needsSync = true,
        ))
    }

    /** Update encrypted fields (for password change). */
    suspend fun updateEncrypted(id: String, name: String, query: String) {
        dao.updateEncrypted(id, name, query)
    }

    /** Insert or replace raw record (for sync). */
    suspend fun insertOrReplace(record: SavedSearch) = dao.insertOrReplace(record)

    /** List records needing sync. */
    suspend fun listNeedingSync(): List<SavedSearch> = dao.listNeedingSync()

    private fun decrypt(record: SavedSearch, key: SecretKey): DecryptedSavedSearch {
        val encName = CryptoService.parseEncryptedJSON(record.name)
        val encQuery = CryptoService.parseEncryptedJSON(record.query)
        return DecryptedSavedSearch(
            id = record.id,
            name = CryptoService.decryptText(encName, key),
            query = CryptoService.decryptText(encQuery, key),
            displayOrder = record.displayOrder,
            createdAt = record.createdAt,
            modifiedAt = record.modifiedAt,
        )
    }
}
