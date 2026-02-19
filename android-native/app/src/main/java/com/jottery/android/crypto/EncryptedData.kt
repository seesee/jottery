package com.jottery.android.crypto

import kotlinx.serialization.Serializable

/**
 * Encrypted data wire format — JSON: {"ciphertext":"<base64>","iv":"<base64>"}
 * Base64 of ciphertext includes the GCM auth tag (last 16 bytes).
 */
@Serializable
data class EncryptedData(
    val ciphertext: String,  // base64(ciphertext + tag)
    val iv: String,          // base64(12-byte nonce)
)
