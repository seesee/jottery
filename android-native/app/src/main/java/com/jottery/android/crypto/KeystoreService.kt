package com.jottery.android.crypto

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Manages secure storage using EncryptedSharedPreferences (for API key, client ID)
 * and Android Keystore (for biometric-protected master key).
 */
object KeystoreService {

    private const val PREFS_NAME = "jottery_secure_prefs"
    private const val KEY_API_KEY = "api_key"
    private const val KEY_CLIENT_ID = "client_id"

    // Biometric key storage
    private const val KEYSTORE_ALIAS = "jottery_biometric_key"
    private const val BIOMETRIC_PREFS = "jottery_biometric_prefs"
    private const val KEY_ENCRYPTED_MASTER_KEY = "encrypted_master_key"
    private const val KEY_MASTER_KEY_IV = "master_key_iv"

    // MARK: - Encrypted SharedPreferences (API Key / Client ID)

    private fun getEncryptedPrefs(context: Context) = EncryptedSharedPreferences.create(
        context,
        PREFS_NAME,
        MasterKey.Builder(context)
            .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
            .build(),
        EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
        EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
    )

    fun getAPIKey(context: Context): String? =
        getEncryptedPrefs(context).getString(KEY_API_KEY, null)

    fun setAPIKey(context: Context, apiKey: String) =
        getEncryptedPrefs(context).edit().putString(KEY_API_KEY, apiKey).apply()

    fun deleteAPIKey(context: Context) =
        getEncryptedPrefs(context).edit().remove(KEY_API_KEY).apply()

    fun getClientId(context: Context): String? =
        getEncryptedPrefs(context).getString(KEY_CLIENT_ID, null)

    fun setClientId(context: Context, clientId: String) =
        getEncryptedPrefs(context).edit().putString(KEY_CLIENT_ID, clientId).apply()

    fun deleteClientId(context: Context) =
        getEncryptedPrefs(context).edit().remove(KEY_CLIENT_ID).apply()

    // MARK: - Biometric Key (Android Keystore)

    /**
     * Store the master key bytes encrypted with a Keystore key
     * that requires biometric authentication to use.
     */
    fun storeBiometricKey(context: Context, masterKeyData: ByteArray) {
        // Generate or retrieve the Keystore key
        val keystoreKey = getOrCreateBiometricKeystoreKey()

        // Encrypt the master key
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, keystoreKey)
        val encrypted = cipher.doFinal(masterKeyData)
        val iv = cipher.iv

        // Store encrypted data in regular SharedPreferences
        context.getSharedPreferences(BIOMETRIC_PREFS, Context.MODE_PRIVATE)
            .edit()
            .putString(KEY_ENCRYPTED_MASTER_KEY, android.util.Base64.encodeToString(encrypted, android.util.Base64.NO_WRAP))
            .putString(KEY_MASTER_KEY_IV, android.util.Base64.encodeToString(iv, android.util.Base64.NO_WRAP))
            .apply()
    }

    /**
     * Retrieve the Cipher for biometric decryption (to be passed to BiometricPrompt).
     * Returns null if no biometric key exists.
     */
    fun getBiometricDecryptCipher(context: Context): Cipher? {
        val prefs = context.getSharedPreferences(BIOMETRIC_PREFS, Context.MODE_PRIVATE)
        val ivB64 = prefs.getString(KEY_MASTER_KEY_IV, null) ?: return null

        val keystore = KeyStore.getInstance("AndroidKeyStore")
        keystore.load(null)
        val key = keystore.getKey(KEYSTORE_ALIAS, null) as? SecretKey ?: return null

        val iv = android.util.Base64.decode(ivB64, android.util.Base64.NO_WRAP)
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(128, iv))
        return cipher
    }

    /**
     * Decrypt the master key using the authenticated cipher from BiometricPrompt.
     */
    fun decryptBiometricKey(context: Context, cipher: Cipher): ByteArray? {
        val prefs = context.getSharedPreferences(BIOMETRIC_PREFS, Context.MODE_PRIVATE)
        val encB64 = prefs.getString(KEY_ENCRYPTED_MASTER_KEY, null) ?: return null
        val encrypted = android.util.Base64.decode(encB64, android.util.Base64.NO_WRAP)
        return cipher.doFinal(encrypted)
    }

    fun hasBiometricKey(context: Context): Boolean {
        val prefs = context.getSharedPreferences(BIOMETRIC_PREFS, Context.MODE_PRIVATE)
        return prefs.contains(KEY_ENCRYPTED_MASTER_KEY)
    }

    fun deleteBiometricKey(context: Context) {
        context.getSharedPreferences(BIOMETRIC_PREFS, Context.MODE_PRIVATE)
            .edit().clear().apply()
        try {
            val keystore = KeyStore.getInstance("AndroidKeyStore")
            keystore.load(null)
            keystore.deleteEntry(KEYSTORE_ALIAS)
        } catch (_: Exception) {
            // Key may not exist
        }
    }

    private fun getOrCreateBiometricKeystoreKey(): SecretKey {
        val keystore = KeyStore.getInstance("AndroidKeyStore")
        keystore.load(null)
        keystore.getKey(KEYSTORE_ALIAS, null)?.let { return it as SecretKey }

        val keyGen = KeyGenerator.getInstance(
            KeyProperties.KEY_ALGORITHM_AES,
            "AndroidKeyStore",
        )
        keyGen.init(
            KeyGenParameterSpec.Builder(
                KEYSTORE_ALIAS,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .setUserAuthenticationRequired(true)
                .setInvalidatedByBiometricEnrollment(true)
                .build()
        )
        return keyGen.generateKey()
    }
}
