package com.jottery.android.service

import android.util.Base64
import android.util.Log
import com.jottery.android.crypto.CryptoService
import com.jottery.android.crypto.EncryptedData
import com.jottery.android.database.repository.EncryptionRepository
import com.jottery.android.database.repository.SyncRepository
import com.jottery.android.model.EncryptionMetadata
import com.jottery.android.model.PutWrappedKeyRequest
import com.jottery.android.network.SyncClient
import com.jottery.android.viewmodel.AppState
import javax.crypto.SecretKey
import javax.crypto.spec.SecretKeySpec

/**
 * Envelope encryption operations — migration, onboarding, and password change.
 *
 * Envelope encryption decouples the master key from the password:
 * a random master key encrypts data, and the password only wraps/unwraps
 * the master key. This makes password changes O(1) instead of O(n).
 */
object EnvelopeService {

    private const val TAG = "EnvelopeService"

    // MARK: - Migrate Legacy → Envelope

    /**
     * Attempt to migrate from legacy (direct PBKDF2) to envelope encryption.
     * Non-fatal: failures are logged and the legacy path continues to work.
     */
    suspend fun tryMigrateToEnvelope(
        appState: AppState,
        syncRepo: SyncRepository,
        encryptionRepo: EncryptionRepository,
        syncClient: SyncClient,
        password: String,
        masterKey: SecretKey,
    ) {
        try {
            val syncMeta = syncRepo.getSyncMetadata()
            val userId = syncMeta?.userId
            if (syncMeta == null || !syncMeta.syncEnabled || userId.isNullOrEmpty()) {
                Log.d(TAG, "[Envelope] Migration skipped — no userId in sync metadata")
                return
            }

            // Check if already migrated
            val metadata = encryptionRepo.get()
            if (metadata?.envelopeVersion != null) {
                Log.d(TAG, "[Envelope] Already migrated — skipping")
                return
            }

            val masterKeyData = masterKey.encoded

            // Derive wrapping key from password + userId (1M iterations)
            val wrappingKey = CryptoService.deriveWrappingKey(password, userId)

            // Wrap master key for server
            val serverWrapped = CryptoService.wrapMasterKey(masterKeyData, wrappingKey)
            val serverBlob = CryptoService.serializeEncryptedJSON(serverWrapped)

            // Upload to server
            syncClient.putWrappedKey(
                PutWrappedKeyRequest(
                    blob = serverBlob,
                    kdfVersion = CryptoService.WRAPPING_KDF_VERSION,
                    kdfIterations = CryptoService.WRAPPING_ITERATIONS,
                )
            )

            // Generate device salt and derive device key for local wrapping
            val deviceSalt = CryptoService.generateSalt()
            val deviceKey = CryptoService.deriveKey(password, deviceSalt, CryptoService.DEFAULT_ITERATIONS)

            // Wrap master key locally with device key
            val localWrapped = CryptoService.wrapMasterKey(masterKeyData, deviceKey)
            val localBlob = CryptoService.serializeEncryptedJSON(localWrapped)

            // Create verification token
            val verificationEncrypted = CryptoService.encryptText(
                EncryptionMetadata.VERIFICATION_PLAINTEXT, masterKey
            )
            val verificationJSON = CryptoService.serializeEncryptedJSON(verificationEncrypted)

            // Save envelope metadata (clears legacy salt/iterations)
            encryptionRepo.saveEnvelope(
                envelopeVersion = 1,
                deviceSalt = Base64.encodeToString(deviceSalt, Base64.NO_WRAP),
                localWrappedMaster = localBlob,
                wrappingKdfVersion = CryptoService.WRAPPING_KDF_VERSION,
            )

            // Update verification token
            encryptionRepo.get()?.let { meta ->
                encryptionRepo.store(meta.copy(verification = verificationJSON))
            }

            Log.i(TAG, "[Envelope] Migration complete")
        } catch (e: Exception) {
            Log.w(TAG, "[Envelope] Migration failed (non-fatal): ${e.message}")
        }
    }

    // MARK: - Onboard From Server

    /**
     * Onboard a new device by downloading the wrapped master key from the server.
     * Returns the unwrapped master key as a SecretKey.
     */
    suspend fun onboardFromServer(
        syncClient: SyncClient,
        encryptionRepo: EncryptionRepository,
        password: String,
        userId: String,
        localPassword: String? = null,
    ): SecretKey {
        val response = syncClient.getWrappedKey()
            ?: throw EnvelopeException("No wrapped key found on server")

        // Derive wrapping key from password + userId
        val wrappingKey = CryptoService.deriveWrappingKey(password, userId)

        // Unwrap master key
        val serverWrapped = CryptoService.parseEncryptedJSON(response.blob)
        val masterKeyData = CryptoService.unwrapMasterKey(serverWrapped, wrappingKey)

        require(masterKeyData.size == CryptoService.KEY_LENGTH) { "Invalid master key length" }

        val masterKey = SecretKeySpec(masterKeyData, "AES")

        // Generate device salt for local storage
        val deviceSalt = CryptoService.generateSalt()
        val deviceKey = CryptoService.deriveKey(localPassword ?: password, deviceSalt, CryptoService.DEFAULT_ITERATIONS)

        // Wrap master key locally
        val localWrapped = CryptoService.wrapMasterKey(masterKeyData, deviceKey)
        val localBlob = CryptoService.serializeEncryptedJSON(localWrapped)

        // Create verification token
        val verificationEncrypted = CryptoService.encryptText(
            EncryptionMetadata.VERIFICATION_PLAINTEXT, masterKey
        )
        val verificationJSON = CryptoService.serializeEncryptedJSON(verificationEncrypted)

        // Save envelope metadata
        encryptionRepo.saveEnvelope(
            envelopeVersion = 1,
            deviceSalt = Base64.encodeToString(deviceSalt, Base64.NO_WRAP),
            localWrappedMaster = localBlob,
            wrappingKdfVersion = CryptoService.WRAPPING_KDF_VERSION,
        )

        // Update verification token
        encryptionRepo.get()?.let { meta ->
            encryptionRepo.store(meta.copy(verification = verificationJSON))
        }

        return masterKey
    }

