package com.jottery.android.crypto

import android.content.Context
import android.content.SharedPreferences
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyPermanentlyInvalidatedException
import android.security.keystore.KeyProperties
import android.util.Base64
import android.util.Log
import androidx.annotation.VisibleForTesting
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey
import java.io.File
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Secure storage, using the Android Keystore directly: for the API key and client ID,
 * and for the biometric-protected master key.
 */
object KeystoreService {

    private const val TAG = "KeystoreService"
    private const val KEY_API_KEY = "api_key"
    private const val KEY_CLIENT_ID = "client_id"

    // Each value is sealed under a Keystore key that never leaves the Keystore
    private const val SEALED_PREFS = "jottery_sealed_prefs"
    private const val SEALED_KEY_ALIAS = "jottery_sealed_prefs_key"
    private const val GCM_TAG_BITS = 128
    private const val GCM_IV_BYTES = 12

    // What older builds used. Google has deprecated EncryptedSharedPreferences, so
    // it's only opened to move those values here, and goes once no install needs it.
    private const val LEGACY_PREFS = "jottery_secure_prefs"

    // Biometric key storage
    private const val KEYSTORE_ALIAS = "jottery_biometric_key"
    private const val BIOMETRIC_PREFS = "jottery_biometric_prefs"
    private const val KEY_ENCRYPTED_MASTER_KEY = "encrypted_master_key"
    private const val KEY_MASTER_KEY_IV = "master_key_iv"

    // MARK: - Sealed Preferences (API Key / Client ID)

    @Volatile private var legacyChecked = false

    @Synchronized
    private fun sealedPrefs(context: Context): SharedPreferences {
        val app = context.applicationContext
        val prefs = app.getSharedPreferences(SEALED_PREFS, Context.MODE_PRIVATE)
        if (!legacyChecked) {
            moveLegacyValues(app, prefs)
            legacyChecked = true
        }
        return prefs
    }

    /**
     * Bring over what an older build stored. If its file can't be opened it's useless,
     * as it always was, so it's deleted and the device registers again. A failure on
     * this side is different: the values are still good, so the file stays for next time.
     */
    private fun moveLegacyValues(app: Context, prefs: SharedPreferences) {
        val file = File(File(app.applicationInfo.dataDir, "shared_prefs"), "$LEGACY_PREFS.xml")
        if (!file.exists()) return

        val legacy = try {
            openLegacyPrefs(app)
        } catch (e: Exception) {
            Log.w(TAG, "Old secure preferences unreadable, starting again: ${e.javaClass.simpleName}")
            app.deleteSharedPreferences(LEGACY_PREFS)
            return
        }

        val editor = prefs.edit()
        for (name in listOf(KEY_API_KEY, KEY_CLIENT_ID)) {
            // Anything already here is newer
            if (prefs.contains(name)) continue
            legacy.getString(name, null)?.let { editor.putString(name, seal(it)) }
        }
        if (editor.commit()) app.deleteSharedPreferences(LEGACY_PREFS)
    }

    private fun openLegacyPrefs(context: Context) = EncryptedSharedPreferences.create(
        context,
        LEGACY_PREFS,
        MasterKey.Builder(context)
            .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
            .build(),
        EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
        EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
    )

    private fun sealingKey(): SecretKey {
        val keystore = KeyStore.getInstance("AndroidKeyStore")
        keystore.load(null)
        keystore.getKey(SEALED_KEY_ALIAS, null)?.let { return it as SecretKey }

        val keyGen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        keyGen.init(
            KeyGenParameterSpec.Builder(
                SEALED_KEY_ALIAS,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build()
        )
        return keyGen.generateKey()
    }

    /** The IV, then the ciphertext, as Base64. The Keystore chooses a fresh IV each time. */
    private fun seal(value: String): String {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, sealingKey())
        val sealed = cipher.iv + cipher.doFinal(value.toByteArray(Charsets.UTF_8))
        return Base64.encodeToString(sealed, Base64.NO_WRAP)
    }

    private fun unseal(sealed: String): String {
        val bytes = Base64.decode(sealed, Base64.NO_WRAP)
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, sealingKey(), GCMParameterSpec(GCM_TAG_BITS, bytes, 0, GCM_IV_BYTES))
        return String(cipher.doFinal(bytes, GCM_IV_BYTES, bytes.size - GCM_IV_BYTES), Charsets.UTF_8)
    }

    /** Null if it can't be read. The value stays, as that may not last, and saving a new one replaces it. */
    private fun read(context: Context, name: String): String? {
        val sealed = sealedPrefs(context).getString(name, null) ?: return null
        return try {
            unseal(sealed)
        } catch (e: Exception) {
            Log.w(TAG, "Couldn't open a stored value: ${e.javaClass.simpleName}")
            null
        }
    }

    private fun write(context: Context, name: String, value: String) =
        sealedPrefs(context).edit().putString(name, seal(value)).apply()

    private fun remove(context: Context, name: String) =
        sealedPrefs(context).edit().remove(name).apply()

    fun getAPIKey(context: Context): String? = read(context, KEY_API_KEY)

    fun setAPIKey(context: Context, apiKey: String) = write(context, KEY_API_KEY, apiKey)

    fun deleteAPIKey(context: Context) = remove(context, KEY_API_KEY)

    fun getClientId(context: Context): String? = read(context, KEY_CLIENT_ID)

    fun setClientId(context: Context, clientId: String) = write(context, KEY_CLIENT_ID, clientId)

    fun deleteClientId(context: Context) = remove(context, KEY_CLIENT_ID)

    /** For tests: look for an older build's file again, as a new process would. */
    @VisibleForTesting
    internal fun forgetLegacyCheck() {
        legacyChecked = false
    }

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
            .putString(KEY_ENCRYPTED_MASTER_KEY, Base64.encodeToString(encrypted, Base64.NO_WRAP))
            .putString(KEY_MASTER_KEY_IV, Base64.encodeToString(cipher.iv, Base64.NO_WRAP))
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

        val iv = Base64.decode(ivB64, Base64.NO_WRAP)
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
        val encrypted = Base64.decode(encB64, Base64.NO_WRAP)
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
