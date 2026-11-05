package com.jottery.android.util

import android.net.Uri
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class QuickActionTest {
    @Test fun `the launcher's two shortcuts are understood`() {
        assertEquals(QuickAction.NEW_NOTE, QuickAction.from(Uri.parse("jottery://new_note")))
        assertEquals(QuickAction.SEARCH, QuickAction.from(Uri.parse("jottery://search")))
    }

    @Test fun `anything else is nothing, because any app can send this activity a link`() {
        assertNull(QuickAction.from(null))
        assertNull(QuickAction.from(Uri.parse("jottery://wipe")))
        assertNull(QuickAction.from(Uri.parse("https://new_note")))
        assertNull(QuickAction.from(Uri.parse("jottery:new_note")))
    }
}
