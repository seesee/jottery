package com.jottery.android.util

import java.time.Instant
import java.time.ZoneOffset
import java.time.format.DateTimeFormatter

/**
 * ISO 8601 date/time utilities matching iOS Date+ISO8601.swift.
 */
object DateUtils {

    /** ISO 8601 formatter with fractional seconds and UTC timezone. */
    private val isoFormatter: DateTimeFormatter =
        DateTimeFormatter.ofPattern("yyyy-MM-dd'T'HH:mm:ss.SSS'Z'")
            .withZone(ZoneOffset.UTC)

    /** Current time as ISO 8601 string. */
    fun nowISO8601(): String = isoFormatter.format(Instant.now())

    /** Parse an ISO 8601 string to Instant. Handles with and without fractional seconds. */
    fun parseISO8601(dateString: String): Instant? {
        return try {
            Instant.parse(dateString)
        } catch (_: Exception) {
            try {
                // Try parsing without fractional seconds
                val normalized = if (dateString.endsWith("Z") && !dateString.contains(".")) {
                    dateString.replace("Z", ".000Z")
                } else {
                    dateString
                }
                Instant.parse(normalized)
            } catch (_: Exception) {
                null
            }
        }
    }

    /** Format an Instant to ISO 8601 string. */
    fun formatISO8601(instant: Instant): String = isoFormatter.format(instant)
}
