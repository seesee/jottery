package com.jottery.android.service

import com.jottery.android.model.DecryptedNote
import com.jottery.android.model.NoteCategories
import com.jottery.android.model.SortOrder
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.DateTimeParseException

/**
 * In-memory search over decrypted notes.
 *
 * Supports the full Jottery query syntax for parity with the web, TUI, and iOS clients:
 *
 * - **Tags**: `#tagname` (AND logic), `#tag1 | #tag2` (OR logic)
 * - **Exclusion**: `-term`, `-#tag`
 * - **Text**: plain words (AND logic), `"exact phrase"`, `word1 | word2` (OR)
 * - **Wildcards**: `dog*`, `*dog`, `*dog*`
 * - **Modifiers**: `has:attachment`, `created:>YYYY-MM-DD`, `modified:<YYYY-MM-DD`,
 *   `created:YYYY-MM-DD..YYYY-MM-DD`, `words:>N`, `words:<N`, `words:N..N`,
 *   `color:name`, `colour:name`, `category:name`
 */
object SearchService {

    // ------------------------------------------------------------------ //
    //  Public API
    // ------------------------------------------------------------------ //

    /**
     * Result of a filter operation, including the match count for UI display.
     */
    data class FilterResult(
        val notes: List<DecryptedNote>,
        val matchCount: Int,
        val totalCount: Int,
    )

    /**
     * Filter notes by a search query string.
     *
     * Returns a [FilterResult] containing the matched notes plus counts
     * for the search bar display (e.g. "87/456").
     */
    fun filter(
        notes: List<DecryptedNote>,
        query: String,
        categories: NoteCategories = NoteCategories(),
    ): FilterResult {
        val trimmed = query.trim()
        if (trimmed.isEmpty()) {
            return FilterResult(notes = notes, matchCount = notes.size, totalCount = notes.size)
        }

        val parsed = parseQuery(trimmed, categories)
        val filtered = notes.filter { matchesQuery(it, parsed, categories) }
        return FilterResult(notes = filtered, matchCount = filtered.size, totalCount = notes.size)
    }

    /** Whether a query asks for archived notes, wherever the list is showing. */
    fun searchesArchive(query: String): Boolean = parseQuery(query.trim()).archivedOnly

    /**
     * Sort notes by the given order, with pinned notes always first.
     */
    fun sort(notes: List<DecryptedNote>, order: SortOrder): List<DecryptedNote> {
        return notes.sortedWith(Comparator { a, b ->
            // Pinned notes always come first
            if (a.pinned != b.pinned) {
                return@Comparator if (a.pinned) -1 else 1
            }
            when (order) {
                SortOrder.RECENT -> b.modifiedAt.compareTo(a.modifiedAt)
                SortOrder.OLDEST -> a.modifiedAt.compareTo(b.modifiedAt)
                SortOrder.ALPHA -> a.title.compareTo(b.title, ignoreCase = true)
                SortOrder.CREATED -> b.createdAt.compareTo(a.createdAt)
            }
        })
    }

    // ------------------------------------------------------------------ //
    //  Parsed Query
    // ------------------------------------------------------------------ //

    data class ParsedQuery(
        // Tags (AND -- all must match)
        val requiredTags: MutableList<String> = mutableListOf(),
        // Tags to exclude
        val excludeTags: MutableList<String> = mutableListOf(),

        // Text terms (AND -- all must match)
        val requiredText: MutableList<TextMatch> = mutableListOf(),
        // Terms joined by |: at least one in each group must match, tags and text alike
        val orGroups: MutableList<List<OrTerm>> = mutableListOf(),
        // Text to exclude
        val excludeText: MutableList<String> = mutableListOf(),

        // Modifiers
        var hasAttachment: Boolean? = null,
        var createdAfter: Instant? = null,
        var createdBefore: Instant? = null,
        var modifiedAfter: Instant? = null,
        var modifiedBefore: Instant? = null,
        var wordCountMin: Int? = null,
        var wordCountMax: Int? = null,
        val colors: MutableList<String> = mutableListOf(),
        val excludeColors: MutableList<String> = mutableListOf(),
        // `archived` searches archived notes only
        var archivedOnly: Boolean = false,
        // `color:red tag` or `color:red note` narrows it to that kind
        var tagColorOnly: Boolean = false,
        var noteColorOnly: Boolean = false,
    )

