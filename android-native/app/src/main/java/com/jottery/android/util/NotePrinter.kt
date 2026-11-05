package com.jottery.android.util

import android.content.Context
import android.print.PrintAttributes
import android.print.PrintManager
import android.text.TextUtils
import android.webkit.WebView
import android.webkit.WebViewClient

/**
 * Prints a note through the system's print dialog. The text is escaped, and the
 * WebView has no JavaScript, so a note can't run anything by being printed.
 */
object NotePrinter {
    /** The print job reads from this, so it has to live until the next print or lock. */
    private var webView: WebView? = null

    fun print(context: Context, title: String, text: String) {
        release()
        val view = WebView(context)
        view.settings.javaScriptEnabled = false
        view.settings.allowFileAccess = false
        view.webViewClient = object : WebViewClient() {
            override fun onPageFinished(page: WebView, url: String?) {
                val manager = context.getSystemService(PrintManager::class.java) ?: return
                manager.print(title, page.createPrintDocumentAdapter(title), PrintAttributes.Builder().build())
            }
        }
        webView = view
        val html = "<!doctype html><meta charset=\"utf-8\">" +
            "<pre style=\"white-space:pre-wrap;word-wrap:break-word;font-family:monospace\">" +
            TextUtils.htmlEncode(text) + "</pre>"
        view.loadDataWithBaseURL(null, html, "text/html", "UTF-8", null)
    }

    /** It holds decrypted text, so it goes on lock. */
    fun release() {
        webView?.destroy()
        webView = null
    }
}
