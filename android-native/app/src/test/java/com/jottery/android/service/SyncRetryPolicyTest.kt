package com.jottery.android.service

import com.jottery.android.network.SyncClientException
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import java.io.IOException

class SyncRetryPolicyTest {
    @Test fun `a failure backs off and levels out`() {
        val delays = (0..7).map { SyncRetryPolicy.delaySeconds(IOException(), it) }

        assertEquals(listOf(30L, 60L, 120L, 240L, 480L, 900L, 900L, 900L), delays)
    }

    @Test fun `a huge attempt count can't overflow into a tiny delay`() {
        assertEquals(900L, SyncRetryPolicy.delaySeconds(IOException(), Int.MAX_VALUE))
    }

    @Test fun `a revoked device or disabled account isn't retried`() {
        assertNull(SyncRetryPolicy.delaySeconds(SyncClientException("x", 401), 0))
        assertNull(SyncRetryPolicy.delaySeconds(SyncClientException("x", 403), 0))
    }

    @Test fun `a rate limit waits as long as the server said`() {
        val limited = SyncClientException("x", 429, retryAfterSeconds = 45)

        assertEquals(45L, SyncRetryPolicy.delaySeconds(limited, 3))
        assertEquals(60L, SyncRetryPolicy.delaySeconds(SyncClientException("x", 429), 3))
    }

    @Test fun `a full account isn't retried until the user makes room`() {
        assertNull(SyncRetryPolicy.delaySeconds(SyncClientException("x", 507, code = "storage_quota_exceeded"), 0))
    }

    @Test fun `a busy server is asked again when it said, then given more room`() {
        val busy = SyncClientException("x", 503, retryAfterSeconds = 1)

        assertEquals(1L, SyncRetryPolicy.delaySeconds(busy, 0))
        assertEquals(30L, SyncRetryPolicy.delaySeconds(busy, 1))
        assertEquals(60L, SyncRetryPolicy.delaySeconds(busy, 2))
    }

    @Test fun `a timeout is retried, because sending a push twice is safe`() {
        assertEquals(30L, SyncRetryPolicy.delaySeconds(SyncClientException("x", 408), 0))
    }

    @Test fun `a server error is retried`() {
        assertEquals(30L, SyncRetryPolicy.delaySeconds(SyncClientException("x", 503), 0))
    }
}