    sealed class OrTerm {
        data class Tag(val name: String) : OrTerm()
        data class Text(val match: TextMatch) : OrTerm()
    }

    sealed class TextMatch {
        /** Plain word -- case-insensitive contains. */
        data class Contains(val term: String) : TextMatch()
        /** word* -- matches words starting with prefix. */
        data class Prefix(val term: String) : TextMatch()
        /** *word -- matches words ending with suffix. */
        data class Suffix(val term: String) : TextMatch()
        /** *word* -- same as contains, but from explicit wildcard syntax. */
        data class Infix(val term: String) : TextMatch()
        /** "exact phrase" -- case-insensitive phrase match. */
        data class Exact(val phrase: String) : TextMatch()

        fun matches(text: String): Boolean = when (this) {
            is Contains -> text.contains(term)
            is Prefix -> matchesWordPrefix(text, term)
            is Suffix -> matchesWordSuffix(text, term)
            is Infix -> text.contains(term)
            is Exact -> text.contains(phrase)
        }

        private fun matchesWordPrefix(text: String, prefix: String): Boolean {
            // Check if any word in the text starts with the prefix
            return text.split(WORD_SPLIT_REGEX).any { word ->
                word.isNotEmpty() && word.startsWith(prefix)
            }
        }

        private fun matchesWordSuffix(text: String, suffix: String): Boolean {
            return text.split(WORD_SPLIT_REGEX).any { word ->
                word.isNotEmpty() && word.endsWith(suffix)
            }
        }

        companion object {
            private val WORD_SPLIT_REGEX = Regex("[\\s\\n\\p{Punct}]+")
        }
    }

    // ------------------------------------------------------------------ //
    //  Query Parsing
    // ------------------------------------------------------------------ //

    fun parseQuery(raw: String, categories: NoteCategories = NoteCategories()): ParsedQuery {
        val query = ParsedQuery()
        // 1. Extract modifiers (removed from the query string)
        val remaining = extractModifiers(raw, query, categories)
        // 2. Tokenize the remainder (handles quotes, #tags, -exclusions, | OR)
        tokenize(remaining, query)
        return query
    }

    // ------------------------------------------------------------------ //
    //  Modifier Extraction
    // ------------------------------------------------------------------ //

    private val DATE_PATTERN = """(\d{4}-\d{2}-\d{2})"""
    private val DATE_REGEX = Regex(DATE_PATTERN)

    private fun extractModifiers(input: String, query: ParsedQuery, categories: NoteCategories): String {
        var text = input

        // archived, archived:true
        val archivedRegex = Regex("""(?i)(?<![^\s])archived(?::true)?(?![^\s])""")
        if (archivedRegex.containsMatchIn(text)) {
            query.archivedOnly = true
            text = archivedRegex.replaceFirst(text, "")
        }

        // has:attachment
        val hasAttachmentRegex = Regex("""\bhas:attachment\b""")
        if (hasAttachmentRegex.containsMatchIn(text)) {
            query.hasAttachment = true
            text = hasAttachmentRegex.replaceFirst(text, "")
        }

        // created:>DATE
        text = extractDateModifier(text, Regex("""created:>$DATE_PATTERN""")) { query.createdAfter = it }
        // created:<DATE
        text = extractDateModifier(text, Regex("""created:<$DATE_PATTERN"""), endOfDay = true) { query.createdBefore = it }
        // created:DATE..DATE
        text = extractDateRange(text, "created", { query.createdAfter = it }, { query.createdBefore = it })

        // modified:>DATE
        text = extractDateModifier(text, Regex("""modified:>$DATE_PATTERN""")) { query.modifiedAfter = it }
        // modified:<DATE
        text = extractDateModifier(text, Regex("""modified:<$DATE_PATTERN"""), endOfDay = true) { query.modifiedBefore = it }
        // modified:DATE..DATE
        text = extractDateRange(text, "modified", { query.modifiedAfter = it }, { query.modifiedBefore = it })

        // words:>N, words:<N, words:N..N
        text = extractWordCount(text, query)

        // color: / colour: / category: filters
        text = extractColorFilters(text, query, categories)

        return text.trim()
    }

