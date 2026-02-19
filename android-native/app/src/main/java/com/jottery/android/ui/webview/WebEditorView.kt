package com.jottery.android.ui.webview

import android.annotation.SuppressLint
import android.webkit.JavascriptInterface
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.AndroidView

/**
 * Android WebView wrapper matching iOS's WebEditorView.
 * Uses @JavascriptInterface for bridge communication.
 */
@SuppressLint("SetJavaScriptEnabled")
@Composable
fun WebEditorView(
    htmlAsset: String,
    content: String,
    isDark: Boolean = isSystemInDarkTheme(),
    fontSize: Float = 16f,
    onContentChanged: (String) -> Unit,
    onReady: () -> Unit = {},
    onOpenLink: (String) -> Unit = {},
    onRequestAttachment: (String) -> Unit = {},
    modifier: Modifier = Modifier,
) {
    val bridge = remember {
        WebBridge(
            onContentChanged = onContentChanged,
            onReady = onReady,
            onOpenLink = onOpenLink,
            onRequestAttachment = onRequestAttachment,
        )
    }

    AndroidView(
        factory = { context ->
            WebView(context).apply {
                settings.javaScriptEnabled = true
                settings.domStorageEnabled = true
                settings.allowFileAccess = true

                addJavascriptInterface(bridge, "Android")

                webViewClient = object : WebViewClient() {
                    override fun shouldOverrideUrlLoading(
                        view: WebView?,
                        request: WebResourceRequest?,
                    ): Boolean {
                        request?.url?.toString()?.let { onOpenLink(it) }
                        return true
                    }

                    override fun onPageFinished(view: WebView?, url: String?) {
                        super.onPageFinished(view, url)
                        // Set initial content after page loads
                        val escapedContent = content
                            .replace("\\", "\\\\")
                            .replace("\"", "\\\"")
                            .replace("\n", "\\n")
                            .replace("\r", "\\r")
                        view?.evaluateJavascript(
                            "window.bridge.setContent(\"$escapedContent\", $isDark)",
                            null,
                        )
                        view?.evaluateJavascript(
                            "window.bridge.setFontSize($fontSize)",
                            null,
                        )
                    }
                }

                loadUrl("file:///android_asset/$htmlAsset")
            }
        },
        update = { webView ->
            // Update theme if it changes
            webView.evaluateJavascript("window.bridge.setTheme($isDark)", null)
        },
        modifier = modifier,
    )
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
    @JavascriptInterface
    fun postMessage(jsonString: String) {
        try {
            val json = kotlinx.serialization.json.Json.parseToJsonElement(jsonString)
            val obj = json.jsonObject
            when (obj["type"]?.jsonPrimitive?.content) {
                "contentChanged" -> {
                    val content = obj["content"]?.jsonPrimitive?.content ?: return
                    onContentChanged(content)
                }
                "ready" -> onReady()
                "requestAttachment" -> {
                    val id = obj["id"]?.jsonPrimitive?.content ?: return
                    onRequestAttachment(id)
                }
                "openLink" -> {
                    val url = obj["url"]?.jsonPrimitive?.content ?: return
                    onOpenLink(url)
                }
            }
        } catch (_: Exception) {
            // Invalid JSON — ignore
        }
    }
}

private val kotlinx.serialization.json.JsonElement.jsonObject
    get() = this as kotlinx.serialization.json.JsonObject

private val kotlinx.serialization.json.JsonElement.jsonPrimitive
    get() = this as kotlinx.serialization.json.JsonPrimitive
