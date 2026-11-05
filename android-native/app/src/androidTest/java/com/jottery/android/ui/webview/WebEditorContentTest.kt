package com.jottery.android.ui.webview

import android.view.KeyCharacterMap
import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.json.JSONArray
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * The web editors keep their own buffer, so anything that changes a note from
 * outside must reach the page, or the next keystroke writes the old buffer back.
 */
@RunWith(AndroidJUnit4::class)
class WebEditorContentTest {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private lateinit var scenario: ActivityScenario<ComponentActivity>
    private lateinit var activity: ComponentActivity

    @Before fun setUp() {
        scenario = ActivityScenario.launch(ComponentActivity::class.java)
        scenario.onActivity { activity = it }
    }

    @After fun tearDown() = scenario.close()

    private fun findWebView(view: View): WebView? = when (view) {
        is WebView -> view
        is ViewGroup -> (0 until view.childCount).firstNotNullOfOrNull { findWebView(view.getChildAt(it)) }
        else -> null
    }

    private fun evaluate(script: String): String {
        val done = CountDownLatch(1)
        var value = "null"
        activity.runOnUiThread {
            val webView = findWebView(activity.window.decorView)
            if (webView == null) {
                done.countDown()
            } else {
                webView.evaluateJavascript(script) {
                    value = it ?: "null"
                    done.countDown()
                }
            }
        }
        assertTrue(done.await(10, TimeUnit.SECONDS))
        return JSONArray("[$value]").get(0).toString()
    }

    private fun pageText() = evaluate("(document.querySelector('.cm-content') || document.body).textContent")

    private fun waitFor(what: String, condition: () -> Boolean) {
        repeat(150) {
            if (condition()) return
            Thread.sleep(100)
        }
        throw AssertionError("Timed out waiting for $what; page says: ${pageText()}")
    }

    @Test fun textChangedOutsideTheEditorReachesThePage() {
        var content by mutableStateOf("first text")
        instrumentation.runOnMainSync {
            activity.setContent {
                WebCalcEditorView(
                    content = content,
                    fontSize = 14f,
                    onContentChanged = { content = it },
                    modifier = Modifier.fillMaxSize(),
                )
            }
        }
        waitFor("the first text") { pageText().contains("first text") }

        instrumentation.runOnMainSync { content = "text from a sync" }

        waitFor("the new text") { pageText().contains("text from a sync") }
        assertTrue(!pageText().contains("first text"))
    }

    @Test fun typingIsReportedAndNotSentBack() {
        var content by mutableStateOf("")
        val reported = CopyOnWriteArrayList<String>()
        instrumentation.runOnMainSync {
            activity.setContent {
                WebCalcEditorView(
                    content = content,
                    fontSize = 14f,
                    onContentChanged = {
                        reported += it
                        content = it
                    },
                    modifier = Modifier.fillMaxSize(),
                )
            }
        }
        waitFor("the editor") { evaluate("document.querySelector('.cm-content') != null") == "true" }

        instrumentation.runOnMainSync { findWebView(activity.window.decorView)!!.requestFocus() }

        evaluate("document.querySelector('.cm-content').focus()")
        Thread.sleep(300)

        // Faster than Compose can recompose. Sent straight to the WebView, because
        // the input system drops injected keys when the soft keyboard appears.
        val typed = "qzxwvkqzxwvkqzxwvkqzxwvkqzxwvk"
        val keys = KeyCharacterMap.load(KeyCharacterMap.VIRTUAL_KEYBOARD).getEvents(typed.toCharArray())
        for (key in keys) {
            activity.runOnUiThread { findWebView(activity.window.decorView)!!.dispatchKeyEvent(key) }
            Thread.sleep(2)
        }

        repeat(100) {
            if (reported.lastOrNull() == typed) return@repeat
            Thread.sleep(100)
        }
        assertEquals("page: ${pageText()} reports: ${reported.takeLast(3)}", typed, reported.lastOrNull())
        Thread.sleep(500)
        assertTrue(pageText(), pageText().startsWith(typed))
    }
}