    private fun extractDateModifier(
        text: String,
        pattern: Regex,
        endOfDay: Boolean = false,
        setter: (Instant) -> Unit,
    ): String {
        val match = pattern.find(text) ?: return text
        val matched = match.value
        // The date is always the last 10 characters (YYYY-MM-DD)
        val dateStr = matched.takeLast(10)
        val date = parseDate(dateStr)
        if (date != null) {
            // A "before" date includes that day, as on web and the TUI
            setter(
                if (endOfDay) {
                    LocalDate.parse(dateStr, DATE_FORMATTER).plusDays(1)
                        .atStartOfDay(ZoneId.systemDefault()).toInstant()
                } else {
                    date
                }
            )
        }
        return text.removeRange(match.range)
    }

    private fun extractDateRange(
        text: String,
        prefix: String,
        setAfter: (Instant) -> Unit,
        setBefore: (Instant) -> Unit,
    ): String {
        val pattern = Regex("""$prefix:$DATE_PATTERN\.\.$DATE_PATTERN""")
        val match = pattern.find(text) ?: return text
        val matched = match.value

        val dates = DATE_REGEX.findAll(matched).toList()
        if (dates.size >= 2) {
            val d1 = parseDate(dates[0].value)
            val d2 = parseDate(dates[1].value)
            if (d1 != null) setAfter(d1)
            if (d2 != null) {
                // Set before to end of day (start of next day)
                val nextDay = LocalDate.parse(dates[1].value, DATE_FORMATTER).plusDays(1)
                setBefore(nextDay.atStartOfDay(ZoneId.systemDefault()).toInstant())
            }
        }
        return text.removeRange(match.range)
    }

    private fun extractWordCount(text: String, query: ParsedQuery): String {
        var result = text

        // words:>N
        val gtPattern = Regex("""words:>(\d+)""")
        gtPattern.find(result)?.let { match ->
            val num = match.groupValues[1].toIntOrNull()
            if (num != null) query.wordCountMin = num
            result = result.removeRange(match.range)
        }

        // words:<N
        val ltPattern = Regex("""words:<(\d+)""")
        ltPattern.find(result)?.let { match ->
            val num = match.groupValues[1].toIntOrNull()
            if (num != null) query.wordCountMax = num
            result = result.removeRange(match.range)
        }

        // words:N..N
        val rangePattern = Regex("""words:(\d+)\.\.(\d+)""")
        rangePattern.find(result)?.let { match ->
            val min = match.groupValues[1].toIntOrNull()
            val max = match.groupValues[2].toIntOrNull()
            if (min != null) query.wordCountMin = min
            if (max != null) query.wordCountMax = max
            result = result.removeRange(match.range)
        }

        return result
    }

    private fun extractColorFilters(text: String, query: ParsedQuery, categories: NoteCategories): String {
        var result = text

        fun narrow(kind: String) {
            if (kind.equals("tag", ignoreCase = true)) query.tagColorOnly = true
            if (kind.equals("note", ignoreCase = true)) query.noteColorOnly = true
        }

        // A name the user chose can be several words, so the names come first, longest first
        for ((colour, name) in categories.names.entries.sortedByDescending { it.value.length }) {
            val pattern = Regex("""(?i)\bcategory:${Regex.escape(name)}(?![\w-])(?:\s+(note|tag)\b)?""")
            var match = pattern.find(result)
            while (match != null) {
                query.colors.add(colour)
                narrow(match.groupValues[1])
                result = result.removeRange(match.range)
                match = pattern.find(result)
            }
        }

        // Match color:name, colour:name, or category:name (case-insensitive)
        // `note` or `tag` after it says which colour is meant
        val colorPattern = Regex("""(?i)\b(?:colou?r|category):([a-z0-9#]+)\b(?:\s+(note|tag)\b)?""")
        var match = colorPattern.find(result)
        while (match != null) {
            val colorName = match.groupValues[1].lowercase()
            query.colors.add(colorName)
            narrow(match.groupValues[2])
            result = result.removeRange(match.range)
            match = colorPattern.find(result)
        }

        return result
    }

