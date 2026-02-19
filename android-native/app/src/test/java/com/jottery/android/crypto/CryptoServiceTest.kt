package com.jottery.android.crypto

import org.junit.Assert.*
import org.junit.Test
import java.security.MessageDigest
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.PBEKeySpec
import javax.crypto.spec.SecretKeySpec
import javax.crypto.SecretKeyFactory

/**
 * JVM unit tests for cryptographic operations.
 *
 * These tests verify the core crypto logic without requiring Android APIs.
 * They use java.util.Base64 instead of android.util.Base64 and directly
 * exercise the javax.crypto primitives that CryptoService wraps.
 */
class CryptoServiceTest {

    companion object {
        private const val KEY_LENGTH = 32 // 256 bits
        private const val NONCE_LENGTH = 12
        private const val TAG_LENGTH = 128 // bits
        private const val SALT_LENGTH = 32
        private const val DEFAULT_ITERATIONS = 600_000
    }

    // -- Key Derivation --

    @Test
    fun `PBKDF2 derives 32-byte key`() {
        val password = "test-password"
        val salt = ByteArray(SALT_LENGTH).also { SecureRandom().nextBytes(it) }
        val key = deriveKey(password, salt, DEFAULT_ITERATIONS)
        assertEquals(KEY_LENGTH, key.encoded.size)
    }

    @Test
    fun `PBKDF2 is deterministic for same inputs`() {
        val password = "hello-world"
        val salt = ByteArray(SALT_LENGTH).also { SecureRandom().nextBytes(it) }
        val key1 = deriveKey(password, salt, DEFAULT_ITERATIONS)
        val key2 = deriveKey(password, salt, DEFAULT_ITERATIONS)
        assertArrayEquals(key1.encoded, key2.encoded)
    }

    @Test
    fun `PBKDF2 produces different keys for different passwords`() {
        val salt = ByteArray(SALT_LENGTH).also { SecureRandom().nextBytes(it) }
        val key1 = deriveKey("password-a", salt, DEFAULT_ITERATIONS)
        val key2 = deriveKey("password-b", salt, DEFAULT_ITERATIONS)
        assertFalse(key1.encoded.contentEquals(key2.encoded))
    }

    @Test
    fun `PBKDF2 produces different keys for different salts`() {
        val password = "same-password"
        val salt1 = ByteArray(SALT_LENGTH).also { SecureRandom().nextBytes(it) }
        val salt2 = ByteArray(SALT_LENGTH).also { SecureRandom().nextBytes(it) }
        val key1 = deriveKey(password, salt1, DEFAULT_ITERATIONS)
        val key2 = deriveKey(password, salt2, DEFAULT_ITERATIONS)
        assertFalse(key1.encoded.contentEquals(key2.encoded))
    }

    // -- AES-256-GCM Encrypt/Decrypt --

    @Test
    fun `AES-GCM round-trip produces original plaintext`() {
        val key = generateKey()
        val plaintext = "Hello, Jottery!".toByteArray(Charsets.UTF_8)
        val (ciphertext, nonce) = encrypt(plaintext, key)
        val decrypted = decrypt(ciphertext, nonce, key)
        assertArrayEquals(plaintext, decrypted)
    }

    @Test
    fun `AES-GCM ciphertext is longer than plaintext by tag size`() {
        val key = generateKey()
        val plaintext = "Short message".toByteArray(Charsets.UTF_8)
        val (ciphertext, _) = encrypt(plaintext, key)
        // GCM adds 16-byte (128-bit) authentication tag
        assertEquals(plaintext.size + 16, ciphertext.size)
    }

    @Test
    fun `AES-GCM with different nonces produces different ciphertexts`() {
        val key = generateKey()
        val plaintext = "Same plaintext".toByteArray(Charsets.UTF_8)
        val (ct1, _) = encrypt(plaintext, key)
        val (ct2, _) = encrypt(plaintext, key)
        // Overwhelmingly likely to be different due to random nonces
        assertFalse(ct1.contentEquals(ct2))
    }

