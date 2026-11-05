package com.jottery.android.crypto

import android.os.SystemClock
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import javax.crypto.SecretKey

/**
 * A key whose one copy of its bytes can be wiped, which SecretKeySpec's can't.
 * Reading it afterwards throws rather than hand out zeros, because anything
 * encrypted with those would be lost.
 *
 * The JVM can still have made copies while using it, inside a Cipher for instance,
 * so this shortens how long the key sits in memory rather than guaranteeing it's
 * gone. Android gives no way to do better.
 */
class WipeableKey(bytes: ByteArray) : SecretKey {
    private val bytes = bytes.clone()

    @Volatile private var wiped = false

    override fun getAlgorithm() = "AES"
    override fun getFormat() = "RAW"
    override fun getEncoded(): ByteArray {
        if (wiped) throw KeyWipedException()
        return bytes.clone()
    }

    override fun destroy() {
        wiped = true
        bytes.fill(0)
    }

    override fun isDestroyed() = wiped
}

/** Something outlived the lock that wiped its key. */
class KeyWipedException : IllegalStateException("The vault has been locked")

/**
 * Manages the in-memory master key, auto-lock timer, and biometric unlock.
 * The SecretKey is kept in memory only — never persisted to disk.
 */
class KeyManager(
    private val scope: CoroutineScope,
    /** Counts through deep sleep and can't be wound back, unlike the wall clock. */
    private val clock: () -> Long = SystemClock::elapsedRealtime,
) {
    /** The in-memory master key. */
    var masterKey: SecretKey? = null
        internal set

    var isUnlocked: Boolean = false
        private set

    /** Seconds since last user interaction before auto-locking. 0 means never. */
    var autoLockTimeout: Long = 15 * 60L
        set(value) {
            field = value
            // Leaving "Never" must start the timer without waiting for an unlock
            if (isUnlocked) startAutoLockTimer()
        }

    /** Called when the auto-lock timer fires. */
    var onAutoLock: (() -> Unit)? = null

    private var lastActivityTime = clock()
    private var autoLockJob: Job? = null

    // MARK: - Unlock

    /** Derive the master key from a password and store it in memory. */
    fun unlock(password: String, salt: ByteArray, iterations: Int): SecretKey =
        adopt(CryptoService.deriveKey(password, salt, iterations).encoded)

    /** Restore the master key from raw bytes. The caller keeps its own copy. */
    fun unlockWithKeyData(keyData: ByteArray): SecretKey = adopt(keyData)

    private fun adopt(keyData: ByteArray): SecretKey {
        val previous = masterKey
        val key = WipeableKey(keyData)
        masterKey = key
        (previous as? WipeableKey)?.destroy()
        isUnlocked = true
        lastActivityTime = clock()
        startAutoLockTimer()
        return key
    }

    /** Lock the application — wipe the in-memory key. */
    fun lock() {
        release()?.destroy()
    }

    /**
     * Lock, but hand the key to the caller, who must [SecretKey.destroy] it. For a
     * final write that has to finish first.
     */
    fun release(): SecretKey? {
        val key = masterKey
        masterKey = null
        isUnlocked = false
        stopAutoLockTimer()
        return key
    }

    /** Record user activity to reset the auto-lock countdown. */
    fun recordActivity() {
        lastActivityTime = clock()
    }

    // MARK: - Auto-Lock

    private fun startAutoLockTimer() {
        stopAutoLockTimer()
        if (autoLockTimeout <= 0) return

        autoLockJob = scope.launch {
            while (true) {
                delay(30_000L) // Check every 30 seconds
                checkAutoLock()
            }
        }
    }

    private fun stopAutoLockTimer() {
        autoLockJob?.cancel()
        autoLockJob = null
    }

    /** Whether the timeout has passed since the user last did anything. */
    fun isIdleTimeoutExceeded(): Boolean {
        if (!isUnlocked || autoLockTimeout <= 0) return false
        return (clock() - lastActivityTime) / 1000L >= autoLockTimeout
    }

    private fun checkAutoLock() {
        if (isIdleTimeoutExceeded()) {
            onAutoLock?.invoke() ?: lock()
        }
    }
}
