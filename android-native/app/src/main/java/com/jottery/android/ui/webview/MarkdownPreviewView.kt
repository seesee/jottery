package com.jottery.android.ui.webview

import android.annotation.SuppressLint
import android.os.SystemClock
import android.view.MotionEvent
import android.view.ViewGroup
import android.webkit.WebView
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.AndroidView
import com.jottery.android.ui.theme.LocalIsDarkTheme

private const val LINK_TAP_WINDOW_MS = 1_500L

/**
 * Markdown preview using the preview.html WebView bundle.
 * Content is set via the bridge; attachment references resolved via callback.
 */
@SuppressLint("SetJavaScriptEnabled")
@Composable
fun MarkdownPreviewView(
    content: String,
    isDark: Boolean = LocalIsDarkTheme.current,
    fontSize: Float = 16f,
    onRequestAttachment: (id: String, callback: (String) -> Unit) -> Unit = { _, _ -> },
    onOpenLink: (String) -> Unit = {},
    modifier: Modifier = Modifier,
) {
    val currentOnRequestAttachment by rememberUpdatedState(onRequestAttachment)
    val currentOnOpenLink by rememberUpdatedState(onOpenLink)
    val currentContent by rememberUpdatedState(content)
    val currentIsDark by rememberUpdatedState(isDark)
    val currentFontSize by rememberUpdatedState(fontSize)
    val pageState = remember { WebPageState() }

    // Keep a reference to the WebView so we can call resolveAttachment from the callback
    val webViewRef = remember { arrayOfNulls<WebView>(1) }
    val lastTouch = remember { longArrayOf(0L) }

    val bridge = remember {
        WebBridge(
            onContentChanged = {},
            onReady = {},
            // The page has no way to prove a tap, so the native side checks for one:
            // a link can only open just after the reader touched the preview.
            onOpenLink = { url ->
                if (SystemClock.uptimeMillis() - lastTouch[0] < LINK_TAP_WINDOW_MS) {
                    currentOnOpenLink(url)
                }
            },
            onRequestAttachment = { id ->
                currentOnRequestAttachment(id) { dataUrl ->
                    val wv = webViewRef[0] ?: return@currentOnRequestAttachment
                    val escapedId = org.json.JSONObject.quote(id)
                    val escapedUrl = org.json.JSONObject.quote(dataUrl)
                    wv.evaluateJavascript(
                        "window.bridge.resolveAttachment($escapedId, $escapedUrl)",
                        null,
                    )
                }
            },
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

                setOnTouchListener { view, event ->
                    lastTouch[0] = SystemClock.uptimeMillis()
                    if (event.action == MotionEvent.ACTION_UP) view.performClick()
                    false
                }

                webViewClient = BundleWebViewClient(
                    context = context,
                    onOpenLink = { currentOnOpenLink(it) },
                    onPageLoaded = { view ->
                        val text = currentContent
                        val dark = currentIsDark
                        val size = currentFontSize
                        val jsonContent = org.json.JSONObject.quote(text)
                        view?.evaluateJavascript(
                            "window.bridge.setContent($jsonContent, $dark)",
                            null,
                        )
                        view?.evaluateJavascript(
                            "window.bridge.setFontSize($size)",
                            null,
                        )
                        pageState.loaded(text, dark, size)
                    },
                )

                webViewRef[0] = this
                loadUrl("${BUNDLE_ORIGIN}preview.html")
            }
        },
        onRelease = {
            webViewRef[0] = null
            it.release()
        },
        update = { webView ->
            if (pageState.themeChanged(isDark)) {
                webView.evaluateJavascript("window.bridge.setTheme($isDark)", null)
            }
            if (pageState.fontSizeChanged(fontSize)) {
                webView.evaluateJavascript("window.bridge.setFontSize($fontSize)", null)
            }
            // The preview never edits, so any change comes from outside
            if (pageState.isExternalChange(content)) {
                val jsonContent = org.json.JSONObject.quote(content)
                webView.evaluateJavascript("window.bridge.setContent($jsonContent, $isDark)", null)
            }
        },
        modifier = modifier,
    )
}