    @Test(expected = Exception::class)
    fun `AES-GCM decryption with wrong key throws`() {
        val key1 = generateKey()
        val key2 = generateKey()
        val plaintext = "Secret".toByteArray(Charsets.UTF_8)
        val (ciphertext, nonce) = encrypt(plaintext, key1)
        decrypt(ciphertext, nonce, key2) // Should throw
    }

    @Test
    fun `AES-GCM handles empty plaintext`() {
        val key = generateKey()
        val plaintext = ByteArray(0)
        val (ciphertext, nonce) = encrypt(plaintext, key)
        val decrypted = decrypt(ciphertext, nonce, key)
        assertArrayEquals(plaintext, decrypted)
    }

    @Test
    fun `AES-GCM handles large plaintext`() {
        val key = generateKey()
        val plaintext = ByteArray(100_000) { (it % 256).toByte() }
        val (ciphertext, nonce) = encrypt(plaintext, key)
        val decrypted = decrypt(ciphertext, nonce, key)
        assertArrayEquals(plaintext, decrypted)
    }

    // -- SHA-256 --

    @Test
    fun `SHA-256 produces 32-byte hash`() {
        val data = "test data".toByteArray(Charsets.UTF_8)
        val hash = sha256(data)
        assertEquals(32, hash.size)
    }

    @Test
    fun `SHA-256 is deterministic`() {
        val data = "same input".toByteArray(Charsets.UTF_8)
        val hash1 = sha256(data)
        val hash2 = sha256(data)
        assertArrayEquals(hash1, hash2)
    }

    @Test
    fun `SHA-256 known vector`() {
        // SHA-256("hello") = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
        val hash = sha256("hello".toByteArray(Charsets.UTF_8))
        val hex = hash.joinToString("") { "%02x".format(it) }
        assertEquals("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824", hex)
    }

    // -- Deterministic JSON --

    @Test
    fun `deterministic JSON sorts keys alphabetically`() {
        // The buildDeterministicJSON function produces JSON with keys in this exact order:
        // archived, attachments, content, locked, pinned, showPreview, syntaxLanguage, tags, wordWrap
        val json = buildDeterministicJSON(
            content = "\"encrypted-content\"",
            tags = "\"encrypted-tags\"",
            archived = false,
            locked = false,
            pinned = true,
            showPreview = false,
            syntaxLanguage = "markdown",
            wordWrap = true,
            attachments = "[]",
        )

        // Verify key order
        val keyPattern = Regex("\"(\\w+)\":")
        val keys = keyPattern.findAll(json).map { it.groupValues[1] }.toList()
        assertEquals(
            listOf("archived", "attachments", "content", "locked", "pinned", "showPreview", "syntaxLanguage", "tags", "wordWrap"),
            keys,
        )
    }

    @Test
    fun `deterministic JSON is identical for same inputs`() {
        val json1 = buildDeterministicJSON("\"c1\"", "\"t1\"", false, false, false, false, "markdown", true, "[]")
        val json2 = buildDeterministicJSON("\"c1\"", "\"t1\"", false, false, false, false, "markdown", true, "[]")
        assertEquals(json1, json2)
    }

    // -- Hash Chain --

    @Test
    fun `updateHashChain prepends new hash`() {
        val chain = listOf("hash-b", "hash-a")
        val updated = updateHashChain("hash-c", chain)
        assertEquals(listOf("hash-c", "hash-b", "hash-a"), updated)
    }

    @Test
    fun `updateHashChain limits to 50 entries`() {
        val chain = (1..55).map { "hash-$it" }
        val updated = updateHashChain("hash-new", chain)
        assertEquals(50, updated.size)
        assertEquals("hash-new", updated.first())
    }

    @Test
    fun `findCommonAncestor returns first shared hash`() {
        val chainA = listOf("a3", "a2", "common", "old")
        val chainB = listOf("b2", "b1", "common", "old")
        assertEquals("common", findCommonAncestor(chainA, chainB))
    }

