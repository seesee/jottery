package com.jottery.android.crypto

import android.util.Base64
import com.jottery.android.model.SyncableNoteFields
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import java.security.SecureRandom
import java.util.UUID
import javax.crypto.Cipher
import javax.crypto.SecretKey
import javax.crypto.SecretKeyFactory
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.PBEKeySpec
import javax.crypto.spec.SecretKeySpec

/**
 * AES-256-GCM encryption service, byte-for-byte compatible with the iOS, web, and TUI clients.
 *
 * Wire format matches iOS `CryptoService.swift`:
 * - Key derivation: PBKDF2-HMAC-SHA256
 * - Cipher: AES-256-GCM
 * - Nonce: 12 random bytes
 * - Ciphertext encoding: base64(ciphertext || 16-byte tag)
 * - JSON: {"ciphertext":"...","iv":"..."}
 *
 * Note: Java's AES/GCM/NoPadding automatically appends the 16-byte GCM tag
 * to the ciphertext during encryption, and expects it appended during decryption.
 * This matches iOS's manual concatenation of ciphertext + tag.
 */
object CryptoService {

    // MARK: - Constants

    const val KEY_LENGTH = 32           // 256 bits
    const val NONCE_LENGTH = 12         // 96 bits — GCM recommendation
    const val SALT_LENGTH = 32          // 256 bits
    const val TAG_LENGTH_BITS = 128     // 16 bytes
    const val DEFAULT_ITERATIONS = 600_000
    const val MIN_ITERATIONS = 100_000
    const val MAX_HASH_CHAIN_LENGTH = 50

    private val secureRandom = SecureRandom()
    private val json = Json { ignoreUnknownKeys = true }

    // MARK: - Key Derivation

    /**
     * Derive a 256-bit AES key from a password using PBKDF2-HMAC-SHA256.
     * Uses the same algorithm as iOS's CCKeyDerivationPBKDF.
     *
     * IMPORTANT: PBEKeySpec takes a CharArray. The PBKDF2 standard uses the
     * UTF-8 encoding of the password, and Java's PBKDF2WithHmacSHA256 does this
     * correctly by converting the CharArray to UTF-8 bytes internally.
     */
    fun deriveKey(password: String, salt: ByteArray, iterations: Int): SecretKey {
        val effectiveIterations = maxOf(iterations, MIN_ITERATIONS)
        val spec = PBEKeySpec(
            password.toCharArray(),
            salt,
            effectiveIterations,
            KEY_LENGTH * 8,  // key length in bits
        )
        val factory = SecretKeyFactory.getInstance("PBKDF2WithHmacSHA256")
        val keyBytes = factory.generateSecret(spec).encoded
        spec.clearPassword()
        return SecretKeySpec(keyBytes, "AES")
    }

    // MARK: - Encrypt / Decrypt

    /** Encrypt plaintext to the standard wire format. */
    fun encryptText(plaintext: String, key: SecretKey): EncryptedData {
        return encrypt(plaintext.toByteArray(Charsets.UTF_8), key)
    }

    /** Decrypt the standard wire format back to plaintext. */
    fun decryptText(encrypted: EncryptedData, key: SecretKey): String {
        val data = decrypt(encrypted, key)
        return String(data, Charsets.UTF_8)
    }

    /** Encrypt raw data. */
    fun encrypt(data: ByteArray, key: SecretKey): EncryptedData {
        val nonce = ByteArray(NONCE_LENGTH)
        secureRandom.nextBytes(nonce)

        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        val gcmSpec = GCMParameterSpec(TAG_LENGTH_BITS, nonce)
        cipher.init(Cipher.ENCRYPT_MODE, key, gcmSpec)

        // Java GCM auto-appends the 16-byte tag to the ciphertext output.
        // This matches iOS's manual ciphertext + tag concatenation.
        val combined = cipher.doFinal(data)

        return EncryptedData(
            ciphertext = Base64.encodeToString(combined, Base64.NO_WRAP),
            iv = Base64.encodeToString(nonce, Base64.NO_WRAP),
        )
    }

    /** Decrypt raw data. */
    fun decrypt(encrypted: EncryptedData, key: SecretKey): ByteArray {
        val combinedData = Base64.decode(encrypted.ciphertext, Base64.NO_WRAP)
        val nonceData = Base64.decode(encrypted.iv, Base64.NO_WRAP)

        require(combinedData.size >= 16) { "Ciphertext too short" }

        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        val gcmSpec = GCMParameterSpec(TAG_LENGTH_BITS, nonceData)
        cipher.init(Cipher.DECRYPT_MODE, key, gcmSpec)

        // Java expects ciphertext + tag appended (same as our wire format).
        return cipher.doFinal(combinedData)
    }

