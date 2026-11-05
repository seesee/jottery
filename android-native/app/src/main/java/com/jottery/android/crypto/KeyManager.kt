package com.jottery.android.crypto

import android.os.SystemClock
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import javax.crypto.SecretKey
import javax.crypto.spec.SecretKeySpec

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
    fun unlock(password: String, salt: ByteArray, iterations: Int): SecretKey {
        val key = CryptoService.deriveKey(password, salt, iterations)
        masterKey = key
        isUnlocked = true
        lastActivityTime = clock()
        startAutoLockTimer()
        return key
    }

    /** Restore the master key from raw bytes (biometric unlock path). */
    fun unlockWithKeyData(keyData: ByteArray) {
        masterKey = SecretKeySpec(keyData, "AES")
        isUnlocked = true
        lastActivityTime = clock()
        startAutoLockTimer()
    }

    /** Lock the application — wipe the in-memory key. */
    fun lock() {
        masterKey = null
        isUnlocked = false
        stopAutoLockTimer()
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
