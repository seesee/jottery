import Foundation

/// In-memory search over decrypted notes.
///
/// Supports the full Jottery query syntax for parity with the web and TUI clients:
///
/// - **Tags**: `#tagname` (AND logic), `#tag1 | #tag2` (OR logic)
/// - **Text**: plain words (AND logic), `"exact phrase"`, `word1 | word2` (OR)
/// - **Exclusion**: `-term`, `-#tag`
/// - **Wildcards**: `dog*`, `*dog`, `*dog*`
/// - **Modifiers**: `has:attachment`, `created:>YYYY-MM-DD`, `modified:<YYYY-MM-DD`,
///   `created:YYYY-MM-DD..YYYY-MM-DD`, `words:>N`, `words:<N`, `words:N..N`,
///   `color:name`, `colour:name`, `category:name`
enum SearchService {

    // MARK: - Public API

    /// Filter notes by a search query string.
    static func filter(notes: [DecryptedNote], query: String) -> [DecryptedNote] {
        let trimmed = query.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else { return notes }

        let parsed = parseQuery(trimmed)
        return notes.filter { matchesQuery(note: $0, query: parsed) }
    }

    /// Sort notes by the given order, pinned notes always first.
    static func sort(notes: [DecryptedNote], order: SortOrder) -> [DecryptedNote] {
        notes.sorted { a, b in
            if a.pinned != b.pinned { return a.pinned }
            switch order {
            case .recent:  return a.modifiedAt > b.modifiedAt
            case .oldest:  return a.modifiedAt < b.modifiedAt
            case .alpha:   return a.title.localizedCaseInsensitiveCompare(b.title) == .orderedAscending
            case .created: return a.createdAt > b.createdAt
            }
        }
    }

    // MARK: - Parsed Query

    struct ParsedQuery {
        // Tags (AND — all must match)
        var requiredTags: [String] = []
        // Tags (OR groups — at least one in each group must match)
        var orTagGroups: [[String]] = []
        // Tags to exclude
        var excludeTags: [String] = []

        // Text terms (AND — all must match)
        var requiredText: [TextMatch] = []
        // Text terms (OR groups)
        var orTextGroups: [[TextMatch]] = []
        // Text to exclude
        var excludeText: [String] = []

        // Modifiers
        var hasAttachment: Bool?
        var createdAfter: Date?
        var createdBefore: Date?
        var modifiedAfter: Date?
        var modifiedBefore: Date?
        var wordCountMin: Int?
        var wordCountMax: Int?
        var colors: [String] = []
        var excludeColors: [String] = []
    }

    enum TextMatch {
        case contains(String)    // plain word
        case prefix(String)      // word*
        case suffix(String)      // *word
        case infix(String)       // *word*  (same as contains, but from wildcard syntax)
        case exact(String)       // "exact phrase"

        func matches(_ text: String) -> Bool {
            switch self {
            case .contains(let term):  return text.contains(term)
            case .prefix(let term):    return matchesWordPrefix(text, prefix: term)
            case .suffix(let term):    return matchesWordSuffix(text, suffix: term)
            case .infix(let term):     return text.contains(term)
            case .exact(let phrase):   return text.contains(phrase)
            }
        }

        private func matchesWordPrefix(_ text: String, prefix: String) -> Bool {
            // Check if any word in the text starts with the prefix
            let words = text.split(whereSeparator: { $0.isWhitespace || $0.isNewline || $0.isPunctuation })
            return words.contains { String($0).hasPrefix(prefix) }
        }

        private func matchesWordSuffix(_ text: String, suffix: String) -> Bool {
            let words = text.split(whereSeparator: { $0.isWhitespace || $0.isNewline || $0.isPunctuation })
            return words.contains { String($0).hasSuffix(suffix) }
        }
    }

    // MARK: - Query Parsing

