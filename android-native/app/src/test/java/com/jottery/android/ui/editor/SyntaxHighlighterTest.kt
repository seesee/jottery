package com.jottery.android.ui.editor

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SyntaxHighlighterTest {
    private fun kinds(text: String, language: String) =
        SyntaxHighlighter.tokens(text, language).map { text.substring(it.start, it.end) to it.kind }

    @Test fun `python keywords, strings, numbers and comments are found`() {
        assertEquals(
            listOf(
                "def" to TokenKind.KEYWORD,
                "return" to TokenKind.KEYWORD,
                "\"hi\"" to TokenKind.STRING,
                "42" to TokenKind.NUMBER,
                "# done" to TokenKind.COMMENT,
            ),
            kinds("def f():\n    return \"hi\", 42  # done", "python"),
        )
    }

    @Test fun `a keyword inside a string or comment isn't one`() {
        assertEquals(listOf("\"if while\"" to TokenKind.STRING), kinds("\"if while\"", "javascript"))
        assertEquals(listOf("// if while" to TokenKind.COMMENT), kinds("// if while", "javascript"))
    }

    @Test fun `a word containing a keyword isn't one`() {
        assertTrue(kinds("iffy format", "python").isEmpty())
    }

    @Test fun `a triple-quoted python string spans lines`() {
        val text = "\"\"\"one\ntwo\"\"\" + 1"
        assertEquals("\"\"\"one\ntwo\"\"\"" to TokenKind.STRING, kinds(text, "python").first())
    }

    @Test fun `an unclosed string stops at the end of its line`() {
        assertEquals(listOf("\"oops" to TokenKind.STRING, "if" to TokenKind.KEYWORD), kinds("\"oops\nif", "javascript"))
    }

    @Test fun `an escaped quote doesn't end a string`() {
        assertEquals(listOf("\"a\\\"b\"" to TokenKind.STRING), kinds("\"a\\\"b\"", "javascript"))
    }

    @Test fun `sql keywords ignore case`() {
        assertEquals(listOf("SELECT" to TokenKind.KEYWORD, "from" to TokenKind.KEYWORD), kinds("SELECT x from y", "sql"))
    }

    @Test fun `xml tags, attribute values and comments are found`() {
        assertEquals(
            listOf(
                "<!-- c -->" to TokenKind.COMMENT,
                "<a" to TokenKind.MARKUP,
                "\"v\"" to TokenKind.STRING,
                "</a" to TokenKind.MARKUP,
            ),
            kinds("<!-- c --><a b=\"v\">text</a>", "xml"),
        )
    }

    @Test fun `markdown headings, code and fences are found`() {
        val tokens = kinds("# Title\nplain `code` **bold**\n```\nfenced\n```", "markdown")

        assertEquals("# Title" to TokenKind.HEADING, tokens[0])
        assertEquals("`code`" to TokenKind.STRING, tokens[1])
        assertEquals("**bold**" to TokenKind.KEYWORD, tokens[2])
        assertEquals("fenced" to TokenKind.STRING, tokens[4])
    }

    @Test fun `plain text and unknown languages have nothing to colour`() {
        assertTrue(SyntaxHighlighter.tokens("if while 42", "plain").isEmpty())
        assertTrue(SyntaxHighlighter.tokens("if while 42", "calc").isEmpty())
    }

    @Test fun `a very long note goes without, rather than slow every keystroke`() {
        assertTrue(SyntaxHighlighter.tokens("if ".repeat(SyntaxHighlighter.MAX_CHARS), "python").isEmpty())
    }

    @Test fun `tokens stay inside the text and in order, whatever it holds`() {
        val samples = listOf("", "\"", "/*", "<", "<a", "#", "```", "'''", "\\", "1.2.3", "<!--", "\"\\")
        for (language in listOf("python", "javascript", "typescript", "perl", "bash", "sql", "json", "css", "xml", "markdown")) {
            for (text in samples) {
                var end = 0
                for (token in SyntaxHighlighter.tokens(text, language)) {
                    assertTrue("$language: $text", token.start >= end && token.end <= text.length && token.start < token.end)
                    end = token.end
                }
            }
        }
    }
}
