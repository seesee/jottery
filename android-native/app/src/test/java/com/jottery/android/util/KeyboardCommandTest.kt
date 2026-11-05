package com.jottery.android.util

import android.view.KeyEvent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class KeyboardCommandTest {
    private fun event(key: Int, meta: Int, action: Int = KeyEvent.ACTION_DOWN, repeat: Int = 0) =
        KeyEvent(0, 0, action, key, repeat, meta)

    @Test fun `Ctrl and a key is the command`() {
        assertEquals(KeyboardCommand.NEW_NOTE, KeyboardCommand.from(event(KeyEvent.KEYCODE_N, KeyEvent.META_CTRL_ON)))
        assertEquals(KeyboardCommand.LOCK, KeyboardCommand.from(event(KeyEvent.KEYCODE_L, KeyEvent.META_CTRL_ON)))
    }

    @Test fun `typing isn't a command`() {
        assertNull(KeyboardCommand.from(event(KeyEvent.KEYCODE_N, 0)))
    }

    @Test fun `other modifiers belong to something else`() {
        assertNull(KeyboardCommand.from(event(KeyEvent.KEYCODE_S, KeyEvent.META_CTRL_ON or KeyEvent.META_SHIFT_ON)))
        assertNull(KeyboardCommand.from(event(KeyEvent.KEYCODE_S, KeyEvent.META_CTRL_ON or KeyEvent.META_ALT_ON)))
    }

    @Test fun `a key held down or let go does nothing more`() {
        assertNull(KeyboardCommand.from(event(KeyEvent.KEYCODE_N, KeyEvent.META_CTRL_ON, repeat = 1)))
        assertNull(KeyboardCommand.from(event(KeyEvent.KEYCODE_N, KeyEvent.META_CTRL_ON, action = KeyEvent.ACTION_UP)))
    }

    @Test fun `a key with no command is nothing`() {
        assertNull(KeyboardCommand.from(event(KeyEvent.KEYCODE_Q, KeyEvent.META_CTRL_ON)))
    }
}