    // MARK: - Envelope Setup (Register / Unlock)

    /**
     * Set up envelope encryption after device registration or on unlock.
     * Checks the server for an existing wrapped key first (another device may
     * have already uploaded one). If found, onboards from the server key and
     * re-encrypts local data if the master key differs.
     */
    suspend fun tryEnvelopeSetup(
        appState: AppState,
        syncRepo: SyncRepository,
        encryptionRepo: EncryptionRepository,
        syncClient: SyncClient,
        password: String,
        masterKey: SecretKey,
    ) {
        try {
            val syncMeta = syncRepo.getSyncMetadata()
            val userId = syncMeta?.userId
            if (syncMeta == null || !syncMeta.syncEnabled || userId.isNullOrEmpty()) {
                Log.d(TAG, "[Envelope] Setup skipped — no userId in sync metadata")
                return
            }

            // Already migrated — nothing to do
            if (encryptionRepo.get()?.envelopeVersion != null) {
                Log.d(TAG, "[Envelope] Already migrated — skipping setup")
                return
            }

            // Check server for an existing wrapped key from another device
            val serverResponse = try { syncClient.getWrappedKey() } catch (_: Exception) { null }

            if (serverResponse != null) {
                // Another device already uploaded — onboard from server
                Log.i(TAG, "[Envelope] Server has wrapped key — onboarding from server")
                val serverKey = onboardFromServer(syncClient, encryptionRepo, password, userId)

                // If the server key differs from our local key, re-encrypt all data
                val localKeyData = masterKey.encoded
                val serverKeyData = serverKey.encoded
                if (!localKeyData.contentEquals(serverKeyData)) {
                    Log.i(TAG, "[Envelope] Server key differs from local — re-encrypting data")
                    appState.reEncryptAllData(masterKey, serverKey)
                    appState.keyManager.unlockWithKeyData(serverKeyData)
                    appState.loadNotes()
                }
            } else {
                // No server key — we're the first device, upload ours
                Log.i(TAG, "[Envelope] No server key — migrating local key to envelope")
                tryMigrateToEnvelope(appState, syncRepo, encryptionRepo, syncClient, password, masterKey)
            }
        } catch (e: Exception) {
            Log.w(TAG, "[Envelope] Setup failed (non-fatal): ${e.message}")
        }
    }

    // MARK: - Fast Password Change

    /**
     * Local re-wrap only — new device salt, re-wrap master key, save to DB.
     * Throws on failure so the caller knows immediately.
     */
    suspend fun changePasswordLocal(
        encryptionRepo: EncryptionRepository,
        newPassword: String,
        masterKeyData: ByteArray,
    ) {
        val newDeviceSalt = CryptoService.generateSalt()
        val newDeviceKey = CryptoService.deriveKey(newPassword, newDeviceSalt, CryptoService.DEFAULT_ITERATIONS)

        // Re-wrap master key locally
        val localWrapped = CryptoService.wrapMasterKey(masterKeyData, newDeviceKey)
        val localBlob = CryptoService.serializeEncryptedJSON(localWrapped)

        // Update verification token
        val masterKey = SecretKeySpec(masterKeyData, "AES")
        val verificationEncrypted = CryptoService.encryptText(
            EncryptionMetadata.VERIFICATION_PLAINTEXT, masterKey
        )
        val verificationJSON = CryptoService.serializeEncryptedJSON(verificationEncrypted)

        // Save updated envelope metadata
        encryptionRepo.saveEnvelope(
            envelopeVersion = 1,
            deviceSalt = Base64.encodeToString(newDeviceSalt, Base64.NO_WRAP),
            localWrappedMaster = localBlob,
            wrappingKdfVersion = CryptoService.WRAPPING_KDF_VERSION,
        )

        // Update verification token
        encryptionRepo.get()?.let { meta ->
            encryptionRepo.store(meta.copy(verification = verificationJSON))
        }

        Log.i(TAG, "[Envelope] Local password re-wrap complete")
    }

    /**
     * Server re-wrap — upload new wrapped key. Non-fatal.
     */
    suspend fun changePasswordServer(
        syncRepo: SyncRepository,
        syncClient: SyncClient,
        newPassword: String,
        masterKeyData: ByteArray,
    ) {
        try {
            val syncMeta = syncRepo.getSyncMetadata()
            val userId = syncMeta?.userId
            if (syncMeta == null || !syncMeta.syncEnabled || userId.isNullOrEmpty()) {
                Log.d(TAG, "[Envelope] Server re-wrap skipped — no userId")
                return
            }

            val newWrappingKey = CryptoService.deriveWrappingKey(newPassword, userId)
            val serverWrapped = CryptoService.wrapMasterKey(masterKeyData, newWrappingKey)
            val serverBlob = CryptoService.serializeEncryptedJSON(serverWrapped)

            syncClient.putWrappedKey(
                PutWrappedKeyRequest(
                    blob = serverBlob,
                    kdfVersion = CryptoService.WRAPPING_KDF_VERSION,
                    kdfIterations = CryptoService.WRAPPING_ITERATIONS,
                )
            )
            Log.i(TAG, "[Envelope] Server re-wrap complete")
        } catch (e: Exception) {
            Log.w(TAG, "[Envelope] Server re-wrap failed (non-fatal): ${e.message}")
        }
    }
}

class EnvelopeException(message: String) : Exception(message)
