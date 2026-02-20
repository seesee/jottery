package com.jottery.android.network

import android.util.Log
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
 * Connects to the SSE endpoint and notifies on changes.
 * Includes exponential backoff reconnection.
 */
class SSEClient(
    private val baseUrl: String,
    private val scope: CoroutineScope,
) {
    companion object {
        private const val TAG = "SSEClient"
        private const val INITIAL_BACKOFF_MS = 1000L
        private const val MAX_BACKOFF_MS = 60_000L
    }

    var onSyncNeeded: (() -> Unit)? = null

    private var eventSource: EventSource? = null
    private var reconnectJob: Job? = null
    private var currentBackoff = INITIAL_BACKOFF_MS
    private var sseToken: String? = null
    private var isRunning = false

    private val client = OkHttpClient.Builder()
        .connectTimeout(30, TimeUnit.SECONDS)
        .readTimeout(0, TimeUnit.SECONDS) // No timeout for SSE
        .build()

    fun start(token: String) {
        sseToken = token
        isRunning = true
        connect()
    }

    fun stop() {
        isRunning = false
        reconnectJob?.cancel()
        reconnectJob = null
        eventSource?.cancel()
        eventSource = null
        currentBackoff = INITIAL_BACKOFF_MS
    }

    fun updateToken(token: String) {
        sseToken = token
        // Reconnect with new token
        eventSource?.cancel()
        connect()
    }

    private fun connect() {
        if (!isRunning) return
        val token = sseToken ?: return

        val request = Request.Builder()
            .url("$baseUrl/api/v1/sync/events?token=$token")
            .header("Accept", "text/event-stream")
            .build()

        val factory = EventSources.createFactory(client)
        eventSource = factory.newEventSource(request, object : EventSourceListener() {
            override fun onOpen(eventSource: EventSource, response: Response) {
                Log.d(TAG, "SSE connected")
                currentBackoff = INITIAL_BACKOFF_MS
            }

            override fun onEvent(
                eventSource: EventSource,
                id: String?,
                type: String?,
                data: String,
            ) {
                Log.d(TAG, "SSE event: type=$type data=$data")
                when (type) {
                    "sync", "change", "note_changed" -> {
                        onSyncNeeded?.invoke()
                    }
                }
            }

            override fun onClosed(eventSource: EventSource) {
                Log.d(TAG, "SSE closed")
                scheduleReconnect()
            }

            override fun onFailure(
                eventSource: EventSource,
                t: Throwable?,
                response: Response?,
            ) {
                Log.w(TAG, "SSE failure: ${t?.message}")
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
            if (isActive && isRunning) {
                connect()
            }
        }
    }
}
