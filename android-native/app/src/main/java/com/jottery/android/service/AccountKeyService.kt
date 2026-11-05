package com.jottery.android.service

import android.util.Log
import com.jottery.android.database.repository.EncryptionRepository
import com.jottery.android.database.repository.SyncRepository
import com.jottery.android.network.SyncClient

/**
 * Keeps the account's wrapped key and this device's record of it in step. The
 * decision about what to do is [EnvelopeService.reconcile]: this carries it out
 * and maintains the flag that decision depends on.
 */
class AccountKeyService(
    private val client: SyncClient,
    private val syncRepo: SyncRepository,
    private val encryptionRepo: EncryptionRepository,
) {
    /** What's left for the caller to do or tell the user. */
    sealed class Outcome {
        data object InStep : Outcome()
        data class MoveToAccountKey(val accountKey: ByteArray) : Outcome()
        data object Diverged : Outcome()
        data object CannotHeal : Outcome()
        data object PasswordChangedElsewhere : Outcome()
    }

    /** After an unlock with [notesPassword]. Throws if the server can't be reached. */
    suspend fun settle(
        notesPassword: String,
        localKey: ByteArray,
        canVouchForLocalKey: suspend () -> Boolean,
    ): Outcome {
        val meta = syncRepo.getSyncMetadata() ?: return Outcome.InStep
        val userId = meta.userId
        if (userId.isNullOrEmpty()) return Outcome.InStep

        // A change made here is still on its way: the blob is known to be behind
        // and won't open, which says nothing about any other device.
        if (meta.pendingWrappedKey != null) {
            uploadPending()
            return Outcome.InStep
        }

        val isLegacy = encryptionRepo.get()?.envelopeVersion == null
        val action = EnvelopeService.reconcile(
            client = client,
            userId = userId,
            notesPassword = notesPassword,
            localKey = localKey,
            isLegacyVault = isLegacy,
            hasOpenedBlob = meta.wrappedKeyOpened,
            canVouchForLocalKey = canVouchForLocalKey,
        )
        return when (action) {
            EnvelopeService.Reconciliation.Publish -> {
                EnvelopeService.publishKey(client, userId, notesPassword, localKey)
                syncRepo.markWrappedKeyOpened()
                if (isLegacy) EnvelopeService.saveLocalEnvelope(encryptionRepo, localKey, notesPassword)
                Outcome.InStep
            }
            is EnvelopeService.Reconciliation.InStep -> {
                syncRepo.markWrappedKeyOpened()
                if (action.finishMigration) {
                    EnvelopeService.saveLocalEnvelope(encryptionRepo, localKey, notesPassword)
                }
                Outcome.InStep
            }
            is EnvelopeService.Reconciliation.MoveToAccountKey -> {
                syncRepo.markWrappedKeyOpened()
                Outcome.MoveToAccountKey(action.accountKey)
            }
            EnvelopeService.Reconciliation.Diverged -> {
                syncRepo.markWrappedKeyOpened()
                Outcome.Diverged
            }
            EnvelopeService.Reconciliation.CannotHeal -> Outcome.CannotHeal
            EnvelopeService.Reconciliation.PasswordChangedElsewhere -> Outcome.PasswordChangedElsewhere
        }
    }

    /**
     * Send a rewrap left by a password change. Returns true if nothing is left
     * waiting. Runs at unlock and before every sync, so a failure only delays it.
     */
    suspend fun uploadPending(): Boolean {
        val blob = syncRepo.getSyncMetadata()?.pendingWrappedKey ?: return true
        return try {
            EnvelopeService.uploadBlob(client, blob)
            syncRepo.clearPendingWrappedKey(blob)
            syncRepo.markWrappedKeyOpened()
            true
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: Exception) {
            Log.w(TAG, "[Envelope] Rewrap still waiting: ${e.javaClass.simpleName}")
            false
        }
    }

    private companion object {
        const val TAG = "AccountKeyService"
    }
}