    // ------------------------------------------------------------------ //
    //  Tokenization
    // ------------------------------------------------------------------ //

    private sealed class RawToken {
        data class Tag(val name: String) : RawToken()
        data class ExcludeTag(val name: String) : RawToken()
        data class Exclude(val term: String) : RawToken()
        data class Text(val term: String) : RawToken()
        data class Phrase(val phrase: String) : RawToken()
        data object OrSeparator : RawToken()
    }

    /**
     * The grammar shared by every client (docs/search-query-fixture.json). Terms
     * side by side are ANDed. | joins the terms on either side into a group where
     * any one may match, so `#work dog | cat` is #work AND (dog OR cat). An
     * exclusion is always ANDed and ends a group, and a | with nothing on one side
     * is ignored.
     */
    private fun tokenize(input: String, query: ParsedQuery) {
        var group = mutableListOf<RawToken>()
        var joinNext = false

        fun endGroup() {
            when (group.size) {
                0 -> Unit
                1 -> when (val token = group[0]) {
                    is RawToken.Tag -> query.requiredTags.add(token.name)
                    else -> textMatchFromToken(token)?.let { query.requiredText.add(it) }
                }
                else -> query.orGroups.add(
                    group.mapNotNull { token ->
                        if (token is RawToken.Tag) OrTerm.Tag(token.name)
                        else textMatchFromToken(token)?.let { OrTerm.Text(it) }
                    }
                )
            }
            group = mutableListOf()
        }

        for (token in splitTokens(input)) {
            when (token) {
                is RawToken.OrSeparator -> joinNext = group.isNotEmpty()
                is RawToken.ExcludeTag -> {
                    endGroup()
                    joinNext = false
                    query.excludeTags.add(token.name)
                }
                is RawToken.Exclude -> {
                    endGroup()
                    joinNext = false
                    query.excludeText.add(token.term)
                }
                else -> {
                    if (!joinNext) endGroup()
                    joinNext = false
                    group.add(token)
                }
            }
        }
        endGroup()
    }

    private fun splitTokens(input: String): List<RawToken> {
        val tokens = mutableListOf<RawToken>()
        var i = 0

        while (i < input.length) {
            val ch = input[i]

            // Skip whitespace
            if (ch.isWhitespace()) {
                i++
                continue
            }

            // OR separator
            if (ch == '|') {
                tokens.add(RawToken.OrSeparator)
                i++
                continue
            }

            // Excluded phrase
            if (ch == '-' && i + 1 < input.length && input[i + 1] == '"') {
                val start = i + 2
                val end = input.indexOf('"', start).takeIf { it != -1 } ?: input.length
                tokens.add(RawToken.Exclude(input.substring(start, end).lowercase()))
                i = minOf(end + 1, input.length)
                continue
            }

            // Quoted phrase
            if (ch == '"') {
                val start = i + 1
                val end = input.indexOf('"', start)
                if (end != -1) {
                    val phrase = input.substring(start, end)
                    tokens.add(RawToken.Phrase(phrase))
                    i = end + 1
                } else {
                    // No closing quote -- treat rest as phrase
                    val phrase = input.substring(start)
                    tokens.add(RawToken.Phrase(phrase))
                    i = input.length
                }
                continue
            }

            // Read a word (until whitespace or |)
            val wordStart = i
            while (i < input.length && !input[i].isWhitespace() && input[i] != '|') {
                i++
            }
            val word = input.substring(wordStart, i)

            when {
                word.startsWith("-#") && word.length > 2 ->
                    tokens.add(RawToken.ExcludeTag(word.drop(2).lowercase()))
                word.startsWith("#") && word.length > 1 ->
                    tokens.add(RawToken.Tag(word.drop(1).lowercase()))
                word.startsWith("-") && word.length > 1 ->
                    tokens.add(RawToken.Exclude(word.drop(1).lowercase()))
                word.isNotEmpty() ->
                    tokens.add(RawToken.Text(word.lowercase()))
            }
        }

        return tokens
    }

