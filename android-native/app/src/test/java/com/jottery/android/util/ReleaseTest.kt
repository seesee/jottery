package com.jottery.android.util

import com.jottery.android.BuildConfig
import com.jottery.android.service.RepoFile
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import java.security.MessageDigest
import java.util.Base64

/** What a release has to get right, and nothing used to check. */
class ReleaseTest {
    @Test fun `the app's version is the release's`() {
        val version = Regex(""""version"\s*:\s*"([^"]+)"""").find(RepoFile.at("package.json").readText())!!.groupValues[1]

        assertEquals(version, BuildConfig.VERSION_NAME)
    }

    @Test fun `an update can install over what came before`() {
        // 1 was the only version code ever used before this was derived
        assertTrue(BuildConfig.VERSION_CODE > 1)
    }

    @Test fun `the WebView pages are the ones the bundles built`() {
        // A fix to a bundle, such as to the preview's sanitiser, has to reach the app
        for (page in listOf("calc.html", "outliner.html", "preview.html")) {
            assertArrayEquals(
                "$page differs: copy it from webview-bundles/dist",
                RepoFile.at("webview-bundles/dist/$page").readBytes(),
                File("src/main/assets/$page").readBytes(),
            )
        }
    }

    @Test fun `the preview's content security policy allows its own script`() {
        val html = File("src/main/assets/preview.html").readText()
        val script = Regex("""<script(?:\s[^>]*)?>(.*?)</script>""", RegexOption.DOT_MATCHES_ALL).find(html)!!.groupValues[1]
        val hash = Base64.getEncoder().encodeToString(MessageDigest.getInstance("SHA-256").digest(script.toByteArray()))

        assertTrue("the page wouldn't run at all", "script-src 'sha256-$hash'" in html)
    }
}
