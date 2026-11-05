package com.jottery.android.ui

import com.jottery.android.ui.webview.isOpenableLink
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class LinkPolicyTest {
    @Test fun `web and mail links open`() {
        listOf("http://example.com", "https://example.com/a?b=c", "HTTPS://EXAMPLE.COM", "mailto:a@b.c")
            .forEach { assertTrue(it, isOpenableLink(it)) }
    }

    @Test fun `nothing else does`() {
        listOf(
            "tel:999?secret%20note%20body",
            "intent://scan/#Intent;scheme=zxing;end",
            "file:///data/data/com.jottery.android/databases/jottery.db",
            "content://com.jottery.android.attachments/x",
            "javascript:alert(1)",
            "sms:123",
            "no-scheme-at-all",
            "",
        ).forEach { assertFalse(it, isOpenableLink(it)) }
    }
}
