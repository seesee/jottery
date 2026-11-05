package com.jottery.android.ui.webview

import android.annotation.SuppressLint
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.ViewGroup
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.AndroidView
import com.jottery.android.ui.theme.LocalIsDarkTheme

/**
 * Android WebView wrapper matching iOS's WebEditorView.
 * Uses @JavascriptInterface for bridge communication.
 */
@SuppressLint("SetJavaScriptEnabled")
@Composable
fun WebEditorView(
    htmlAsset: String,
    content: String,
    isDark: Boolean = LocalIsDarkTheme.current,
    fontSize: Float = 16f,
    onContentChanged: (String) -> Unit,
    onReady: () -> Unit = {},
    onOpenLink: (String) -> Unit = {},
    onRequestAttachment: (String) -> Unit = {},
    modifier: Modifier = Modifier,
) {
    // Keep callback references current across recompositions.
    // The bridge is created once (remember) but callbacks may change when
    // NoteEditorView recomposes with a new note parameter. rememberUpdatedState
    // ensures the bridge always invokes the latest callback.
    val currentOnContentChanged by rememberUpdatedState(onContentChanged)
    val currentOnReady by rememberUpdatedState(onReady)
    val currentOnOpenLink by rememberUpdatedState(onOpenLink)
    val currentOnRequestAttachment by rememberUpdatedState(onRequestAttachment)

    // The latest text, read when the page finishes loading.
    val currentContent by rememberUpdatedState(content)

    // Tells the page's own edits coming back from anything else that changed the
    // note, such as a sync, a restored version or a resolved conflict. The page
    // must be told about those, or the next keystroke would write its stale buffer
    // back over the note.
    val pageState = remember { WebPageState() }

    val bridge = remember {
        WebBridge(
            onContentChanged = {
                pageState.reported(it)
                currentOnContentChanged(it)
            },
            onReady = { currentOnReady() },
            onOpenLink = { currentOnOpenLink(it) },
            onRequestAttachment = { currentOnRequestAttachment(it) },
        )
    }

    AndroidView(
        factory = { context ->
            WebView(context).apply {
                layoutParams = ViewGroup.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.MATCH_PARENT,
                )
                configureForBundle()

                addJavascriptInterface(bridge, "Android")

                webViewClient = BundleWebViewClient(
                    context = context,
                    onOpenLink = { currentOnOpenLink(it) },
                    onPageLoaded = { view ->
                        // Set initial content after page loads using JSON string
                        // escaping (handles all special characters)
                        val text = currentContent
                        val jsonContent = org.json.JSONObject.quote(text)
                        view?.evaluateJavascript(
                            "window.bridge.setContent($jsonContent, $isDark)",
                            null,
                        )
                        view?.evaluateJavascript(
                            "window.bridge.setFontSize($fontSize)",
                            null,
                        )
                        pageState.loaded(text)
                    },
                )

                loadUrl("$BUNDLE_ORIGIN$htmlAsset")
            }
        },
        onRelease = { it.release() },
        update = { webView ->
            webView.evaluateJavascript("window.bridge.setTheme($isDark)", null)
            webView.evaluateJavascript("window.bridge.setFontSize($fontSize)", null)
            if (pageState.isExternalChange(content)) {
                val jsonContent = org.json.JSONObject.quote(content)
                webView.evaluateJavascript("window.bridge.setContent($jsonContent, $isDark)", null)
            }
        },
        modifier = modifier,
    )
}

/**
 * Works out whether text arriving from Compose is news to the page. Compose runs
 * behind the page: by the time it recomposes with one keystroke, the page may have
 * reported two more, so comparing with the latest report would see a change, send
 * the older text back, and lose what was just typed. Each report is therefore
 * remembered until Compose has caught up with it. Main thread only.
 */
internal class WebPageState {
    private var loaded = false
    private var composed: String? = null
    private val unconfirmed = ArrayDeque<String>()

    fun loaded(content: String) {
        loaded = true
        composed = content
        unconfirmed.clear()
    }

    fun reported(content: String) {
        unconfirmed.addLast(content)
        if (unconfirmed.size > MAX_UNCONFIRMED) unconfirmed.removeFirst()
    }

    /** True if [content] came from somewhere other than the page. */
    fun isExternalChange(content: String): Boolean {
        if (!loaded || content == composed) return false
        composed = content

        val index = unconfirmed.lastIndexOf(content)
        if (index >= 0) {
            repeat(index + 1) { unconfirmed.removeFirst() }
            return false
        }
        unconfirmed.clear()
        return true
    }

    private companion object {
        const val MAX_UNCONFIRMED = 64
    }
}

/**
 * JavaScript bridge interface. Methods run on a background thread —
 * UI updates must be dispatched to the main thread.
 */
class WebBridge(
    private val onContentChanged: (String) -> Unit,
    private val onReady: () -> Unit,
    private val onOpenLink: (String) -> Unit,
    private val onRequestAttachment: (String) -> Unit,
) {
    private val mainHandler = Handler(Looper.getMainLooper())

    @JavascriptInterface
    fun postMessage(jsonString: String) {
        try {
            val json = kotlinx.serialization.json.Json.parseToJsonElement(jsonString)
            val obj = json.jsonObject
            when (obj["type"]?.jsonPrimitive?.content) {
                "contentChanged" -> {
                    val content = obj["content"]?.jsonPrimitive?.content ?: return
                    mainHandler.post { onContentChanged(content) }
                }
                "ready" -> mainHandler.post { onReady() }
                "requestAttachment" -> {
                    val id = obj["id"]?.jsonPrimitive?.content ?: return
                    mainHandler.post { onRequestAttachment(id) }
                }
                "openLink" -> {
                    val url = obj["url"]?.jsonPrimitive?.content ?: return
                    mainHandler.post { onOpenLink(url) }
                }
            }
        } catch (e: Exception) {
            Log.e("WebBridge", "postMessage failed: ${e.message}", e)
        }
    }
}

private val kotlinx.serialization.json.JsonElement.jsonObject
    get() = this as kotlinx.serialization.json.JsonObject

private val kotlinx.serialization.json.JsonElement.jsonPrimitive
    get() = this as kotlinx.serialization.json.JsonPrimitive