    private fun classifyTextMatch(term: String): TextMatch {
        return when {
            term.startsWith("*") && term.endsWith("*") && term.length > 2 ->
                TextMatch.Infix(term.drop(1).dropLast(1))
            term.endsWith("*") && term.length > 1 ->
                TextMatch.Prefix(term.dropLast(1))
            term.startsWith("*") && term.length > 1 ->
                TextMatch.Suffix(term.drop(1))
            else ->
                TextMatch.Contains(term)
        }
    }

    private fun textMatchFromToken(token: RawToken): TextMatch? = when (token) {
        is RawToken.Text -> classifyTextMatch(token.term)
        is RawToken.Phrase -> TextMatch.Exact(token.phrase.lowercase())
        is RawToken.Tag -> null
        is RawToken.ExcludeTag -> null
        is RawToken.Exclude -> null
        is RawToken.OrSeparator -> null
    }

    // ------------------------------------------------------------------ //
    //  Matching
    // ------------------------------------------------------------------ //

    private fun matchesQuery(note: DecryptedNote, query: ParsedQuery, categories: NoteCategories): Boolean {
        val lowerContent = note.searchableContent
        val lowerTags = note.searchableTags
        val searchableText = lowerContent + " " + lowerTags.joinToString(" ")

        // Required tags (AND). #tag matches any tag containing that text.
        for (tag in query.requiredTags) {
            if (lowerTags.none { it.contains(tag) }) return false
        }

        // Excluded tags
        for (tag in query.excludeTags) {
            if (lowerTags.any { it.contains(tag) }) return false
        }

        // Required text (AND)
        for (match in query.requiredText) {
            if (!match.matches(searchableText)) return false
        }

        // Groups joined by |
        for (group in query.orGroups) {
            val anyMatch = group.any { term ->
                when (term) {
                    is OrTerm.Tag -> lowerTags.any { it.contains(term.name) }
                    is OrTerm.Text -> term.match.matches(searchableText)
                }
            }
            if (!anyMatch) return false
        }

        // Excluded text
        for (term in query.excludeText) {
            if (lowerContent.contains(term)) return false
        }

        if (query.archivedOnly && !note.archived) return false

        // has:attachment
        if (query.hasAttachment == true) {
            if (note.attachments.isEmpty()) return false
        }

        // Date filters
        if (query.createdAfter != null && note.createdAt.isBefore(query.createdAfter)) return false
        if (query.createdBefore != null && !note.createdAt.isBefore(query.createdBefore)) return false
        if (query.modifiedAfter != null && note.modifiedAt.isBefore(query.modifiedAfter)) return false
        if (query.modifiedBefore != null && !note.modifiedAt.isBefore(query.modifiedBefore)) return false

        // Word count
        if (query.wordCountMin != null && note.wordCount < query.wordCountMin!!) return false
        if (query.wordCountMax != null && note.wordCount > query.wordCountMax!!) return false

        // Color / category
        if (query.colors.isNotEmpty()) {
            val noteColor = note.color?.lowercase()
            val tagColors = note.tags.mapNotNull(categories::colourOfTag)
            val matched = query.colors.any { colour ->
                val onNote = colour == noteColor
                val onTag = colour in tagColors
                when {
                    query.tagColorOnly -> onTag
                    query.noteColorOnly -> onNote
                    else -> onNote || onTag
                }
            }
            if (!matched) return false
        }

        return true
    }

    // ------------------------------------------------------------------ //
    //  Date Parsing
    // ------------------------------------------------------------------ //

    private val DATE_FORMATTER: DateTimeFormatter = DateTimeFormatter.ofPattern("yyyy-MM-dd")

    /**
     * Parse a date string (YYYY-MM-DD) into an [Instant] at the start of that day
     * in the system default time zone.
     */
    private fun parseDate(dateStr: String): Instant? {
        return try {
            val localDate = LocalDate.parse(dateStr, DATE_FORMATTER)
            localDate.atStartOfDay(ZoneId.systemDefault()).toInstant()
        } catch (_: DateTimeParseException) {
            null
        }
    }
}
