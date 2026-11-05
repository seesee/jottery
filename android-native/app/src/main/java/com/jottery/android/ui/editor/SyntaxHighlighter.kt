package com.jottery.android.ui.editor

/** What a stretch of text is, for colouring. */
enum class TokenKind { KEYWORD, STRING, COMMENT, NUMBER, MARKUP, HEADING }

data class Token(val start: Int, val end: Int, val kind: TokenKind)

/**
 * Finds what to colour in a note. A single pass over the text with a small grammar
 * per language: enough to read code by, without a parser or a dependency to ship.
 * Anything it doesn't recognise is simply left plain.
 */
object SyntaxHighlighter {

    /** Colouring runs on each keystroke, so a very long note goes without it. */
    const val MAX_CHARS = 100_000

    private class Grammar(
        val keywords: Set<String>,
        val lineComments: List<String> = emptyList(),
        val blockComment: Pair<String, String>? = null,
        val quotes: String = "\"'",
        val tripleQuotes: Boolean = false,
        val ignoreCase: Boolean = false,
    )

    private val JS_KEYWORDS = setOf(
        "async", "await", "break", "case", "catch", "class", "const", "continue", "default", "delete", "do",
        "else", "export", "extends", "false", "finally", "for", "from", "function", "if", "import", "in",
        "instanceof", "let", "new", "null", "of", "return", "static", "super", "switch", "this", "throw",
        "true", "try", "typeof", "undefined", "var", "void", "while", "yield",
    )

    private val grammars = mapOf(
        "javascript" to Grammar(JS_KEYWORDS, listOf("//"), "/*" to "*/", "\"'`"),
        "typescript" to Grammar(
            JS_KEYWORDS + setOf(
                "abstract", "any", "as", "boolean", "declare", "enum", "implements", "interface", "keyof",
                "namespace", "never", "number", "private", "protected", "public", "readonly", "string",
                "type", "unknown",
            ),
            listOf("//"), "/*" to "*/", "\"'`",
        ),
        "python" to Grammar(
            setOf(
                "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue",
                "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import",
                "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while",
                "with", "yield",
            ),
            listOf("#"), tripleQuotes = true,
        ),
        "perl" to Grammar(
            setOf(
                "die", "do", "else", "elsif", "eval", "for", "foreach", "if", "last", "local", "my", "next",
                "our", "package", "print", "require", "return", "shift", "sub", "unless", "until", "use",
                "while",
            ),
            listOf("#"),
        ),
        "bash" to Grammar(
            setOf(
                "case", "do", "done", "echo", "elif", "else", "esac", "exit", "export", "fi", "for", "function",
                "if", "in", "local", "read", "return", "set", "shift", "then", "until", "while",
            ),
            listOf("#"),
        ),
        "sql" to Grammar(
            setOf(
                "add", "all", "alter", "and", "as", "asc", "between", "by", "case", "create", "default", "delete",
                "desc", "distinct", "drop", "else", "end", "exists", "foreign", "from", "group", "having", "in",
                "index", "inner", "insert", "into", "is", "join", "key", "left", "like", "limit", "not", "null",
                "offset", "on", "or", "order", "outer", "primary", "references", "right", "select", "set",
                "table", "then", "union", "unique", "update", "values", "when", "where",
            ),
            listOf("--"), "/*" to "*/", ignoreCase = true,
        ),
        "json" to Grammar(setOf("true", "false", "null"), quotes = "\""),
        "css" to Grammar(emptySet(), blockComment = "/*" to "*/"),
    )

    /** Tokens in order, not overlapping. Empty for a language with nothing to colour. */
    fun tokens(text: String, language: String): List<Token> {
        if (text.length > MAX_CHARS) return emptyList()
        return when (language) {
            "markdown" -> markdown(text)
            "xml" -> markup(text)
            else -> grammars[language]?.let { code(text, it) } ?: emptyList()
        }
    }

