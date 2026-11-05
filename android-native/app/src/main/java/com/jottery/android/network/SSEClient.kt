package com.jottery.android.network

import android.util.Log
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.sse.EventSource
import okhttp3.sse.EventSourceListener
import okhttp3.sse.EventSources
import java.util.concurrent.TimeUnit

/**
 * Server-Sent Events client for real-time sync notifications.
 *
 * Every connection uses a fresh token: they last five minutes, so one kept for a
 * reconnect is refused for ever. Failures back off exponentially, and retrying
 * stops once the API key itself is refused.
 */
class SSEClient(
    private val baseUrl: String,
    private val scope: CoroutineScope,
    private val initialBackoffMs: Long = 1000L,
) {
    companion object {
        private const val TAG = "SSEClient"
        private const val MAX_BACKOFF_MS = 60_000L
        private const val HTTP_UNAUTHORIZED = 401
        private const val HTTP_FORBIDDEN = 403
    }

    var onSyncNeeded: (() -> Unit)? = null

    /** Exchanges the API key for a token. Throws if that fails. */
    var tokenProvider: (suspend () -> String)? = null

    private var eventSource: EventSource? = null
    private var reconnectJob: Job? = null
    private var currentBackoff = initialBackoffMs

    @Volatile private var isRunning = false
    @Volatile private var isConnected = false

    private val client = OkHttpClient.Builder()
        .connectTimeout(30, TimeUnit.SECONDS)
        .readTimeout(0, TimeUnit.SECONDS) // No timeout for SSE
        .build()

    fun start() {
        isRunning = true
        reconnectNow()
    }

    fun stop() {
        isRunning = false
        isConnected = false
        reconnectJob?.cancel()
        reconnectJob = null
        eventSource?.cancel()
        eventSource = null
        currentBackoff = initialBackoffMs
    }

    /** The network came back: there's no reason to sit out the rest of a backoff. */
    fun reconnectNow() {
        if (!isRunning || isConnected) return
        currentBackoff = initialBackoffMs
        reconnectJob?.cancel()
        reconnectJob = scope.launch(Dispatchers.IO) { connectWithFreshToken() }
    }

    private suspend fun connectWithFreshToken() {
        val provider = tokenProvider ?: return
        val token = try {
            provider()
        } catch (e: CancellationException) {
            throw e
        } catch (e: SyncClientException) {
            if (e.statusCode == HTTP_UNAUTHORIZED || e.statusCode == HTTP_FORBIDDEN) {
                // No token will come until the device is registered or the account
                // restored, and a normal sync reports that to the user.
                Log.w(TAG, "SSE stopped: the server refused the API key")
                stop()
            } else {
                scheduleReconnect()
            }
            return
        } catch (e: Exception) {
            Log.w(TAG, "SSE token failed: ${e.javaClass.simpleName}")
            scheduleReconnect()
            return
        }
        connect(token)
    }

    private fun connect(token: String) {
        if (!isRunning) return

        val request = Request.Builder()
            .url("$baseUrl/api/v1/sync/events?token=$token")
            .header("Accept", "text/event-stream")
            .build()

        val factory = EventSources.createFactory(client)
        eventSource?.cancel()
        eventSource = factory.newEventSource(request, object : EventSourceListener() {
            override fun onOpen(eventSource: EventSource, response: Response) {
                Log.d(TAG, "SSE connected")
                isConnected = true
                currentBackoff = initialBackoffMs
            }

            override fun onEvent(
                eventSource: EventSource,
                id: String?,
                type: String?,
                data: String,
            ) {
                when (type) {
                    "sync", "change", "note_changed" -> {
                        onSyncNeeded?.invoke()
                    }
                }
            }

            override fun onClosed(eventSource: EventSource) {
                Log.d(TAG, "SSE closed")
                isConnected = false
                scheduleReconnect()
            }

            override fun onFailure(
                eventSource: EventSource,
                t: Throwable?,
                response: Response?,
            ) {
                Log.w(TAG, "SSE failure: ${response?.code ?: t?.javaClass?.simpleName}")
                isConnected = false
                scheduleReconnect()
            }
        })
    }

    private fun scheduleReconnect() {
        if (!isRunning) return
        reconnectJob?.cancel()
        reconnectJob = scope.launch(Dispatchers.IO) {
            delay(currentBackoff)
            currentBackoff = (currentBackoff * 2).coerceAtMost(MAX_BACKOFF_MS)
            if (isActive && isRunning) connectWithFreshToken()
        }
    }
}
