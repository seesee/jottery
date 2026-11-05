package com.jottery.android.ui.component

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import com.jottery.android.R
import com.jottery.android.model.SortOrder
import com.jottery.android.model.ThemeMode

@Composable
fun sortOrderLabel(order: SortOrder): String = stringResource(
    when (order) {
        SortOrder.RECENT -> R.string.sort_recent
        SortOrder.OLDEST -> R.string.sort_oldest
        SortOrder.ALPHA -> R.string.sort_alpha
        SortOrder.CREATED -> R.string.sort_created
    }
)

@Composable
fun themeModeLabel(mode: ThemeMode): String = stringResource(
    when (mode) {
        ThemeMode.LIGHT -> R.string.theme_light
        ThemeMode.DARK -> R.string.theme_dark
        ThemeMode.AUTO -> R.string.theme_system
    }
)

/** An auto-lock timeout in minutes, where 0 means never. */
@Composable
fun autoLockLabel(minutes: Int): String = when {
    minutes == 0 -> stringResource(R.string.auto_lock_never)
    minutes % 60 == 0 -> pluralStringResource(R.plurals.auto_lock_hours, minutes / 60, minutes / 60)
    else -> pluralStringResource(R.plurals.auto_lock_minutes, minutes, minutes)
}
