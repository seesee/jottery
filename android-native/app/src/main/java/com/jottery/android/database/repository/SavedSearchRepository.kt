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
    suspend fun create(name: String, query: String, key: SecretKey, order: Int = 0): DecryptedSavedSearch {
        val id = CryptoService.generateUUID()
        val now = DateUtils.nowISO8601()
        val encName = CryptoService.encryptText(name, key)
        val encQuery = CryptoService.encryptText(query, key)

        val record = SavedSearch(
            id = id,
            name = CryptoService.serializeEncryptedJSON(encName),
            query = CryptoService.serializeEncryptedJSON(encQuery),
            displayOrder = order,
            createdAt = now,
            modifiedAt = now,
            needsSync = true,
        )
        dao.insertOrReplace(record)

        return DecryptedSavedSearch(
            id = id,
            name = name,
            query = query,
            displayOrder = order,
            createdAt = now,
            modifiedAt = now,
        )
    }

    /** Give a saved search a new name. */
    suspend fun rename(id: String, name: String, key: SecretKey) {
        val record = dao.getById(id) ?: return
        dao.insertOrReplace(record.copy(
            name = CryptoService.serializeEncryptedJSON(CryptoService.encryptText(name, key)),
            modifiedAt = DateUtils.nowISO8601(),
            needsSync = true,
        ))
    }

    /** Store a new order, touching only the searches that moved. */
    suspend fun setOrder(idsInOrder: List<String>) {
        val now = DateUtils.nowISO8601()
        idsInOrder.forEachIndexed { index, id ->
            val record = dao.getById(id) ?: return@forEachIndexed
            if (record.displayOrder != index) {
                dao.insertOrReplace(record.copy(displayOrder = index, modifiedAt = now, needsSync = true))
            }
        }
    }

    /** Soft-delete a saved search. */
    suspend fun delete(id: String) {
        val record = dao.getById(id) ?: return
        val now = DateUtils.nowISO8601()
        dao.insertOrReplace(record.copy(
            deleted = true,
            deletedAt = now,
            modifiedAt = now,
            needsSync = true,
        ))
    }

    /** Update encrypted fields (for password change). */
    suspend fun updateEncrypted(id: String, name: String, query: String) {
        dao.updateEncrypted(id, name, query)
    }

    /** Insert or replace raw record (for sync). */
    suspend fun insertOrReplace(record: SavedSearch) = dao.insertOrReplace(record)

    /** List all raw records (for backup). */
    suspend fun listAllRaw(): List<SavedSearch> = dao.listAll()

    /** Clear the flag, unless the search changed after [modifiedAt] was pushed. */
    suspend fun markSynced(id: String, syncedAt: String, modifiedAt: String) =
        dao.markSynced(id, syncedAt, modifiedAt)

    suspend fun getRaw(id: String): SavedSearch? = dao.getById(id)

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
