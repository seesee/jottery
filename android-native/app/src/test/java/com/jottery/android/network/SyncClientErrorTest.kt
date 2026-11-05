package com.jottery.android.network

import com.jottery.android.model.SyncPullRequest
import kotlinx.coroutines.runBlocking
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class SyncClientErrorTest {
    private val server = MockWebServer().apply { start() }
    private val client = SyncClient(server.url("/").toString().trimEnd('/'), "k", "c")

    @After fun tearDown() = server.shutdown()

    private fun failure(response: MockResponse): SyncClientException {
        server.enqueue(response)
        return runBlocking {
            runCatching {
                client.pull(SyncPullRequest(knownNoteIds = emptyList(), knownAttachmentIds = emptyList()))
            }.exceptionOrNull() as SyncClientException
        }
    }

    @Test fun `a rate limit carries its wait, from the header first`() {
        val e = failure(
            MockResponse().setResponseCode(429).setHeader("Retry-After", "30")
                .setBody("""{"error":"Too many attempts. Try again in 30 seconds.","code":"rate_limited","retryAfter":99}""")
        )

        assertEquals(429, e.statusCode)
        assertEquals(30L, e.retryAfterSeconds)
        assertEquals("rate_limited", e.code)
        assertEquals("Too many attempts. Try again in 30 seconds.", e.serverMessage)
    }

    @Test fun `the body's wait is used when the header is missing`() {
        val e = failure(MockResponse().setResponseCode(429).setBody("""{"retryAfter":12}"""))

        assertEquals(12L, e.retryAfterSeconds)
    }

    @Test fun `a server can't ask for a wait of years`() {
        val e = failure(MockResponse().setResponseCode(429).setHeader("Retry-After", "99999999"))

        assertEquals(3600L, e.retryAfterSeconds)
    }

    @Test fun `an error that isn't JSON is still an error`() {
        val e = failure(MockResponse().setResponseCode(502).setBody("<html>Bad gateway</html>"))

        assertEquals(502, e.statusCode)
        assertNull(e.serverMessage)
    }

    @Test fun `server text is cleaned before anyone can show it`() {
        val e = failure(
            MockResponse().setResponseCode(403).setBody("""{"error":"bad\u0007\nwords${"x".repeat(1000)}"}""")
        )

        assertTrue(e.serverMessage!!.none { it.isISOControl() })
        assertTrue(e.serverMessage!!.length <= 300)
    }

    @Test fun `raw server text stays out of the exception's own message`() {
        val e = failure(MockResponse().setResponseCode(500).setBody("""{"error":"secret detail"}"""))

        assertEquals("HTTP 500", e.message)
    }
}
