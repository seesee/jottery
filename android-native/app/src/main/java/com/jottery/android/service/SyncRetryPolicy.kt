package com.jottery.android.service

import com.jottery.android.network.SyncClientException

/** How long to wait before trying a failed sync again. */
object SyncRetryPolicy {
    private const val BASE_SECONDS = 30L
    private const val MAX_SECONDS = 15 * 60L
    private const val DEFAULT_RATE_LIMIT_SECONDS = 60L
    private const val MAX_DOUBLINGS = 10

    /**
     * Seconds to wait, or null if retrying can't help: a 401 needs the device
     * registered again, a 403 the account restored, and a 507 the user to make room
     * (docs/sync-protocol.md). None of them discards anything, and the next periodic
     * sync or unlock tries again.
     */
    fun delaySeconds(error: Throwable, attempt: Int): Long? {
        if (error is SyncClientException) {
            when (error.statusCode) {
                401, 403, 507 -> return null
                429 -> return error.retryAfterSeconds ?: DEFAULT_RATE_LIMIT_SECONDS
                // A busy server says a second, and every device hears the same.
                // Once that hasn't worked, give it the room any failure gets.
                503 -> error.retryAfterSeconds?.let { wait ->
                    return if (attempt == 0) wait else maxOf(wait, backoff(attempt - 1))
                }
            }
        }
        return backoff(attempt)
    }

    private fun backoff(attempt: Int): Long =
        (BASE_SECONDS shl attempt.coerceIn(0, MAX_DOUBLINGS)).coerceAtMost(MAX_SECONDS)
}
