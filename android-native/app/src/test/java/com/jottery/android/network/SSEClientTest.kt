package com.jottery.android.network

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import okhttp3.mockwebserver.Dispatcher
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okhttp3.mockwebserver.RecordedRequest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.atomic.AtomicInteger

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class SSEClientTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val paths = CopyOnWriteArrayList<String>()

    /** Accepts only the newest token, as the server does once one has expired. */
    @Volatile private var validToken = ""

    private val server = MockWebServer().apply {
        dispatcher = object : Dispatcher() {
            override fun dispatch(request: RecordedRequest): MockResponse {
                paths += request.path.orEmpty()
                return if (request.path.orEmpty().endsWith("token=$validToken")) {
                    MockResponse()
                        .setHeader("Content-Type", "text/event-stream")
                        .setBody("event: sync\ndata: pull\n\n")
                } else {
                    MockResponse().setResponseCode(401).setBody("{}")
                }
            }
        }
        start()
    }

    private val sse = SSEClient(server.url("/").toString().trimEnd('/'), scope, initialBackoffMs = 5)

    @After fun tearDown() {
        sse.stop()
        scope.cancel()
        server.shutdown()
    }

    private suspend fun until(condition: () -> Boolean) = withTimeout(10_000) {
        while (!condition()) delay(5)
    }

    @Test fun `a reconnect uses a fresh token, because the old one has expired`() = runBlocking {
        val issued = AtomicInteger()
        val events = AtomicInteger()
        sse.onSyncNeeded = { events.incrementAndGet() }
        sse.tokenProvider = {
            val n = issued.incrementAndGet()
            // The first token is already out of date by the time it's used
            if (n > 1) validToken = "token-$n"
            "token-$n"
        }

        sse.start()

        until { events.get() > 0 }
        assertTrue(paths.first().endsWith("token=token-1"))
        assertTrue(paths.any { it.endsWith("token=token-2") })
    }

    @Test fun `a refused API key stops the retries`() = runBlocking {
        val attempts = AtomicInteger()
        sse.tokenProvider = {
            attempts.incrementAndGet()
            throw SyncClientException("HTTP 401", 401)
        }

        sse.start()

        until { attempts.get() > 0 }
        delay(100)
        assertEquals(1, attempts.get())
        assertTrue(paths.isEmpty())
    }

    @Test fun `a failing token request keeps trying`() = runBlocking {
        val attempts = AtomicInteger()
        sse.tokenProvider = {
            attempts.incrementAndGet()
            throw SyncClientException("HTTP 503", 503)
        }

        sse.start()

        until { attempts.get() >= 3 }
    }
}
