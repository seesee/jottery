package com.jottery.android.ui.webview

import android.annotation.SuppressLint
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.AndroidView

/**
 * Markdown preview using the preview.html WebView bundle.
 * Content is set via the bridge; attachment references resolved via callback.
 */
@SuppressLint("SetJavaScriptEnabled")
@Composable
fun MarkdownPreviewView(
    content: String,
    isDark: Boolean = isSystemInDarkTheme(),
    fontSize: Float = 16f,
    onRequestAttachment: (String) -> Unit = {},
    modifier: Modifier = Modifier,
) {
    val bridge = WebBridge(
        onContentChanged = {},
        onReady = {},
        onOpenLink = {},
        onRequestAttachment = onRequestAttachment,
    )

    AndroidView(
        factory = { context ->
            WebView(context).apply {
                settings.javaScriptEnabled = true
                settings.domStorageEnabled = true
                settings.allowFileAccess = true

                addJavascriptInterface(bridge, "Android")

                webViewClient = object : WebViewClient() {
                    override fun onPageFinished(view: WebView?, url: String?) {
                        super.onPageFinished(view, url)
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

                loadUrl("file:///android_asset/preview.html")
            }
        },
        modifier = modifier,
    )
}
