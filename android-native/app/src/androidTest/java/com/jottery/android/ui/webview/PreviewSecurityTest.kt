package com.jottery.android.ui.webview

import android.os.Handler
import android.os.Looper
import android.webkit.WebView
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * Runs the shipped preview.html in a real WebView, set up as the app sets it up.
 * Only a device can show that script doesn't run: the sanitiser, the CSP and the
 * WebView settings all have to hold.
 */
@RunWith(AndroidJUnit4::class)
class PreviewSecurityTest {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val main = Handler(Looper.getMainLooper())

    private val links = CopyOnWriteArrayList<String>()
    private val attachments = CopyOnWriteArrayList<String>()
    private val openedNotes = CopyOnWriteArrayList<String>()

    private fun <T> onMain(block: () -> T): T {
        var result: Result<T>? = null
        instrumentation.runOnMainSync { result = runCatching(block) }
        return result!!.getOrThrow()
    }

    private fun loadPage(asset: String = "preview.html"): WebView {
        val loaded = CountDownLatch(1)
        val webView = onMain {
            val context = instrumentation.targetContext
            WebView(context).apply {
                configureForBundle()
                addJavascriptInterface(
                    WebBridge(
                        onContentChanged = {},
                        onReady = {},
                        onOpenLink = { links += it },
                        onRequestAttachment = { attachments += it },
                        onOpenNote = { openedNotes += it },
                    ),
                    "Android",
                )
                webViewClient = BundleWebViewClient(
                    context = context,
                    onOpenLink = { links += it },
                    onPageLoaded = { loaded.countDown() },
                )
                loadUrl("$BUNDLE_ORIGIN$asset")
            }
        }
        assertTrue("the page never loaded", loaded.await(30, TimeUnit.SECONDS))
        return webView
    }

    private fun evaluate(webView: WebView, script: String): String {
        val done = CountDownLatch(1)
        var value = ""
        main.post {
            webView.evaluateJavascript(script) {
                value = it ?: ""
                done.countDown()
            }
        }
        assertTrue("script timed out", done.await(15, TimeUnit.SECONDS))
        return value
    }

    private fun render(webView: WebView, markdown: String): String {
        evaluate(webView, "window.bridge.setContent(${JSONObject.quote(markdown)}, false)")
        // Long enough for anything that slipped through to fire
        Thread.sleep(700)
        // evaluateJavascript answers in JSON
        return JSONArray("[" + evaluate(webView, "document.getElementById('content').innerHTML") + "]").getString(0)
    }

    /** Every element in the rendered note that could run something. */
    private fun activeElements(webView: WebView): String = evaluate(
        webView,
        """
        Array.from(document.querySelectorAll('#content *')).filter(function (el) {
          var attributes = Array.from(el.attributes);
          return ['SCRIPT', 'IFRAME', 'OBJECT', 'EMBED', 'FORM', 'STYLE'].indexOf(el.tagName) >= 0 ||
            attributes.some(function (a) {
              return a.name.indexOf('on') === 0 || /^\s*javascript:/i.test(a.value);
            });
        }).length
        """.trimIndent(),
    )

    @Test fun theInlineBundleRunsUnderItsOwnCsp() {
        val webView = loadPage()

        assertEquals("\"function\"", evaluate(webView, "typeof window.bridge.setContent"))
        val html = render(webView, "# Title\n\n**bold** and `code`")
        assertTrue(html, html.contains("<strong>bold</strong>"))
    }

    @Test fun scriptInANoteNeverReachesTheBridge() {
        val webView = loadPage()
        val call = "Android.postMessage(JSON.stringify({type:'openLink',url:'tel:999'}))"

        val payloads = listOf(
            "<img src=x onerror=\"$call\">",
            "<script>$call</script>",
            "<svg onload=\"$call\"></svg>",
            "<iframe srcdoc=\"<script>parent.$call</script>\"></iframe>",
            "<a href=\"javascript:$call\">x</a>",
            "[link](javascript:$call)",
            "![a\" onerror=\"$call](http://example.com/a.png)",
            "![x](attachment:\"onerror=\"$call)",
            "<math><mtext></form><form><mglyph><style></math><img src onerror=\"$call\">",
        )
        for (payload in payloads) {
            val html = render(webView, payload)
            assertEquals("$payload -> $html", "0", activeElements(webView))
        }

        assertEquals(emptyList<String>(), links.toList())
    }

    @Test fun cspStopsScriptEvenIfTheSanitiserMissedIt() {
        val webView = loadPage()

        // Straight into the DOM, past the sanitiser
        evaluate(
            webView,
            "document.getElementById('content').innerHTML = " +
                "'<img src=x onerror=\"Android.postMessage(JSON.stringify({type:\\'openLink\\',url:\\'tel:1\\'}))\">'",
        )
        Thread.sleep(700)

        assertEquals(emptyList<String>(), links.toList())
    }

    @Test fun remoteImagesAreNotLoaded() {
        val webView = loadPage()

        val html = render(webView, "![pixel](https://tracker.example/p.gif)")

        assertFalse(html, html.contains("<img"))
        assertTrue(html, html.contains("blocked-image"))
    }

    @Test fun attachmentImagesStillResolve() {
        val webView = loadPage()
        val id = "11111111-1111-4111-8111-111111111111"
        val pixel = "data:image/gif;base64,R0lGODlhAQABAAAAACw="

        render(webView, "![photo](attachment:$id)")
        assertEquals(listOf(id), attachments.toList())

        evaluate(webView, "window.bridge.resolveAttachment('$id', '$pixel')")
        val src = evaluate(webView, "document.querySelector('img').getAttribute('src')")
        assertEquals("\"$pixel\"", src)
    }

