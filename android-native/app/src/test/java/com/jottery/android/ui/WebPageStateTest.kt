package com.jottery.android.ui

import com.jottery.android.ui.webview.WebPageState
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class WebPageStateTest {
    private val page = WebPageState().apply { loaded("start") }

    @Test fun `nothing is external before the page has loaded`() {
        assertFalse(WebPageState().isExternalChange("anything"))
    }

    @Test fun `theme and font go to the page only when they change`() {
        val state = WebPageState().apply { loaded("x", isDark = false, size = 14f) }

        assertFalse("recomposing must not rebuild the calc editor", state.themeChanged(false))
        assertFalse(state.fontSizeChanged(14f))

        assertTrue(state.themeChanged(true))
        assertFalse(state.themeChanged(true))
        assertTrue(state.fontSizeChanged(16f))
        assertFalse(state.fontSizeChanged(16f))
    }

    @Test fun `the same text again is no change`() {
        assertFalse(page.isExternalChange("start"))
    }

    @Test fun `a change from elsewhere is external`() {
        assertTrue(page.isExternalChange("from a sync"))
    }

    @Test fun `the page's own edit coming back is not`() {
        page.reported("t")
        assertFalse(page.isExternalChange("t"))
    }

    @Test fun `Compose running behind fast typing never sends the page old text`() {
        page.reported("t")
        page.reported("ty")
        page.reported("typ")

        assertFalse(page.isExternalChange("t"))
        assertFalse(page.isExternalChange("typ"))
    }

    @Test fun `a recomposition skipping a report is still an echo`() {
        page.reported("a")
        page.reported("ab")
        page.reported("abc")

        assertFalse(page.isExternalChange("ab"))
        assertFalse(page.isExternalChange("abc"))
    }

    @Test fun `an outside change while edits are pending is external`() {
        page.reported("t")

        assertTrue(page.isExternalChange("from a sync"))
    }
}
