package com.jottery.android.model

import android.util.Base64
import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey
import com.jottery.android.util.DateUtils

/**
 * Encryption metadata. Singleton table (id=1).
 */
@Entity(tableName = "encryption_metadata")
data class EncryptionMetadata(
    @PrimaryKey
    val id: Int = 1,

    // Base64-encoded 32-byte salt (null when envelope active)
    val salt: String? = null,

    // PBKDF2 iterations (null when envelope active)
    val iterations: Int? = null,

    @ColumnInfo(name = "created_at")
    val createdAt: String,

    val algorithm: String = "AES-256-GCM",

    // Encrypted known plaintext for password verification
    val verification: String? = null,

    // Envelope encryption fields (post-migration)
    @ColumnInfo(name = "envelope_version")
    val envelopeVersion: Int? = null,       // 1 = envelope active

    @ColumnInfo(name = "device_salt")
    val deviceSalt: String? = null,         // Base64-encoded per-device salt

    @ColumnInfo(name = "local_wrapped_master")
    val localWrappedMaster: String? = null,  // JSON {"ciphertext":"...","iv":"..."}

    @ColumnInfo(name = "wrapping_kdf_version")
    val wrappingKdfVersion: Int? = null,     // 1 = PBKDF2
) {
    val saltData: ByteArray?
        get() = try {
            salt?.let { Base64.decode(it, Base64.NO_WRAP) }
        } catch (_: Exception) {
            null
        }

    companion object {
        const val VERIFICATION_PLAINTEXT = "jottery-vault-ok"

        fun new(salt: ByteArray, iterations: Int, verification: String): EncryptionMetadata {
            return EncryptionMetadata(
                salt = Base64.encodeToString(salt, Base64.NO_WRAP),
                iterations = iterations,
                createdAt = DateUtils.nowISO8601(),
                verification = verification,
            )
        }
    }
}