    @Test fun safeLinksStayLinksAndOthersBecomeText() {
        val webView = loadPage()

        val html = render(webView, "[a](https://example.com) [b](tel:999) [c](file:///etc/passwd)")

        assertTrue(html, html.contains("href=\"https://example.com\""))
        assertFalse(html, html.contains("tel:"))
        assertFalse(html, html.contains("file:"))
    }

    @Test fun theEditorBundlesStillLoadThroughTheAssetLoader() {
        for (asset in listOf("calc.html", "outliner.html")) {
            val webView = loadPage(asset)
            assertEquals(asset, "\"function\"", evaluate(webView, "typeof window.bridge.setContent"))
        }
    }

    @Test fun theFilesystemIsOutOfReach() {
        val webView = loadPage()
        val settings = onMain { webView.settings }

        assertFalse(settings.allowFileAccess)
        assertFalse(settings.allowContentAccess)
    }

    // MARK: - Note links

    private val linkedId = "22222222-2222-4222-8222-222222222222"

    private fun renderLinked(webView: WebView, content: String) {
        val titles = org.json.JSONObject(mapOf(linkedId to "Shopping list")).toString()
        evaluate(webView, "window.bridge.setNoteTitles($titles)")
        evaluate(webView, "window.bridge.setContent(${org.json.JSONObject.quote(content)}, false)")
    }

    private fun waitForOpenedNote(): String {
        repeat(200) {
            openedNotes.firstOrNull()?.let { return it }
            Thread.sleep(25)
        }
        throw AssertionError("the note link never reached the native side")
    }

    @Test fun aWikiLinkByTitleOpensItsNote() {
        val webView = loadPage()
        renderLinked(webView, "See [[Shopping list]]")

        assertTrue(evaluate(webView, "document.body.innerText").contains("Shopping list"))
        evaluate(webView, "document.querySelector('.note-link').click()")

        assertEquals(linkedId, waitForOpenedNote())
    }

    @Test fun aCopiedLinkShowsItsNotesTitleAndOpensIt() {
        val webView = loadPage()
        renderLinked(webView, "[](link:$linkedId)")

        assertTrue(evaluate(webView, "document.querySelector('.note-link').textContent").contains("Shopping list"))
        evaluate(webView, "document.querySelector('.note-link').click()")

        assertEquals(linkedId, waitForOpenedNote())
    }

    @Test fun aLinkToNoNoteIsShownButGoesNowhere() {
        val webView = loadPage()
        renderLinked(webView, "[[Nothing by this name]]")

        assertEquals("\"true\"", evaluate(webView, "String(!!document.querySelector('.note-link.missing'))"))
        evaluate(webView, "document.querySelector('.note-link').click()")
        Thread.sleep(200)

        assertTrue(openedNotes.isEmpty())
    }

    @Test fun aTitleCantCarryScriptIntoThePage() {
        val webView = loadPage()
        val hostile = "<img src=x onerror=\"Android.postMessage('{\\\"type\\\":\\\"openLink\\\",\\\"url\\\":\\\"https://evil\\\"}')\">"
        val titles = org.json.JSONObject(mapOf(linkedId to hostile)).toString()
        evaluate(webView, "window.bridge.setNoteTitles($titles)")
        evaluate(webView, "window.bridge.setContent(${org.json.JSONObject.quote("[](link:$linkedId)")}, false)")
        Thread.sleep(300)

        assertTrue(links.isEmpty())
        assertEquals("\"0\"", evaluate(webView, "String(document.querySelectorAll('img[onerror]').length)"))
    }

    // MARK: - Read-only editors

    @Test fun aLockedCalcNoteTakesNoTyping() {
        val webView = loadPage("calc.html")
        evaluate(webView, "window.bridge.setContent('1 + 1', false)")

        assertEquals("\"true\"", evaluate(webView, "document.querySelector('.cm-content').getAttribute('contenteditable')"))
        evaluate(webView, "window.bridge.setReadOnly(true)")
        assertEquals("\"false\"", evaluate(webView, "document.querySelector('.cm-content').getAttribute('contenteditable')"))

        evaluate(webView, "window.bridge.setReadOnly(false)")
        assertEquals("\"true\"", evaluate(webView, "document.querySelector('.cm-content').getAttribute('contenteditable')"))
    }

    @Test fun aLockedCalcNoteStaysLockedAfterTheFontChanges() {
        val webView = loadPage("calc.html")
        evaluate(webView, "window.bridge.setContent('1 + 1', false)")
        evaluate(webView, "window.bridge.setReadOnly(true)")

        // Changing the size rebuilds the whole editor
        evaluate(webView, "window.bridge.setFontSize(20)")

        assertEquals("\"false\"", evaluate(webView, "document.querySelector('.cm-content').getAttribute('contenteditable')"))
    }

    @Test fun aLockedOutlinerNoteTakesNoTyping() {
        val webView = loadPage("outliner.html")
        evaluate(webView, "window.bridge.setContent('- one', false)")

        assertEquals("\"true\"", evaluate(webView, "document.querySelector('.outliner-content').contentEditable"))
        evaluate(webView, "window.bridge.setReadOnly(true)")
        assertEquals("\"false\"", evaluate(webView, "document.querySelector('.outliner-content').contentEditable"))
    }
}