    static func parseQuery(_ raw: String) -> ParsedQuery {
        var query = ParsedQuery()
        var remaining = raw

        // 1. Extract modifiers (they get removed from the query string)
        remaining = extractModifiers(from: remaining, into: &query)

        // 2. Tokenize the remainder (handles quotes, #tags, -exclusions, | OR)
        tokenize(remaining, into: &query)

        return query
    }

    // MARK: - Modifier Extraction

    private static let datePattern = #"(\d{4}-\d{2}-\d{2})"#

    private static func extractModifiers(from input: String, into query: inout ParsedQuery) -> String {
        var text = input

        // has:attachment
        if let range = text.range(of: #"\bhas:attachment\b"#, options: .regularExpression) {
            query.hasAttachment = true
            text.removeSubrange(range)
        }

        // created:>DATE
        text = extractDateModifier(from: text, pattern: #"created:>"# + datePattern, setter: { query.createdAfter = $0 })
        // created:<DATE
        text = extractDateModifier(from: text, pattern: #"created:<"# + datePattern, setter: { query.createdBefore = $0 })
        // created:DATE..DATE
        text = extractDateRange(from: text, prefix: "created", setAfter: { query.createdAfter = $0 }, setBefore: { query.createdBefore = $0 })

        // modified:>DATE
        text = extractDateModifier(from: text, pattern: #"modified:>"# + datePattern, setter: { query.modifiedAfter = $0 })
        // modified:<DATE
        text = extractDateModifier(from: text, pattern: #"modified:<"# + datePattern, setter: { query.modifiedBefore = $0 })
        // modified:DATE..DATE
        text = extractDateRange(from: text, prefix: "modified", setAfter: { query.modifiedAfter = $0 }, setBefore: { query.modifiedBefore = $0 })

        // words:>N, words:<N, words:N..N
        text = extractWordCount(from: text, into: &query)

        // color: / colour: / category: filters
        text = extractColorFilters(from: text, into: &query)

        return text.trimmingCharacters(in: .whitespaces)
    }

    private static func extractDateModifier(from text: String, pattern: String, setter: (Date) -> Void) -> String {
        var result = text
        if let match = result.range(of: pattern, options: .regularExpression) {
            let matched = String(result[match])
            // Extract the date part (last 10 chars = YYYY-MM-DD)
            let dateStr = String(matched.suffix(10))
            if let date = parseDate(dateStr) {
                setter(date)
            }
            result.removeSubrange(match)
        }
        return result
    }

    private static func extractDateRange(from text: String, prefix: String, setAfter: (Date) -> Void, setBefore: (Date) -> Void) -> String {
        var result = text
        let pattern = prefix + #":"# + datePattern + #"\.\."# + datePattern
        if let match = result.range(of: pattern, options: .regularExpression) {
            let matched = String(result[match])
            // Extract both dates
            let dateRegex = try? NSRegularExpression(pattern: datePattern)
            let nsRange = NSRange(matched.startIndex..<matched.endIndex, in: matched)
            let matches = dateRegex?.matches(in: matched, range: nsRange) ?? []
            if matches.count >= 2 {
                if let r1 = Range(matches[0].range, in: matched), let d1 = parseDate(String(matched[r1])) {
                    setAfter(d1)
                }
                if let r2 = Range(matches[1].range, in: matched), let d2 = parseDate(String(matched[r2])) {
                    // Set before to end of day
                    setBefore(Calendar.current.date(byAdding: .day, value: 1, to: d2) ?? d2)
                }
            }
            result.removeSubrange(match)
        }
        return result
    }

    private static func extractWordCount(from text: String, into query: inout ParsedQuery) -> String {
        var result = text

        // words:>N
        if let match = result.range(of: #"words:>(\d+)"#, options: .regularExpression) {
            let matched = String(result[match])
            if let num = Int(matched.replacingOccurrences(of: "words:>", with: "")) {
                query.wordCountMin = num
            }
            result.removeSubrange(match)
        }

        // words:<N
        if let match = result.range(of: #"words:<(\d+)"#, options: .regularExpression) {
            let matched = String(result[match])
            if let num = Int(matched.replacingOccurrences(of: "words:<", with: "")) {
                query.wordCountMax = num
            }
            result.removeSubrange(match)
        }

        // words:N..N
        if let match = result.range(of: #"words:(\d+)\.\.(\d+)"#, options: .regularExpression) {
            let matched = String(result[match])
            let nums = matched.replacingOccurrences(of: "words:", with: "").split(separator: ".").compactMap { Int($0) }
            if nums.count >= 2 {
                query.wordCountMin = nums[0]
                query.wordCountMax = nums[1]
            }
            result.removeSubrange(match)
        }

        return result
    }

    private static func extractColorFilters(from text: String, into query: inout ParsedQuery) -> String {
        var result = text

        // Match color:name, colour:name, or category:name (case-insensitive)
        let colorPattern = #"(?i)\b(?:colou?r|category):([a-z0-9#]+)\b"#
        while let match = result.range(of: colorPattern, options: .regularExpression) {
            let matched = String(result[match])
            // Extract the color name after the colon
            if let colonIdx = matched.firstIndex(of: ":") {
                let colorName = String(matched[matched.index(after: colonIdx)...]).lowercased()
                query.colors.append(colorName)
            }
            result.removeSubrange(match)
        }

        return result
    }

    // MARK: - Tokenization

    private static func tokenize(_ input: String, into query: inout ParsedQuery) {
        let tokens = splitTokens(input)

        // Group tokens by | (OR) separators
        var currentGroup: [RawToken] = []
        var groups: [[RawToken]] = []
        var hasOr = false

        for token in tokens {
            if case .orSeparator = token {
                hasOr = true
                groups.append(currentGroup)
                currentGroup = []
            } else {
                currentGroup.append(token)
            }
        }
        groups.append(currentGroup)

        if hasOr && groups.count > 1 {
            // Check if this is an OR group of tags or text
            let allTags = groups.allSatisfy { group in
                group.count == 1 && {
                    if case .tag = group[0] { return true }
                    return false
                }()
            }

            if allTags {
                // OR tag group
                let tagNames = groups.compactMap { group -> String? in
                    guard let token = group.first, case .tag(let name) = token else { return nil }
                    return name
                }
                query.orTagGroups.append(tagNames)
            } else {
                // OR text group
                let textMatches = groups.compactMap { group -> TextMatch? in
                    guard let token = group.first else { return nil }
                    return textMatchFromToken(token)
                }
                if !textMatches.isEmpty {
                    query.orTextGroups.append(textMatches)
                }
            }
        } else {
            // No OR — treat as AND
            for token in currentGroup {
                switch token {
                case .tag(let name):
                    query.requiredTags.append(name)
                case .excludeTag(let name):
                    query.excludeTags.append(name)
                case .exclude(let term):
                    query.excludeText.append(term)
                case .text(let term):
                    query.requiredText.append(classifyTextMatch(term))
                case .phrase(let phrase):
                    query.requiredText.append(.exact(phrase.lowercased()))
                case .orSeparator:
                    break
                }
            }
        }
    }

    private enum RawToken {
        case tag(String)
        case excludeTag(String)
        case exclude(String)
        case text(String)
        case phrase(String)
        case orSeparator
    }

    private static func splitTokens(_ input: String) -> [RawToken] {
        var tokens: [RawToken] = []
        var i = input.startIndex

        while i < input.endIndex {
            // Skip whitespace
            if input[i].isWhitespace {
                i = input.index(after: i)
                continue
            }

            // OR separator
            if input[i] == "|" {
                tokens.append(.orSeparator)
                i = input.index(after: i)
                continue
            }

            // Quoted phrase
            if input[i] == "\"" {
                let start = input.index(after: i)
                if let end = input[start...].firstIndex(of: "\"") {
                    let phrase = String(input[start..<end])
                    tokens.append(.phrase(phrase))
                    i = input.index(after: end)
                } else {
                    // No closing quote — treat rest as phrase
                    let phrase = String(input[start...])
                    tokens.append(.phrase(phrase))
                    i = input.endIndex
                }
                continue
            }

            // Read a word (until whitespace or |)
            let wordStart = i
            while i < input.endIndex && !input[i].isWhitespace && input[i] != "|" {
                i = input.index(after: i)
            }
            let word = String(input[wordStart..<i])

            if word.hasPrefix("-#") && word.count > 2 {
                tokens.append(.excludeTag(String(word.dropFirst(2)).lowercased()))
            } else if word.hasPrefix("#") && word.count > 1 {
                tokens.append(.tag(String(word.dropFirst()).lowercased()))
            } else if word.hasPrefix("-") && word.count > 1 {
                tokens.append(.exclude(String(word.dropFirst()).lowercased()))
            } else if !word.isEmpty {
                tokens.append(.text(word.lowercased()))
            }
        }

        return tokens
    }

    private static func classifyTextMatch(_ term: String) -> TextMatch {
        if term.hasPrefix("*") && term.hasSuffix("*") && term.count > 2 {
            return .infix(String(term.dropFirst().dropLast()))
        } else if term.hasSuffix("*") && term.count > 1 {
            return .prefix(String(term.dropLast()))
        } else if term.hasPrefix("*") && term.count > 1 {
            return .suffix(String(term.dropFirst()))
        }
        return .contains(term)
    }

    private static func textMatchFromToken(_ token: RawToken) -> TextMatch? {
        switch token {
        case .text(let term): return classifyTextMatch(term)
        case .phrase(let phrase): return .exact(phrase.lowercased())
        case .tag: return nil
        case .excludeTag, .exclude: return nil
        case .orSeparator: return nil
        }
    }

    // MARK: - Matching

    private static func matchesQuery(note: DecryptedNote, query: ParsedQuery) -> Bool {
        let lowerContent = note.content.lowercased()
        let lowerTags = note.tags.map { $0.lowercased() }
        let searchableText = lowerContent + " " + lowerTags.joined(separator: " ")

        // Required tags (AND)
        for tag in query.requiredTags {
            if !lowerTags.contains(tag) { return false }
        }

        // OR tag groups
        for group in query.orTagGroups {
            if !group.contains(where: { lowerTags.contains($0) }) { return false }
        }

        // Excluded tags
        for tag in query.excludeTags {
            if lowerTags.contains(tag) { return false }
        }

        // Required text (AND)
        for match in query.requiredText {
            if !match.matches(searchableText) { return false }
        }

        // OR text groups
        for group in query.orTextGroups {
            if !group.contains(where: { $0.matches(searchableText) }) { return false }
        }

        // Excluded text
        for term in query.excludeText {
            if lowerContent.contains(term) { return false }
        }

        // has:attachment
        if let hasAttachment = query.hasAttachment, hasAttachment {
            if note.attachments.isEmpty { return false }
        }

        // Date filters
        if let after = query.createdAfter, note.createdAt < after { return false }
        if let before = query.createdBefore, note.createdAt > before { return false }
        if let after = query.modifiedAfter, note.modifiedAt < after { return false }
        if let before = query.modifiedBefore, note.modifiedAt > before { return false }

        // Word count
        if let min = query.wordCountMin, note.wordCount < min { return false }
        if let max = query.wordCountMax, note.wordCount > max { return false }

        // Color / category
        if !query.colors.isEmpty {
            guard let noteColor = note.color?.lowercased() else { return false }
            if !query.colors.contains(noteColor) { return false }
        }

        return true
    }

    // MARK: - Date Parsing

    private static let dateFormatter: DateFormatter = {
        let fmt = DateFormatter()
        fmt.dateFormat = "yyyy-MM-dd"
        fmt.locale = Locale(identifier: "en_US_POSIX")
        fmt.timeZone = .current
        return fmt
    }()

    private static func parseDate(_ string: String) -> Date? {
        dateFormatter.date(from: string)
    }
}
