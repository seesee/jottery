package com.jottery.android.util

import android.view.KeyEvent
import androidx.annotation.StringRes
import com.jottery.android.R

/** What a hardware keyboard can do, on a tablet, a Chromebook or a foldable. */
enum class KeyboardCommand(val keyCode: Int, @StringRes val label: Int) {
    NEW_NOTE(KeyEvent.KEYCODE_N, R.string.notes_new),
    SEARCH(KeyEvent.KEYCODE_K, R.string.search_bar_search),
    SAVE(KeyEvent.KEYCODE_S, R.string.action_save),
    FIND(KeyEvent.KEYCODE_F, R.string.editor_find),
    LOCK(KeyEvent.KEYCODE_L, R.string.settings_lock_now);

    companion object {
        /** Ctrl and that key alone, once: Ctrl+Shift+S is some other app's business. */
        fun from(event: KeyEvent): KeyboardCommand? {
            if (event.action != KeyEvent.ACTION_DOWN || event.repeatCount != 0) return null
            if (!event.isCtrlPressed || event.isShiftPressed || event.isAltPressed || event.isMetaPressed) return null
            return entries.firstOrNull { it.keyCode == event.keyCode }
        }
    }
}
