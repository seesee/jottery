package com.jottery.android.ui.webview

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier

/**
 * Outliner editor using the outliner.html WebView bundle.
 */
@Composable
fun WebOutlinerEditorView(
    content: String,
    fontSize: Float = 16f,
    onContentChanged: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    WebEditorView(
        htmlAsset = "outliner.html",
        content = content,
        isDark = isSystemInDarkTheme(),
        fontSize = fontSize,
        onContentChanged = onContentChanged,
        modifier = modifier,
    )
}
