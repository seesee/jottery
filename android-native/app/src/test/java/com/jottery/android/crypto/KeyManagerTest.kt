package com.jottery.android.crypto

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class KeyManagerTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
    private var now = 1_000_000L
    private val manager = KeyManager(scope) { now }

    @After fun tearDown() = scope.cancel()

    @Test fun `the timeout counts time asleep, because the clock does`() {
        manager.autoLockTimeout = 60
        manager.unlockWithKeyData(ByteArray(32))
        now += 59_000
        assertFalse(manager.isIdleTimeoutExceeded())
        now += 1_000
        assertTrue(manager.isIdleTimeoutExceeded())
    }

    @Test fun `activity resets the countdown`() {
        manager.autoLockTimeout = 60
        manager.unlockWithKeyData(ByteArray(32))
        now += 59_000
        manager.recordActivity()
        now += 59_000
        assertFalse(manager.isIdleTimeoutExceeded())
    }

    @Test fun `a timeout of zero means never`() {
        manager.autoLockTimeout = 0
        manager.unlockWithKeyData(ByteArray(32))
        now += 1_000_000_000
        assertFalse(manager.isIdleTimeoutExceeded())
    }

    @Test fun `a locked vault is never timed out`() {
        manager.autoLockTimeout = 60
        now += 1_000_000
        assertFalse(manager.isIdleTimeoutExceeded())
    }

    @Test fun `locking wipes the key, and using it afterwards fails rather than encrypt with zeros`() {
        val key = manager.unlockWithKeyData(ByteArray(32) { 7 })
        val sealed = CryptoService.encryptText("secret", key)
        assertEquals("secret", CryptoService.decryptText(sealed, key))

        manager.lock()

        assertNull(manager.masterKey)
        assertTrue(key.isDestroyed)
        assertThrows(KeyWipedException::class.java) { key.encoded }
        assertThrows(KeyWipedException::class.java) { CryptoService.encryptText("after the lock", key) }
    }

    @Test fun `the caller's bytes are its own`() {
        val bytes = ByteArray(32) { 7 }
        val key = manager.unlockWithKeyData(bytes)

        bytes.fill(0)

        assertArrayEquals(ByteArray(32) { 7 }, key.encoded)
    }

    @Test fun `release locks but leaves the key usable until the caller is done`() {
        val key = manager.unlockWithKeyData(ByteArray(32) { 7 })

        val released = manager.release()

        assertNull(manager.masterKey)
        assertFalse(manager.isUnlocked)
        assertSame(key, released)
        assertEquals(32, released!!.encoded.size)
        released.destroy()
        assertTrue(key.isDestroyed)
    }

    @Test fun `replacing the key wipes the old one`() {
        val old = manager.unlockWithKeyData(ByteArray(32) { 1 })

        manager.unlockWithKeyData(ByteArray(32) { 2 })

        assertTrue(old.isDestroyed)
    }
}
