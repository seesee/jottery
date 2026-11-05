package com.jottery.android.service.backup

import java.io.ByteArrayOutputStream
import java.security.SecureRandom
import java.util.Base64
import java.util.zip.DataFormatException
import java.util.zip.Deflater
import java.util.zip.Inflater
import javax.crypto.AEADBadTagException
import javax.crypto.Cipher
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * The batch encoding of the shared backup format: JSON, then zlib, then JWE
 * compact with `dir` and `A256GCM`. See docs/backup-format.md.
 *
 * Android writes exactly what web's `jose` and `pako` write. It also reads what
 * iOS has written: raw DEFLATE instead of zlib, and no AAD on the ciphertext.
 */
object BackupCodec {

    private const val TAG_BYTES = 16
    private const val IV_BYTES = 12
    private const val PROTECTED_HEADER = """{"alg":"dir","enc":"A256GCM"}"""

    /** Inflated batches larger than this are refused, not read. */
    private const val MAX_INFLATED_BYTES = 512L * 1024 * 1024

    private val random = SecureRandom()
    private val urlEncoder = Base64.getUrlEncoder().withoutPadding()
    private val urlDecoder = Base64.getUrlDecoder()

    fun encryptBatch(json: String, key: SecretKey): String =
        jweEncrypt(deflate(json.toByteArray(Charsets.UTF_8)), key)

    fun decryptBatch(jwe: String, key: SecretKey): String =
        String(inflate(jweDecrypt(jwe, key)), Charsets.UTF_8)

    /** Web's v2.1 records: JSON straight into a JWE, with no compression. */
    fun decryptRecord(jwe: String, key: SecretKey): String =
        String(jweDecrypt(jwe, key), Charsets.UTF_8)

    fun jweEncrypt(plaintext: ByteArray, key: SecretKey): String {
        val header = urlEncoder.encodeToString(PROTECTED_HEADER.toByteArray(Charsets.UTF_8))
        val iv = ByteArray(IV_BYTES).also(random::nextBytes)

        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, key, GCMParameterSpec(TAG_BYTES * 8, iv))
        // RFC 7516: the AAD is the ASCII of the encoded protected header
        cipher.updateAAD(header.toByteArray(Charsets.US_ASCII))
        val sealed = cipher.doFinal(plaintext)

        val ciphertext = sealed.copyOfRange(0, sealed.size - TAG_BYTES)
        val tag = sealed.copyOfRange(sealed.size - TAG_BYTES, sealed.size)
        return listOf(
            header,
            "",
            urlEncoder.encodeToString(iv),
            urlEncoder.encodeToString(ciphertext),
            urlEncoder.encodeToString(tag),
        ).joinToString(".")
    }

    fun jweDecrypt(jwe: String, key: SecretKey): ByteArray {
        val parts = jwe.split(".")
        if (parts.size != 5) throw BackupException.Corrupt()

        val iv: ByteArray
        val sealed: ByteArray
        try {
            iv = urlDecoder.decode(parts[2])
            sealed = urlDecoder.decode(parts[3]) + urlDecoder.decode(parts[4])
        } catch (_: IllegalArgumentException) {
            throw BackupException.Corrupt()
        }

        // Web and Android authenticate the header. iOS doesn't, so try without.
        val aad = parts[0].toByteArray(Charsets.US_ASCII)
        return try {
            open(sealed, iv, key, aad)
        } catch (_: AEADBadTagException) {
            try {
                open(sealed, iv, key, null)
            } catch (_: AEADBadTagException) {
                throw BackupException.WrongPassword()
            }
        }
    }

    private fun open(sealed: ByteArray, iv: ByteArray, key: SecretKey, aad: ByteArray?): ByteArray {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(TAG_BYTES * 8, iv))
        if (aad != null) cipher.updateAAD(aad)
        return cipher.doFinal(sealed)
    }

    /** zlib format, as `pako.deflate` writes. */
    fun deflate(data: ByteArray): ByteArray {
        val deflater = Deflater()
        try {
            deflater.setInput(data)
            deflater.finish()
            val out = ByteArrayOutputStream(data.size / 2 + 64)
            val buffer = ByteArray(64 * 1024)
            while (!deflater.finished()) {
                out.write(buffer, 0, deflater.deflate(buffer))
            }
            return out.toByteArray()
        } finally {
            deflater.end()
        }
    }

    /** Reads zlib, and the raw DEFLATE iOS writes. */
    fun inflate(data: ByteArray): ByteArray =
        try {
            inflate(data, raw = false)
        } catch (_: DataFormatException) {
            try {
                inflate(data, raw = true)
            } catch (_: DataFormatException) {
                throw BackupException.Corrupt()
            }
        }

    private fun inflate(data: ByteArray, raw: Boolean): ByteArray {
        val inflater = Inflater(raw)
        try {
            inflater.setInput(data)
            val out = ByteArrayOutputStream(data.size * 2)
            val buffer = ByteArray(64 * 1024)
            while (!inflater.finished()) {
                val count = inflater.inflate(buffer)
                if (count == 0 && (inflater.needsInput() || inflater.needsDictionary())) {
                    throw DataFormatException("Truncated batch")
                }
                out.write(buffer, 0, count)
                if (out.size() > MAX_INFLATED_BYTES) throw BackupException.Corrupt()
            }
            return out.toByteArray()
        } finally {
            inflater.end()
        }
    }
}
