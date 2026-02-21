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

    // Base64-encoded 32-byte salt
    val salt: String,

    val iterations: Int,

    @ColumnInfo(name = "created_at")
    val createdAt: String,

    val algorithm: String = "AES-256-GCM",

    // Encrypted known plaintext for password verification
    val verification: String? = null,
) {
    val saltData: ByteArray?
        get() = try {
            Base64.decode(salt, Base64.NO_WRAP)
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
