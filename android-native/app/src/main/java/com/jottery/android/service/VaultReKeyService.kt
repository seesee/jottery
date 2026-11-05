package com.jottery.android.service

import com.jottery.android.crypto.CryptoService
import com.jottery.android.crypto.EncryptedData
import com.jottery.android.database.repository.AttachmentRepository
import com.jottery.android.database.repository.EncryptionRepository
import com.jottery.android.database.repository.NoteRepository
import com.jottery.android.database.repository.SavedSearchRepository
import com.jottery.android.database.repository.VersionRepository
import com.jottery.android.model.AttachmentRef
import com.jottery.android.model.EncryptionMetadata
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import javax.crypto.SecretKey

/**
 * Moves a whole vault from one master key to another in a single transaction,
 * with the vault's metadata swapped in the same transaction. Ported from iOS
 * VaultReKeyService. If anything can't be decrypted the transaction rolls back
 * and nothing changes: the vault is on one key or the other, never a mix.
 *
 * Values that already open with the new key are left alone: a vault that joined
 * an account and synced before moving holds rows pulled under that key. Only a
 * value that opens with neither key aborts.
 */
class VaultReKeyService(
    private val noteRepo: NoteRepository,
    private val versionRepo: VersionRepository,
    private val attachmentRepo: AttachmentRepository,
    private val savedSearchRepo: SavedSearchRepository,
    private val encryptionRepo: EncryptionRepository,
    private val transaction: suspend (suspend () -> Unit) -> Unit,
) {
    class Unreadable(table: String, id: String) :
        Exception("Could not re-encrypt $table $id; nothing was changed")

    private val json = Json { ignoreUnknownKeys = true; encodeDefaults = true }

    /** Re-encrypt everything and store [newMetadata]. Notes are queued for sync. */
    suspend fun reKey(oldKey: SecretKey, newKey: SecretKey, newMetadata: EncryptionMetadata) {
        val keys = ReKeyer(oldKey, newKey)
        transaction {
            for (note in noteRepo.listAllRaw()) {
                val content = keys.required(note.content, "notes", note.id)
                val tags = keys.required(note.tags, "notes", note.id)
                val attachments = attachmentList(note.attachments, keys, "notes", note.id)
                if (content != note.content || tags != note.tags || attachments != note.attachments) {
                    noteRepo.updateRaw(
                        note.copy(content = content, tags = tags, attachments = attachments, needsSync = true)
                    )
                }
            }

            for (version in versionRepo.listAll()) {
                // Older builds wrote '[]' for a version with no tags: under no key
                val tags = if (version.tags == "[]") version.tags
                else keys.required(version.tags, "note_versions", version.versionKey)
                val updated = version.copy(
                    content = keys.required(version.content, "note_versions", version.versionKey),
                    tags = tags,
                    attachments = attachmentList(version.attachments, keys, "note_versions", version.versionKey),
                )
                if (updated != version) versionRepo.insertOrReplace(updated)
            }

            // One blob at a time: they can be large
            for (id in attachmentRepo.listBlobIds()) {
                val blob = attachmentRepo.getBlob(id) ?: continue
                val encrypted = try {
                    CryptoService.parseEncryptedJSON(String(blob, Charsets.UTF_8))
                } catch (_: Exception) {
                    continue // Under no key at all
                }
                val updated = keys.reEncrypt(encrypted, "attachments", id) ?: continue
                attachmentRepo.updateBlobData(id, updated.toByteArray(Charsets.UTF_8))
            }

            for (search in savedSearchRepo.listAllRaw()) {
                val name = keys.required(search.name, "saved_searches", search.id)
                val query = keys.required(search.query, "saved_searches", search.id)
                if (name != search.name || query != search.query) {
                    savedSearchRepo.updateEncrypted(search.id, name, query)
                }
            }

            encryptionRepo.store(newMetadata)
        }
    }

    private fun attachmentList(attachments: String, keys: ReKeyer, table: String, id: String): String {
        if (attachments.isBlank() || attachments == "[]") return attachments
        val refs = try {
            json.decodeFromString<List<AttachmentRef>>(attachments)
        } catch (_: Exception) {
            throw Unreadable(table, id)
        }
        var changed = false
        val updated = refs.map { ref ->
            val encrypted = try {
                CryptoService.parseEncryptedJSON(ref.filename)
            } catch (_: Exception) {
                return@map ref // A plaintext name from an older build: under no key
            }
            val filename = keys.reEncrypt(encrypted, table, id) ?: return@map ref
            changed = true
            ref.copy(filename = filename)
        }
        return if (changed) json.encodeToString(updated) else attachments
    }

    private class ReKeyer(private val oldKey: SecretKey, private val newKey: SecretKey) {
        /** A value that must be encrypted, under the old key or already the new. */
        fun required(value: String, table: String, id: String): String {
            val encrypted = try {
                CryptoService.parseEncryptedJSON(value)
            } catch (_: Exception) {
                throw Unreadable(table, id)
            }
            return reEncrypt(encrypted, table, id) ?: value
        }

        /** The value under the new key, or null if it's already there. */
        fun reEncrypt(encrypted: EncryptedData, table: String, id: String): String? {
            val plain = try {
                CryptoService.decrypt(encrypted, oldKey)
            } catch (_: Exception) {
                try {
                    CryptoService.decrypt(encrypted, newKey)
                } catch (_: Exception) {
                    throw Unreadable(table, id)
                }
                return null
            }
            return CryptoService.serializeEncryptedJSON(CryptoService.encrypt(plain, newKey))
        }
    }
}
