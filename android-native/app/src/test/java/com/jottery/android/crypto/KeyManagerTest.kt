package com.jottery.android.crypto

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.After
import org.junit.Assert.assertFalse
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
}