    @Test
    fun `findCommonAncestor returns null when no common hash`() {
        val chainA = listOf("a1", "a2")
        val chainB = listOf("b1", "b2")
        assertNull(findCommonAncestor(chainA, chainB))
    }

    @Test
    fun `isDescendant returns true when ancestor head is in descendant`() {
        val ancestor = listOf("hash-a", "hash-old")
        val descendant = listOf("hash-c", "hash-b", "hash-a", "hash-old")
        assertTrue(isDescendant(ancestor, descendant))
    }

    @Test
    fun `isDescendant returns false when ancestor head is not in descendant`() {
        val ancestor = listOf("hash-x", "hash-old")
        val descendant = listOf("hash-c", "hash-b", "hash-a")
        assertFalse(isDescendant(ancestor, descendant))
    }

    @Test
    fun `isDescendant returns true for empty ancestor`() {
        assertTrue(isDescendant(emptyList(), listOf("anything")))
    }

    // -- Helper Functions (mirrors CryptoService logic without android.util.Base64) --

    private fun deriveKey(password: String, salt: ByteArray, iterations: Int): SecretKey {
        val spec = PBEKeySpec(password.toCharArray(), salt, iterations, KEY_LENGTH * 8)
        val factory = SecretKeyFactory.getInstance("PBKDF2WithHmacSHA256")
        val keyBytes = factory.generateSecret(spec).encoded
        return SecretKeySpec(keyBytes, "AES")
    }

    private fun generateKey(): SecretKey {
        val keyBytes = ByteArray(KEY_LENGTH).also { SecureRandom().nextBytes(it) }
        return SecretKeySpec(keyBytes, "AES")
    }

    private fun encrypt(plaintext: ByteArray, key: SecretKey): Pair<ByteArray, ByteArray> {
        val nonce = ByteArray(NONCE_LENGTH).also { SecureRandom().nextBytes(it) }
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        val spec = GCMParameterSpec(TAG_LENGTH, nonce)
        cipher.init(Cipher.ENCRYPT_MODE, key, spec)
        val ciphertext = cipher.doFinal(plaintext)
        return ciphertext to nonce
    }

    private fun decrypt(ciphertext: ByteArray, nonce: ByteArray, key: SecretKey): ByteArray {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        val spec = GCMParameterSpec(TAG_LENGTH, nonce)
        cipher.init(Cipher.DECRYPT_MODE, key, spec)
        return cipher.doFinal(ciphertext)
    }

    private fun sha256(data: ByteArray): ByteArray {
        return MessageDigest.getInstance("SHA-256").digest(data)
    }

    private fun buildDeterministicJSON(
        content: String,
        tags: String,
        archived: Boolean,
        locked: Boolean,
        pinned: Boolean,
        showPreview: Boolean,
        syntaxLanguage: String,
        wordWrap: Boolean,
        attachments: String,
    ): String {
        return buildString {
            append("{")
            append("\"archived\":$archived,")
            append("\"attachments\":$attachments,")
            append("\"content\":$content,")
            append("\"locked\":$locked,")
            append("\"pinned\":$pinned,")
            append("\"showPreview\":$showPreview,")
            append("\"syntaxLanguage\":\"$syntaxLanguage\",")
            append("\"tags\":$tags,")
            append("\"wordWrap\":$wordWrap")
            append("}")
        }
    }

    private fun updateHashChain(currentHash: String, parentChain: List<String>): List<String> {
        val maxChainLength = 50
        val newChain = mutableListOf(currentHash)
        newChain.addAll(parentChain)
        return if (newChain.size > maxChainLength) newChain.take(maxChainLength) else newChain
    }

    private fun findCommonAncestor(chainA: List<String>, chainB: List<String>): String? {
        val setB = chainB.toSet()
        return chainA.firstOrNull { it in setB }
    }

    private fun isDescendant(ancestorChain: List<String>, descendantChain: List<String>): Boolean {
        if (ancestorChain.isEmpty()) return true
        val ancestorHead = ancestorChain.first()
        return ancestorHead in descendantChain
    }
}
