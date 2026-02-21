package com.jottery.android.ui.webview

import android.annotation.SuppressLint
import android.view.ViewGroup
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.AndroidView
import com.jottery.android.ui.theme.LocalIsDarkTheme

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

    // Keep a reference to the WebView so we can call resolveAttachment from the callback
    val webViewRef = remember { arrayOfNulls<WebView>(1) }

    val bridge = remember {
        WebBridge(
            onContentChanged = {},
            onReady = {},
            onOpenLink = { currentOnOpenLink(it) },
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
                settings.javaScriptEnabled = true
                settings.domStorageEnabled = true
                settings.allowFileAccess = true

                addJavascriptInterface(bridge, "Android")

                webViewClient = object : WebViewClient() {
                    override fun shouldOverrideUrlLoading(
                        view: WebView?,
                        request: WebResourceRequest?,
                    ): Boolean {
                        request?.url?.toString()?.let { currentOnOpenLink(it) }
                        return true
                    }

                    override fun onPageFinished(view: WebView?, url: String?) {
                        super.onPageFinished(view, url)
                        val jsonContent = org.json.JSONObject.quote(content)
                        view?.evaluateJavascript(
                            "window.bridge.setContent($jsonContent, $isDark)",
                            null,
                        )
                        view?.evaluateJavascript(
                            "window.bridge.setFontSize($fontSize)",
                            null,
                        )
                    }
                }

                webViewRef[0] = this
                loadUrl("file:///android_asset/preview.html")
            }
        },
        update = { webView ->
            webView.evaluateJavascript("window.bridge.setTheme($isDark)", null)
            webView.evaluateJavascript("window.bridge.setFontSize($fontSize)", null)
            val jsonContent = org.json.JSONObject.quote(content)
            webView.evaluateJavascript(
                "window.bridge.setContent($jsonContent, $isDark)",
                null,
            )
        },
        modifier = modifier,
    )
}
