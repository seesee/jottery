package com.jottery.android.ui.webview

import android.annotation.SuppressLint
import android.content.Context
import android.net.Uri
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.webkit.WebViewAssetLoader

/**
 * Bundled pages load from a virtual https origin through WebViewAssetLoader, so
 * the WebView needs no file or content access. A note is untrusted input, and the
 * page sits next to a bridge that can reach native code.
 */
internal const val BUNDLE_ORIGIN = "https://appassets.androidplatform.net/assets/"

/** The only link schemes a note may open. */
private val OPENABLE_SCHEMES = setOf("http", "https", "mailto")

internal fun isOpenableLink(url: String): Boolean =
    url.substringBefore(':', "").lowercase() in OPENABLE_SCHEMES

@SuppressLint("SetJavaScriptEnabled")
internal fun WebView.configureForBundle() {
    settings.javaScriptEnabled = true
    settings.domStorageEnabled = false
    settings.allowFileAccess = false
    settings.allowContentAccess = false
    settings.setGeolocationEnabled(false)
    settings.mixedContentMode = WebSettings.MIXED_CONTENT_NEVER_ALLOW
}

/** Clear a page's decrypted text and free the WebView when it leaves the screen. */
internal fun WebView.release() {
    removeJavascriptInterface("Android")
    stopLoading()
    loadUrl("about:blank")
    clearHistory()
    destroy()
}

internal class BundleWebViewClient(
    context: Context,
    private val onOpenLink: (String) -> Unit,
    private val onPageLoaded: (WebView?) -> Unit,
) : WebViewClient() {
    private val assets = WebViewAssetLoader.Builder()
        .addPathHandler("/assets/", WebViewAssetLoader.AssetsPathHandler(context))
        .build()

    override fun shouldInterceptRequest(view: WebView?, request: WebResourceRequest): WebResourceResponse? =
        assets.shouldInterceptRequest(request.url)

    override fun shouldOverrideUrlLoading(view: WebView?, request: WebResourceRequest): Boolean {
        val url: Uri = request.url
        if (url.toString().startsWith(BUNDLE_ORIGIN)) return false
        // Only a tap on the page itself may leave the app
        if (request.isForMainFrame && request.hasGesture()) onOpenLink(url.toString())
        return true
    }

    override fun onPageFinished(view: WebView?, url: String?) {
        super.onPageFinished(view, url)
        onPageLoaded(view)
    }
}