    // MARK: - JSON Helpers

    /** Encrypt a JSON-serialisable value. */
    inline fun <reified T> encryptJSON(value: T, key: SecretKey): EncryptedData {
        val jsonStr = json.encodeToString(value)
        return encryptText(jsonStr, key)
    }

    /** Decrypt JSON back to a typed value. */
    inline fun <reified T> decryptJSON(encrypted: EncryptedData, key: SecretKey): T {
        val jsonStr = decryptText(encrypted, key)
        return json.decodeFromString(jsonStr)
    }

    /** Encrypt a string array (e.g. tags) — JSON-stringify then encrypt. */
    fun encryptStringArray(strings: List<String>, key: SecretKey): EncryptedData {
        return encryptJSON(strings, key)
    }

    /** Decrypt a string array. */
    fun decryptStringArray(encrypted: EncryptedData, key: SecretKey): List<String> {
        return decryptJSON(encrypted, key)
    }

    // MARK: - Hashing

    /** SHA-256 hash, returned as standard base64. */
    fun hash(data: String): String {
        val digest = java.security.MessageDigest.getInstance("SHA-256")
        val hashBytes = digest.digest(data.toByteArray(Charsets.UTF_8))
        return Base64.encodeToString(hashBytes, Base64.NO_WRAP)
    }

    /**
     * Compute content hash for conflict detection.
     * Must match the web client's computeContentHash exactly.
     */
    fun computeContentHash(note: SyncableNoteFields): String {
        val sortedAttachmentIds = note.attachmentIds.sorted()

        val pairs = listOf(
            "archived" to jsonEncodeValue(note.archived),
            "attachments" to jsonEncodeArray(sortedAttachmentIds),
            "content" to jsonEncode(note.content),
            "locked" to jsonEncodeValue(note.locked),
            "pinned" to jsonEncodeValue(note.pinned),
            "showPreview" to jsonEncodeValue(note.showPreview),
            "syntaxLanguage" to jsonEncodeNullableString(note.syntaxLanguage),
            "tags" to jsonEncodeArray(note.tags),
            "wordWrap" to jsonEncodeValue(note.wordWrap),
        )

        val json = "{" + pairs.joinToString(",") { (k, v) -> "${jsonEncode(k)}:$v" } + "}"
        return hash(json)
    }

    // MARK: - Hash Chain

    /** Update hash chain by prepending new hash and trimming. */
    fun updateHashChain(currentHash: String, parentChain: List<String> = emptyList()): List<String> {
        val newChain = mutableListOf(currentHash)
        newChain.addAll(parentChain)
        return if (newChain.size > MAX_HASH_CHAIN_LENGTH) {
            newChain.take(MAX_HASH_CHAIN_LENGTH)
        } else {
            newChain
        }
    }

    /** Find common ancestor between two hash chains. */
    fun findCommonAncestor(chainA: List<String>, chainB: List<String>): String? {
        val chainBSet = chainB.toSet()
        for (hash in chainA) {
            if (hash in chainBSet) return hash
        }
        return null
    }

    // MARK: - Random Generation

    /** Generate random salt (32 bytes). */
    fun generateSalt(): ByteArray {
        val salt = ByteArray(SALT_LENGTH)
        secureRandom.nextBytes(salt)
        return salt
    }

    /** Generate UUID v4 string. */
    fun generateUUID(): String = UUID.randomUUID().toString().lowercase()

    // MARK: - Wire Format Helpers

    /** Parse encrypted data from the JSON string stored in the database. */
    fun parseEncryptedJSON(jsonString: String): EncryptedData {
        return json.decodeFromString(jsonString)
    }

    /** Serialise encrypted data to JSON string for storage. */
    fun serializeEncryptedJSON(encrypted: EncryptedData): String {
        return json.encodeToString(encrypted)
    }

    // MARK: - Deterministic JSON Builder (private)

    private fun jsonEncode(string: String): String {
        val escaped = string
            .replace("\\", "\\\\")
            .replace("\"", "\\\"")
            .replace("\n", "\\n")
            .replace("\r", "\\r")
            .replace("\t", "\\t")
        return "\"$escaped\""
    }

    private fun jsonEncodeValue(value: Boolean): String = if (value) "true" else "false"

    private fun jsonEncodeNullableString(value: String?): String {
        return if (value != null) jsonEncode(value) else "null"
    }

    private fun jsonEncodeArray(items: List<String>): String {
        return "[" + items.joinToString(",") { jsonEncode(it) } + "]"
    }
}

