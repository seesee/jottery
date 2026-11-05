package com.jottery.android.crypto

import android.content.Context
import android.content.SharedPreferences
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyPermanentlyInvalidatedException
import android.security.keystore.KeyProperties
import android.util.Log
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

    private const val TAG = "KeystoreService"
    private const val PREFS_NAME = "jottery_secure_prefs"
    private const val KEY_API_KEY = "api_key"
    private const val KEY_CLIENT_ID = "client_id"

    // Biometric key storage
    private const val KEYSTORE_ALIAS = "jottery_biometric_key"
    private const val BIOMETRIC_PREFS = "jottery_biometric_prefs"
    private const val KEY_ENCRYPTED_MASTER_KEY = "encrypted_master_key"
    private const val KEY_MASTER_KEY_IV = "master_key_iv"

    // MARK: - Encrypted SharedPreferences (API Key / Client ID)

    @Volatile private var encryptedPrefs: SharedPreferences? = null

    /**
     * Built once. If the file can't be opened it's useless, as when it arrived
     * without its Keystore key, so it's deleted and the device registers again.
     */
    @Synchronized
    private fun getEncryptedPrefs(context: Context): SharedPreferences {
        encryptedPrefs?.let { return it }
        val app = context.applicationContext
        val prefs = try {
            openEncryptedPrefs(app)
        } catch (e: Exception) {
            Log.w(TAG, "Secure preferences unreadable, starting again: ${e.javaClass.simpleName}")
            app.deleteSharedPreferences(PREFS_NAME)
            openEncryptedPrefs(app)
        }
        encryptedPrefs = prefs
        return prefs
    }

    private fun openEncryptedPrefs(context: Context) = EncryptedSharedPreferences.create(
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
     * A cipher to seal the master key with. The Keystore key can only be used after
     * a biometric, so this has to go through BiometricPrompt before [storeBiometricKey].
     */
    fun biometricEncryptCipher(): Cipher {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        try {
            cipher.init(Cipher.ENCRYPT_MODE, getOrCreateBiometricKeystoreKey())
        } catch (_: KeyPermanentlyInvalidatedException) {
            // Enrolments changed since the key was made, so it can never be used again
            deleteKeystoreKey()
            cipher.init(Cipher.ENCRYPT_MODE, getOrCreateBiometricKeystoreKey())
        }
        return cipher
    }

    /** Seal the master key with a cipher that BiometricPrompt has authorised. */
    fun storeBiometricKey(context: Context, cipher: Cipher, masterKeyData: ByteArray) {
        val encrypted = cipher.doFinal(masterKeyData)
        context.getSharedPreferences(BIOMETRIC_PREFS, Context.MODE_PRIVATE)
            .edit()
            .putString(KEY_ENCRYPTED_MASTER_KEY, android.util.Base64.encodeToString(encrypted, android.util.Base64.NO_WRAP))
            .putString(KEY_MASTER_KEY_IV, android.util.Base64.encodeToString(cipher.iv, android.util.Base64.NO_WRAP))
            .apply()
    }

    /**
     * The cipher for opening the stored key, to pass to BiometricPrompt. Returns null
     * if there's no key, or if enrolments have changed: the Keystore has then destroyed
     * it, so the stored copy is cleared and the password is the only way in.
     */
    fun getBiometricDecryptCipher(context: Context): Cipher? {
        val prefs = context.getSharedPreferences(BIOMETRIC_PREFS, Context.MODE_PRIVATE)
        val ivB64 = prefs.getString(KEY_MASTER_KEY_IV, null) ?: return null

        val keystore = KeyStore.getInstance("AndroidKeyStore")
        keystore.load(null)
        val key = keystore.getKey(KEYSTORE_ALIAS, null) as? SecretKey
        if (key == null) {
            deleteBiometricKey(context)
            return null
        }

        val iv = android.util.Base64.decode(ivB64, android.util.Base64.NO_WRAP)
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        return try {
            cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(128, iv))
            cipher
        } catch (_: KeyPermanentlyInvalidatedException) {
            deleteBiometricKey(context)
            null
        }
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
        deleteKeystoreKey()
    }

    private fun deleteKeystoreKey() {
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
