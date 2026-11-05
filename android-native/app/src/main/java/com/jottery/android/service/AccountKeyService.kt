package com.jottery.android.service

import android.util.Log
import com.jottery.android.database.repository.EncryptionRepository
import com.jottery.android.database.repository.SyncRepository
import com.jottery.android.network.SyncClient
import com.jottery.android.network.SyncClientException

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

        return reconcile(userId, notesPassword, localKey, meta.wrappedKeyOpened, canVouchForLocalKey, readAgain = true)
    }

    private suspend fun reconcile(
        userId: String,
        notesPassword: String,
        localKey: ByteArray,
        hasOpenedBlob: Boolean,
        canVouchForLocalKey: suspend () -> Boolean,
        readAgain: Boolean,
    ): Outcome {
        val isLegacy = encryptionRepo.get()?.envelopeVersion == null
        val action = EnvelopeService.reconcile(
            client = client,
            userId = userId,
            notesPassword = notesPassword,
            localKey = localKey,
            isLegacyVault = isLegacy,
            hasOpenedBlob = hasOpenedBlob,
            canVouchForLocalKey = canVouchForLocalKey,
        )

        val write = when (action) {
            EnvelopeService.Reconciliation.PublishFirst -> EnvelopeService.KeyWrite.FirstUpload
            is EnvelopeService.Reconciliation.Repair -> EnvelopeService.KeyWrite.Repair(action.updatedAt)
            else -> null
        }
        if (write != null) {
            try {
                EnvelopeService.publishKey(client, userId, notesPassword, localKey, write)
            } catch (e: SyncClientException) {
                if (e.statusCode != HTTP_CONFLICT) throw e
                // Another device wrote the key while this one was deciding, so it
                // isn't old-format, and one that won't open is a password changed
                // elsewhere. Repairing again would overwrite exactly that. Remembering
                // this stops it at the next unlock too, by which time nothing could
                // tell a blob from a moment ago from one that has always been there.
                syncRepo.markWrappedKeyOpened()
                if (!readAgain) return Outcome.InStep
                return reconcile(
                    userId, notesPassword, localKey,
                    hasOpenedBlob = true,
                    canVouchForLocalKey = canVouchForLocalKey,
                    readAgain = false,
                )
            }
            syncRepo.markWrappedKeyOpened()
            if (isLegacy) EnvelopeService.saveLocalEnvelope(encryptionRepo, localKey, notesPassword)
            return Outcome.InStep
        }

        return when (action) {
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
            else -> Outcome.PasswordChangedElsewhere
        }
    }

    /**
     * Send a rewrap left by a password change. Returns true if nothing is left
     * waiting. Runs at unlock and before every sync, so a failure only delays it.
     */
    suspend fun uploadPending(): Boolean {
        val blob = syncRepo.getSyncMetadata()?.pendingWrappedKey ?: return true
        return try {
            EnvelopeService.uploadBlob(client, blob, EnvelopeService.KeyWrite.PasswordChange)
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
        const val HTTP_CONFLICT = 409
    }
}