    private fun code(text: String, grammar: Grammar): List<Token> {
        val tokens = mutableListOf<Token>()
        var i = 0
        while (i < text.length) {
            val c = text[i]
            val block = grammar.blockComment
            when {
                grammar.lineComments.any { text.startsWith(it, i) } -> {
                    val end = text.indexOf('\n', i).let { if (it < 0) text.length else it }
                    tokens += Token(i, end, TokenKind.COMMENT)
                    i = end
                }
                block != null && text.startsWith(block.first, i) -> {
                    val close = text.indexOf(block.second, i + block.first.length)
                    val end = if (close < 0) text.length else close + block.second.length
                    tokens += Token(i, end, TokenKind.COMMENT)
                    i = end
                }
                c in grammar.quotes -> {
                    val end = stringEnd(text, i, grammar.tripleQuotes)
                    tokens += Token(i, end, TokenKind.STRING)
                    i = end
                }
                c.isDigit() && (i == 0 || !isWordChar(text[i - 1])) -> {
                    var end = i + 1
                    while (end < text.length && (text[end].isLetterOrDigit() || text[end] == '.')) end++
                    tokens += Token(i, end, TokenKind.NUMBER)
                    i = end
                }
                isWordChar(c) -> {
                    var end = i + 1
                    while (end < text.length && isWordChar(text[end])) end++
                    val word = text.substring(i, end)
                    if ((if (grammar.ignoreCase) word.lowercase() else word) in grammar.keywords) {
                        tokens += Token(i, end, TokenKind.KEYWORD)
                    }
                    i = end
                }
                else -> i++
            }
        }
        return tokens
    }

    /** Where the string opening at [start] ends. An unclosed one stops at the line's end. */
    private fun stringEnd(text: String, start: Int, tripleQuotes: Boolean): Int {
        val quote = text[start]
        if (tripleQuotes) {
            val triple = quote.toString().repeat(3)
            if (text.startsWith(triple, start)) {
                val close = text.indexOf(triple, start + 3)
                return if (close < 0) text.length else close + 3
            }
        }
        var i = start + 1
        while (i < text.length) {
            when (text[i]) {
                '\\' -> i++
                quote -> return i + 1
                '\n' -> if (quote != '`') return i
            }
            i++
        }
        return text.length
    }

    private fun isWordChar(c: Char) = c.isLetterOrDigit() || c == '_' || c == '$'

    private fun markup(text: String): List<Token> {
        val tokens = mutableListOf<Token>()
        var i = 0
        while (i < text.length) {
            when {
                text.startsWith("<!--", i) -> {
                    val close = text.indexOf("-->", i + 4)
                    val end = if (close < 0) text.length else close + 3
                    tokens += Token(i, end, TokenKind.COMMENT)
                    i = end
                }
                text[i] == '<' -> i = tag(text, i, tokens)
                else -> i++
            }
        }
        return tokens
    }

    /** Colours one tag's name and quoted attributes, and returns where it ends. */
    private fun tag(text: String, start: Int, tokens: MutableList<Token>): Int {
        var i = start + 1
        while (i < text.length && text[i] in "/?!") i++
        val nameStart = i
        while (i < text.length && (text[i].isLetterOrDigit() || text[i] in ":-_.")) i++
        if (i > nameStart) tokens += Token(start, i, TokenKind.MARKUP)
        while (i < text.length && text[i] != '>') {
            if (text[i] == '"' || text[i] == '\'') {
                val end = stringEnd(text, i, tripleQuotes = false)
                tokens += Token(i, end, TokenKind.STRING)
                i = end
            } else {
                i++
            }
        }
        return (i + 1).coerceAtMost(text.length)
    }

    private fun markdown(text: String): List<Token> {
        val tokens = mutableListOf<Token>()
        var lineStart = 0
        var inFence = false
        while (lineStart <= text.length) {
            val lineEnd = text.indexOf('\n', lineStart).let { if (it < 0) text.length else it }
            val line = text.substring(lineStart, lineEnd)
            when {
                line.trimStart().startsWith("```") -> {
                    inFence = !inFence
                    tokens += Token(lineStart, lineEnd, TokenKind.STRING)
                }
                inFence -> if (lineEnd > lineStart) tokens += Token(lineStart, lineEnd, TokenKind.STRING)
                HEADING.matchesAt(line, 0) -> tokens += Token(lineStart, lineEnd, TokenKind.HEADING)
                else -> inlineMarkdown(line, lineStart, tokens)
            }
            lineStart = lineEnd + 1
        }
        return tokens
    }

    private fun inlineMarkdown(line: String, offset: Int, tokens: MutableList<Token>) {
        INLINE.findAll(line).forEach { match ->
            val kind = if (match.value.startsWith("`")) TokenKind.STRING else TokenKind.KEYWORD
            tokens += Token(offset + match.range.first, offset + match.range.last + 1, kind)
        }
    }

    private val HEADING = Regex("#{1,6}\\s")
    private val INLINE = Regex("`[^`]+`|\\*\\*[^*]+\\*\\*|\\[[^\\]]*\\]\\([^)]*\\)")
}
