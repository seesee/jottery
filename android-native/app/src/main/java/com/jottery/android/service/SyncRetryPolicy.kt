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
     * registered again and a 403 needs the account restored (docs/sync-protocol.md).
     * Neither discards anything, and the next unlock tries again.
     */
    fun delaySeconds(error: Throwable, attempt: Int): Long? {
        if (error is SyncClientException) {
            when (error.statusCode) {
                401, 403 -> return null
                429 -> return error.retryAfterSeconds ?: DEFAULT_RATE_LIMIT_SECONDS
            }
        }
        val doublings = attempt.coerceIn(0, MAX_DOUBLINGS)
        return (BASE_SECONDS shl doublings).coerceAtMost(MAX_SECONDS)
    }
}
