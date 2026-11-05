package com.jottery.android.service

import android.util.Base64
import android.util.Log
import com.jottery.android.crypto.CryptoService
import com.jottery.android.database.repository.EncryptionRepository
import com.jottery.android.model.EncryptionMetadata
import com.jottery.android.model.PutWrappedKeyRequest
import com.jottery.android.network.SyncClient
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import javax.crypto.spec.SecretKeySpec

/**
 * Envelope encryption: a master key encrypts the data, and passwords only wrap
 * it, so changing a password never touches the notes.
 *
 * The server stores ciphertext and is not trusted, so three rules hold here (D1):
 * the blob on the server is only ever opened with the notes password, never the
 * account password the server itself receives; a blob that opens is never
 * overwritten, even if it holds a different key; and a device that has already
 * opened or uploaded the blob never overwrites one that stops opening, because
 * that only happens when the password was changed on another device.
 */
object EnvelopeService {

    private const val TAG = "EnvelopeService"
    private const val MAX_TAG_LENGTH = 128

    /**
     * What a write to the account's key is for. There's one key per account, and
     * overwriting it with the wrong one locks out every new device, so a write says
     * what it expects to find. See "Wrapped master key" in docs/sync-protocol.md.
     */
    sealed interface KeyWrite {
        /** The account had no key when this device looked. Refused if it has one now. */
        data object FirstUpload : KeyWrite

        /** Refused if the key has changed since the read that gave [updatedAt]. */
        data class Repair(val updatedAt: String?) : KeyWrite

        /** Deliberate, so it overwrites whatever is there. */
        data object PasswordChange : KeyWrite
    }

    /** How the account's server key relates to this device's vault key. */
    sealed class AccountKeyStatus {
        /** The server holds no key: this device can publish its own. */
        data object NoServerKey : AccountKeyStatus()
        data object MatchesLocalKey : AccountKeyStatus()
        data class Differs(val accountKey: ByteArray) : AccountKeyStatus()
    }

    /** The local half of an envelope, ready to store. */
    data class LocalEnvelope(
        val deviceSalt: String,
        val localWrappedMaster: String,
        val verification: String,
    )

    /**
     * Download the account's wrapped key and open it with the notes password,
     * changing nothing locally. Returns null only when the server says it has no
     * key (404). A network or server failure throws, so it can never be taken
     * for "no key" and lead to the account's key being replaced.
     *
     * @throws EnvelopeException.CannotUnwrap if [notesPassword] doesn't open it.
     */
    suspend fun fetchServerKey(client: SyncClient, userId: String, notesPassword: String): ByteArray? {
        val response = withContext(Dispatchers.IO) { client.getWrappedKey() } ?: return null
        return withContext(Dispatchers.Default) {
            val master = try {
                val wrapped = CryptoService.parseEncryptedJSON(response.blob)
                CryptoService.unwrapMasterKey(wrapped, CryptoService.deriveWrappingKey(notesPassword, userId))
            } catch (e: Exception) {
                throw EnvelopeException.CannotUnwrap(response.updatedAt)
            }
            if (master.size != CryptoService.KEY_LENGTH) throw EnvelopeException.CannotUnwrap(response.updatedAt)
            master
        }
    }

    /** Compare the account's key with the vault's own, changing nothing. */
    suspend fun checkAccountKey(
        client: SyncClient,
        userId: String,
        notesPassword: String,
        localKey: ByteArray,
    ): AccountKeyStatus {
        val serverKey = fetchServerKey(client, userId, notesPassword)
            ?: return AccountKeyStatus.NoServerKey
        return if (serverKey.contentEquals(localKey)) {
            AccountKeyStatus.MatchesLocalKey
        } else {
            AccountKeyStatus.Differs(serverKey)
        }
    }

    /** What an unlock should do about the account's wrapped key. */
    sealed class Reconciliation {
        /** Nothing to change on the server. [finishMigration] is true for a legacy vault. */
        data class InStep(val finishMigration: Boolean) : Reconciliation()

        /** The account has no key: upload this vault's, wrapped with the notes password. */
        data object PublishFirst : Reconciliation()

        /** An old-format blob: replace the one that was read, and no other. */
        data class Repair(val updatedAt: String?) : Reconciliation()

        /** A legacy vault on another key: move it onto the account's. */
        data class MoveToAccountKey(val accountKey: ByteArray) : Reconciliation()

        /** An envelope vault on another key: tell the user, change nothing. */
        data object Diverged : Reconciliation()

        /** The blob won't open and this device can't vouch for its own key. */
        data object CannotHeal : Reconciliation()

        /**
         * The blob won't open, and this device has opened or uploaded it before,
         * so it isn't in the old format: another device changed the password.
         * Overwriting it would undo that change.
         */
        data object PasswordChangedElsewhere : Reconciliation()
    }

