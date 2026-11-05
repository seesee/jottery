package com.jottery.android.ui.component

import android.text.format.DateUtils
import java.time.Instant

/** "5 min. ago", in the user's language, from the platform's own formatter. */
fun relativeTime(instant: Instant): String =
    DateUtils.getRelativeTimeSpanString(
        instant.toEpochMilli(),
        System.currentTimeMillis(),
        DateUtils.MINUTE_IN_MILLIS,
        DateUtils.FORMAT_ABBREV_RELATIVE,
    ).toString()
