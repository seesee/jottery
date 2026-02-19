package com.jottery.android.ui.webview

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier

/**
 * Calculator editor using the calc.html WebView bundle.
 */
@Composable
fun WebCalcEditorView(
    content: String,
    fontSize: Float = 16f,
    onContentChanged: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    WebEditorView(
        htmlAsset = "calc.html",
        content = content,
        isDark = isSystemInDarkTheme(),
        fontSize = fontSize,
        onContentChanged = onContentChanged,
        modifier = modifier,
    )
}