    /**
     * Decide what to do after an unlock. Nothing is changed here.
     *
     * - No key on the server: publish this vault's. Only an explicit 404 counts,
     *   and the server refuses it if another device got there first.
     * - It opens with the notes password: never overwrite it, even if the key
     *   differs, because that means this device has diverged.
     * - It won't open, and this device has opened or uploaded it before: the
     *   password changed elsewhere. Never overwrite it.
     * - It won't open, and this device never has: older clients wrapped it with
     *   the account password. A device that has synced and can read everything it
     *   holds has the account's real key, so it may re-wrap with the notes password
     *   (D1 self-healing). Doing so sets the flag, so that happens once at most, and
     *   the server refuses it if the blob has changed since it was read here.
     *
     * @param hasOpenedBlob this device's flag: it has opened or uploaded the blob.
     * @param canVouchForLocalKey has completed a sync and opens every local note.
     *   Android's check goes further than D1's "has synced" because older builds
     *   could sync on a key of their own after a failed onboarding.
     */
    suspend fun reconcile(
        client: SyncClient,
        userId: String,
        notesPassword: String,
        localKey: ByteArray,
        isLegacyVault: Boolean,
        hasOpenedBlob: Boolean,
        canVouchForLocalKey: suspend () -> Boolean,
    ): Reconciliation {
        val status = try {
            checkAccountKey(client, userId, notesPassword, localKey)
        } catch (e: EnvelopeException.CannotUnwrap) {
            return when {
                hasOpenedBlob -> Reconciliation.PasswordChangedElsewhere
                canVouchForLocalKey() -> Reconciliation.Repair(e.updatedAt)
                else -> Reconciliation.CannotHeal
            }
        }
        return when (status) {
            AccountKeyStatus.NoServerKey -> Reconciliation.PublishFirst
            AccountKeyStatus.MatchesLocalKey -> Reconciliation.InStep(finishMigration = isLegacyVault)
            is AccountKeyStatus.Differs ->
                if (isLegacyVault) Reconciliation.MoveToAccountKey(status.accountKey)
                else Reconciliation.Diverged
        }
    }

    /** [masterKey] wrapped with the notes password, as the server stores it. */
    suspend fun wrapForAccount(userId: String, notesPassword: String, masterKey: ByteArray): String =
        withContext(Dispatchers.Default) {
            val wrappingKey = CryptoService.deriveWrappingKey(notesPassword, userId)
            CryptoService.serializeEncryptedJSON(CryptoService.wrapMasterKey(masterKey, wrappingKey))
        }

    /**
     * Upload a blob from [wrapForAccount]. Throws [SyncClientException] with 409 if
     * the write is refused: don't retry it, read the key again and decide afresh.
     */
    suspend fun uploadBlob(client: SyncClient, blob: String, write: KeyWrite) {
        val request = PutWrappedKeyRequest(
            blob = blob,
            kdfVersion = CryptoService.WRAPPING_KDF_VERSION,
            kdfIterations = CryptoService.WRAPPING_ITERATIONS,
        )
        withContext(Dispatchers.IO) {
            when (write) {
                KeyWrite.FirstUpload -> client.putWrappedKey(request.copy(replace = false))
                KeyWrite.PasswordChange -> client.putWrappedKey(request.copy(replace = true))
                is KeyWrite.Repair -> {
                    // Only a server with conditional writes sends this, and one that
                    // doesn't can't do better than overwrite. It comes from the server,
                    // so it must not be able to break out of the header.
                    val tag = write.updatedAt?.takeIf(::isUsableTag)
                    if (tag != null) client.putWrappedKey(request, ifMatch = tag)
                    else client.putWrappedKey(request.copy(replace = true))
                }
            }
        }
    }

    private fun isUsableTag(tag: String) = tag.length <= MAX_TAG_LENGTH && tag.all { it.code in 0x20..0x7E }

    /** Wrap [masterKey] with the notes password and upload it. */
    suspend fun publishKey(
        client: SyncClient,
        userId: String,
        notesPassword: String,
        masterKey: ByteArray,
        write: KeyWrite,
    ) {
        uploadBlob(client, wrapForAccount(userId, notesPassword, masterKey), write)
    }

    /** A fresh device salt, the master key wrapped under it, and a verification token. */
    suspend fun localEnvelope(masterKey: ByteArray, password: String): LocalEnvelope =
        withContext(Dispatchers.Default) {
            val deviceSalt = CryptoService.generateSalt()
            val deviceKey = CryptoService.deriveKey(password, deviceSalt, CryptoService.DEFAULT_ITERATIONS)
            LocalEnvelope(
                deviceSalt = Base64.encodeToString(deviceSalt, Base64.NO_WRAP),
                localWrappedMaster = CryptoService.serializeEncryptedJSON(
                    CryptoService.wrapMasterKey(masterKey, deviceKey)
                ),
                verification = CryptoService.serializeEncryptedJSON(
                    CryptoService.encryptText(
                        EncryptionMetadata.VERIFICATION_PLAINTEXT,
                        SecretKeySpec(masterKey, "AES"),
                    )
                ),
            )
        }

    /** The vault metadata for [envelope], built on [current]. Nothing is saved. */
    fun metadataFor(envelope: LocalEnvelope, current: EncryptionMetadata) = current.copy(
        salt = null,
        iterations = null,
        verification = envelope.verification,
        envelopeVersion = 1,
        deviceSalt = envelope.deviceSalt,
        localWrappedMaster = envelope.localWrappedMaster,
        wrappingKdfVersion = CryptoService.WRAPPING_KDF_VERSION,
    )

    /**
     * Put the vault on an envelope around [masterKey] with [password], in one
     * write. The key doesn't change, so the data and other devices are unaffected.
     */
    suspend fun saveLocalEnvelope(repo: EncryptionRepository, masterKey: ByteArray, password: String) {
        val current = repo.get() ?: return
        repo.store(metadataFor(localEnvelope(masterKey, password), current))
        Log.i(TAG, "[Envelope] Local envelope saved")
    }
}

sealed class EnvelopeException(message: String) : Exception(message) {
    /**
     * The notes password didn't open the account's key.
     *
     * @param updatedAt identifies the blob that wouldn't open, for a repair.
     */
    class CannotUnwrap(val updatedAt: String? = null) :
        EnvelopeException("The notes password can't open the account's key")
}
