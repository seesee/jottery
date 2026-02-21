package com.jottery.android.service

import com.jottery.android.model.DecryptedNote
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
    fun filter(notes: List<DecryptedNote>, query: String): FilterResult {
        val trimmed = query.trim()
        if (trimmed.isEmpty()) {
            return FilterResult(notes = notes, matchCount = notes.size, totalCount = notes.size)
        }

        val parsed = parseQuery(trimmed)
        val filtered = notes.filter { matchesQuery(it, parsed) }
        return FilterResult(notes = filtered, matchCount = filtered.size, totalCount = notes.size)
    }

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
        // Tags (OR groups -- at least one in each group must match)
        val orTagGroups: MutableList<List<String>> = mutableListOf(),
        // Tags to exclude
        val excludeTags: MutableList<String> = mutableListOf(),

        // Text terms (AND -- all must match)
        val requiredText: MutableList<TextMatch> = mutableListOf(),
        // Text terms (OR groups)
        val orTextGroups: MutableList<List<TextMatch>> = mutableListOf(),
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
    )

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

    fun parseQuery(raw: String): ParsedQuery {
        val query = ParsedQuery()
        // 1. Extract modifiers (removed from the query string)
        val remaining = extractModifiers(raw, query)
        // 2. Tokenize the remainder (handles quotes, #tags, -exclusions, | OR)
        tokenize(remaining, query)
        return query
    }

    // ------------------------------------------------------------------ //
    //  Modifier Extraction
    // ------------------------------------------------------------------ //

    private val DATE_PATTERN = """(\d{4}-\d{2}-\d{2})"""
    private val DATE_REGEX = Regex(DATE_PATTERN)

    private fun extractModifiers(input: String, query: ParsedQuery): String {
        var text = input

        // has:attachment
        val hasAttachmentRegex = Regex("""\bhas:attachment\b""")
        if (hasAttachmentRegex.containsMatchIn(text)) {
            query.hasAttachment = true
            text = hasAttachmentRegex.replaceFirst(text, "")
        }

        // created:>DATE
        text = extractDateModifier(text, Regex("""created:>$DATE_PATTERN""")) { query.createdAfter = it }
        // created:<DATE
        text = extractDateModifier(text, Regex("""created:<$DATE_PATTERN""")) { query.createdBefore = it }
        // created:DATE..DATE
        text = extractDateRange(text, "created", { query.createdAfter = it }, { query.createdBefore = it })

        // modified:>DATE
        text = extractDateModifier(text, Regex("""modified:>$DATE_PATTERN""")) { query.modifiedAfter = it }
        // modified:<DATE
        text = extractDateModifier(text, Regex("""modified:<$DATE_PATTERN""")) { query.modifiedBefore = it }
        // modified:DATE..DATE
        text = extractDateRange(text, "modified", { query.modifiedAfter = it }, { query.modifiedBefore = it })

        // words:>N, words:<N, words:N..N
        text = extractWordCount(text, query)

        // color: / colour: / category: filters
        text = extractColorFilters(text, query)

        return text.trim()
    }

    private fun extractDateModifier(
        text: String,
        pattern: Regex,
        setter: (Instant) -> Unit,
    ): String {
        val match = pattern.find(text) ?: return text
        val matched = match.value
        // The date is always the last 10 characters (YYYY-MM-DD)
        val dateStr = matched.takeLast(10)
        val date = parseDate(dateStr)
        if (date != null) {
            setter(date)
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

    private fun extractColorFilters(text: String, query: ParsedQuery): String {
        var result = text

        // Match color:name, colour:name, or category:name (case-insensitive)
        val colorPattern = Regex("""(?i)\b(?:colou?r|category):([a-z0-9#]+)\b""")
        var match = colorPattern.find(result)
        while (match != null) {
            val colorName = match.groupValues[1].lowercase()
            query.colors.add(colorName)
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

    private fun tokenize(input: String, query: ParsedQuery) {
        val tokens = splitTokens(input)

        // Group tokens by | (OR) separators
        var currentGroup = mutableListOf<RawToken>()
        val groups = mutableListOf<List<RawToken>>()
        var hasOr = false

        for (token in tokens) {
            if (token is RawToken.OrSeparator) {
                hasOr = true
                groups.add(currentGroup)
                currentGroup = mutableListOf()
            } else {
                currentGroup.add(token)
            }
        }
        groups.add(currentGroup)

        if (hasOr && groups.size > 1) {
            // Check if this is an OR group of tags or text
            val allTags = groups.all { group ->
                group.size == 1 && group[0] is RawToken.Tag
            }

            if (allTags) {
                // OR tag group
                val tagNames = groups.mapNotNull { group ->
                    val token = group.firstOrNull()
                    if (token is RawToken.Tag) token.name else null
                }
                query.orTagGroups.add(tagNames)
            } else {
                // OR text group
                val textMatches = groups.mapNotNull { group ->
                    val token = group.firstOrNull() ?: return@mapNotNull null
                    textMatchFromToken(token)
                }
                if (textMatches.isNotEmpty()) {
                    query.orTextGroups.add(textMatches)
                }
            }
        } else {
            // No OR -- treat as AND
            for (token in currentGroup) {
                when (token) {
                    is RawToken.Tag -> query.requiredTags.add(token.name)
                    is RawToken.ExcludeTag -> query.excludeTags.add(token.name)
                    is RawToken.Exclude -> query.excludeText.add(token.term)
                    is RawToken.Text -> query.requiredText.add(classifyTextMatch(token.term))
                    is RawToken.Phrase -> query.requiredText.add(TextMatch.Exact(token.phrase.lowercase()))
                    is RawToken.OrSeparator -> { /* already handled */ }
                }
            }
        }
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

    private fun matchesQuery(note: DecryptedNote, query: ParsedQuery): Boolean {
        val lowerContent = note.content.lowercase()
        val lowerTags = note.tags.map { it.lowercase() }
        val searchableText = lowerContent + " " + lowerTags.joinToString(" ")

        // Required tags (AND)
        for (tag in query.requiredTags) {
            if (tag !in lowerTags) return false
        }

        // OR tag groups
        for (group in query.orTagGroups) {
            if (group.none { it in lowerTags }) return false
        }

        // Excluded tags
        for (tag in query.excludeTags) {
            if (tag in lowerTags) return false
        }

        // Required text (AND)
        for (match in query.requiredText) {
            if (!match.matches(searchableText)) return false
        }

        // OR text groups
        for (group in query.orTextGroups) {
            if (group.none { it.matches(searchableText) }) return false
        }

        // Excluded text
        for (term in query.excludeText) {
            if (lowerContent.contains(term)) return false
        }

        // has:attachment
        if (query.hasAttachment == true) {
            if (note.attachments.isEmpty()) return false
        }

        // Date filters
        if (query.createdAfter != null && note.createdAt.isBefore(query.createdAfter)) return false
        if (query.createdBefore != null && note.createdAt.isAfter(query.createdBefore)) return false
        if (query.modifiedAfter != null && note.modifiedAt.isBefore(query.modifiedAfter)) return false
        if (query.modifiedBefore != null && note.modifiedAt.isAfter(query.modifiedBefore)) return false

        // Word count
        if (query.wordCountMin != null && note.wordCount < query.wordCountMin!!) return false
        if (query.wordCountMax != null && note.wordCount > query.wordCountMax!!) return false

        // Color / category
        if (query.colors.isNotEmpty()) {
            val noteColor = note.color?.lowercase() ?: return false
            if (noteColor !in query.colors) return false
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
