package com.jottery.android.ui.editor

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.OffsetMapping
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.input.TransformedText
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import com.jottery.android.R

/** The colours a note's text is drawn in, from the theme. */
data class SyntaxPalette(
    val keyword: Color,
    val string: Color,
    val comment: Color,
    val number: Color,
    val match: Color,
)

/** Colours the text without changing it, so every offset still lines up. */
private class SyntaxTransformation(
    private val language: String,
    private val palette: SyntaxPalette,
    private val matches: List<IntRange>,
) : VisualTransformation {

    /** Compose calls this on every recomposition, not only when the text changes. */
    private var lastText: String? = null
    private var lastResult: TransformedText? = null

    override fun filter(text: AnnotatedString): TransformedText {
        if (text.text == lastText) lastResult?.let { return it }
        val styled = AnnotatedString.Builder(text.text).apply {
            for (token in SyntaxHighlighter.tokens(text.text, language)) {
                addStyle(style(token.kind), token.start, token.end)
            }
            for (match in matches) {
                if (match.last < text.length) addStyle(SpanStyle(background = palette.match), match.first, match.last + 1)
            }
        }.toAnnotatedString()
        return TransformedText(styled, OffsetMapping.Identity).also {
            lastText = text.text
            lastResult = it
        }
    }

    private fun style(kind: TokenKind) = when (kind) {
        TokenKind.KEYWORD, TokenKind.MARKUP -> SpanStyle(color = palette.keyword)
        TokenKind.STRING -> SpanStyle(color = palette.string)
        TokenKind.COMMENT -> SpanStyle(color = palette.comment, fontStyle = FontStyle.Italic)
        TokenKind.NUMBER -> SpanStyle(color = palette.number)
        TokenKind.HEADING -> SpanStyle(color = palette.keyword, fontWeight = FontWeight.Bold)
    }
}

/**
 * The note's text. A Compose text field rather than a code editor view, so selection
 * handles, the keyboard, spell check and accessibility all stay the platform's own.
 */
@Composable
fun EditorTextArea(
    value: TextFieldValue,
    onValueChange: (TextFieldValue) -> Unit,
    readOnly: Boolean,
    language: String,
    wordWrap: Boolean,
    fontSize: TextUnit,
    matches: List<IntRange>,
    focusRequester: FocusRequester,
    modifier: Modifier = Modifier,
) {
    val colours = MaterialTheme.colorScheme
    val palette = SyntaxPalette(
        keyword = colours.primary,
        string = colours.tertiary,
        comment = colours.onSurfaceVariant,
        number = colours.secondary,
        match = colours.tertiaryContainer,
    )
    val transformation = remember(language, palette, matches) { SyntaxTransformation(language, palette, matches) }
    val textStyle = TextStyle(
        fontFamily = FontFamily.Monospace,
        fontSize = fontSize,
        color = colours.onSurface,
        lineHeight = fontSize * 1.5f,
    )

    BoxWithConstraints(modifier = modifier) {
        val available = maxWidth
        // Without a width to wrap at, each line runs on and the field scrolls sideways
        val layout = if (wordWrap) {
            Modifier.fillMaxSize()
        } else {
            Modifier
                .fillMaxHeight()
                .horizontalScroll(rememberScrollState())
                .widthIn(min = available)
        }
        BasicTextField(
            value = value,
            onValueChange = onValueChange,
            readOnly = readOnly,
            textStyle = textStyle,
            visualTransformation = transformation,
            cursorBrush = SolidColor(colours.primary),
            modifier = layout
                .focusRequester(focusRequester)
                .padding(horizontal = 16.dp, vertical = 12.dp),
            decorationBox = { innerTextField ->
                Box {
                    if (value.text.isEmpty() && !readOnly) {
                        Text(
                            text = stringResource(R.string.editor_placeholder),
                            style = textStyle.copy(color = colours.onSurfaceVariant.copy(alpha = 0.4f)),
                        )
                    }
                    innerTextField()
                }
            },
        )
    }
}
