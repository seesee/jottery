import Foundation

/// In-memory search over decrypted notes.
///
/// Every client follows the same rules, written down with test cases in
/// docs/search-query-fixture.json, so that a saved search means the same thing
/// everywhere. The web client's `searchService.ts` is the reference.
///
/// - **Text**: plain words (AND), `"exact phrase"`, `-word`, `-"a phrase"`
/// - **Tags**: `#tag` matches any tag containing it, `-#tag`
/// - **OR**: `|` joins the terms either side into a group, which can mix tags
///   and text. Groups are ANDed with everything else.
/// - **Wildcards**: `dog*`, `*dog`, `*dog*`
/// - **Modifiers**: `has:attachment`, `archived`, `archived:true`,
///   `created:>YYYY-MM-DD`, `created:<YYYY-MM-DD`, `created:YYYY-MM-DD..YYYY-MM-DD`,
///   the same for `modified:`, `words:>N`, `words:<N`, `words:N..N`,
///   `color:name`, `colour:name`, `category:name`, each of the last three with
///   an optional `note` or `tag` after it
enum SearchService {

    // MARK: - Public API

    /// Filter notes by a search query string. `notes` should hold both active
    /// and archived notes: `archived` can reach either, and without it only
    /// those matching `archiveMode` count. `colours` is what `category:` and
    /// tag colours go by.
    static func filter(
        notes: [DecryptedNote],
        query: String,
        archiveMode: Bool = false,
        colours: NoteColourSettings = NoteColourSettings()
    ) -> [DecryptedNote] {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return notes.filter { $0.archived == archiveMode } }

        let parsed = parseQuery(trimmed, colours: colours)
        return notes.filter { matchesQuery(note: $0, query: parsed, archiveMode: archiveMode, colours: colours) }
    }

    /// Sort notes by the given order, pinned notes always first.
    static func sort(notes: [DecryptedNote], order: SortOrder) -> [DecryptedNote] {
        // A title is built from the note's tags and content, so it's built once
        // per note here: a comparison runs some n log n times.
        if order == .alpha {
            return notes.map { (note: $0, title: $0.title) }
                .sorted { a, b in
                    if a.note.pinned != b.note.pinned { return a.note.pinned }
                    return a.title.localizedCaseInsensitiveCompare(b.title) == .orderedAscending
                }
                .map(\.note)
        }
        return notes.sorted { a, b in
            if a.pinned != b.pinned { return a.pinned }
            switch order {
            case .recent:  return a.modifiedAt > b.modifiedAt
            case .oldest:  return a.modifiedAt < b.modifiedAt
            case .created: return a.createdAt > b.createdAt
            case .alpha:   return false  // Handled above.
            }
        }
    }

    // MARK: - Parsed Query

    struct ParsedQuery: Equatable {
        /// Every group has to match, through any one of its terms.
        var groups: [[Term]] = []
        var excludeText: [String] = []
        var excludeTags: [String] = []

        // Modifiers
        var hasAttachment = false
        var archived: Bool?
        /// `YYYY-MM-DD`, compared with the date part of the note's UTC timestamp.
        /// Both ends include the day named.
        var createdAfter: String?
        var createdBefore: String?
        var modifiedAfter: String?
        var modifiedBefore: String?
        var wordCountMin: Int?
        var wordCountMax: Int?
        var colors: [String] = []
        var colorTarget = ColorTarget.both
    }

    enum ColorTarget: Equatable {
        case note
        case tag
        case both
    }

    enum Term: Equatable {
        case contains(String)    // plain word, or *word*
        case phrase(String)      // "exact phrase"
        case prefix(String)      // word*
        case suffix(String)      // *word
        case tag(String)         // #tag
    }

    // MARK: - Query Parsing

    static func parseQuery(_ raw: String, colours: NoteColourSettings = NoteColourSettings()) -> ParsedQuery {
        var query = ParsedQuery()

        // Modifiers come out of the query string first, as on the web.
        let remaining = extractModifiers(from: raw, into: &query, colours: colours)

        var currentGroup: Int?
        var joinToCurrent = false
        for token in tokens(in: remaining) {
            switch token {
            case .or:
                joinToCurrent = currentGroup != nil
            case .term(let term):
                if joinToCurrent, let currentGroup {
                    query.groups[currentGroup].append(term)
                } else {
                    query.groups.append([term])
                    currentGroup = query.groups.count - 1
                }
                joinToCurrent = false
            case .excludeText(let value):
                // An exclusion always applies, so it ends any OR group.
                query.excludeText.append(value)
                joinToCurrent = false
            case .excludeTag(let value):
                query.excludeTags.append(value)
                joinToCurrent = false
            }
        }
        return query
    }

    // MARK: - Modifier Extraction

    private static let date = #"(\d{4}-\d{2}-\d{2})"#

    private static func extractModifiers(
        from input: String,
        into query: inout ParsedQuery,
        colours: NoteColourSettings
    ) -> String {
        var text = input

        if !extract(#"\bhas:attachment\b"#, from: &text).isEmpty {
            query.hasAttachment = true
        }

        let explicitArchive = extract(#"\barchived?:(true|false|yes|no|1|0)\b"#, from: &text)
        if let value = explicitArchive.first?[0].lowercased() {
            query.archived = ["true", "yes", "1"].contains(value)
        } else if !extract(#"\barchived?\b"#, from: &text).isEmpty {
            query.archived = true
        }

        (query.createdAfter, query.createdBefore) = extractDates("created", from: &text)
        (query.modifiedAfter, query.modifiedBefore) = extractDates("modified", from: &text)
        extractWordCount(from: &text, into: &query)
        extractColors(from: &text, into: &query)
        extractCategories(from: &text, into: &query, colours: colours)

        return text
    }

    private static func extractDates(_ field: String, from text: inout String) -> (after: String?, before: String?) {
        if let range = extract(#"\b"# + field + ":" + date + #"\.\."# + date + #"\b"#, from: &text, firstOnly: true).first {
            return (range[0], range[1])
        }
        let after = extract(#"\b"# + field + ":>" + date + #"\b"#, from: &text, firstOnly: true).first?[0]
        let before = extract(#"\b"# + field + ":<" + date + #"\b"#, from: &text, firstOnly: true).first?[0]
        return (after, before)
    }

    private static func extractWordCount(from text: inout String, into query: inout ParsedQuery) {
        if let range = extract(#"\bwords:(\d+)\.\.(\d+)\b"#, from: &text, firstOnly: true).first {
            query.wordCountMin = Int(range[0])
            query.wordCountMax = Int(range[1])
            return
        }
        query.wordCountMin = extract(#"\bwords:>(\d+)\b"#, from: &text, firstOnly: true).first.flatMap { Int($0[0]) }
        query.wordCountMax = extract(#"\bwords:<(\d+)\b"#, from: &text, firstOnly: true).first.flatMap { Int($0[0]) }
    }

    private static func extractColors(from text: inout String, into query: inout ParsedQuery) {
        let name = #"\bcolou?r:([a-z0-9#]+)"#
        for (qualifier, target) in [(#"\s+note\b"#, ColorTarget.note), (#"\s+tag\b"#, .tag), (#"\b"#, .both)] {
            let found = extract(name + qualifier, from: &text)
            if !found.isEmpty {
                query.colors = found.map { $0[0].lowercased() }
                query.colorTarget = target
                return
            }
        }
    }

    /// `category:` goes by display name. A name nothing has is no filter.
    private static func extractCategories(
        from text: inout String,
        into query: inout ParsedQuery,
        colours: NoteColourSettings
    ) {
        guard query.colors.isEmpty else { return }
        let name = #"\bcategory:([\w\s-]+)"#
        for (qualifier, target) in [(#"\s+note\b"#, ColorTarget.note), (#"\s+tag\b"#, .tag), (#"\b"#, .both)] {
            let found = extract(name + qualifier, from: &text)
            if !found.isEmpty {
                query.colors = found.compactMap { colours.colourKey(forCategory: $0[0]) }
                query.colorTarget = target
                return
            }
        }
    }

    /// Remove what `pattern` matches, case-insensitively, and return the capture
    /// groups of each match.
    private static func extract(_ pattern: String, from text: inout String, firstOnly: Bool = false) -> [[String]] {
        guard let regex = try? NSRegularExpression(pattern: pattern, options: .caseInsensitive) else { return [] }
        var results: [[String]] = []
        let all = regex.matches(in: text, range: NSRange(text.startIndex..., in: text))
        for match in (firstOnly ? Array(all.prefix(1)) : all).reversed() {
            results.insert((1..<match.numberOfRanges).map { group in
                Range(match.range(at: group), in: text).map { String(text[$0]) } ?? ""
            }, at: 0)
            if let range = Range(match.range, in: text) {
                text.replaceSubrange(range, with: " ")
            }
        }
        return results
    }

    // MARK: - Tokenisation

    private enum Token {
        case or
        case term(Term)
        case excludeText(String)
        case excludeTag(String)
    }

    private static func tokens(in input: String) -> [Token] {
        var tokens: [Token] = []
        var index = input.startIndex

        while index < input.endIndex {
            if input[index].isWhitespace {
                index = input.index(after: index)
                continue
            }

            if input[index] == "|" {
                tokens.append(.or)
                index = input.index(after: index)
                continue
            }

            // An unclosed quote runs to the end of the query, so a phrase can be
            // typed and searched for before the closing quote is reached.
            let negatedPhrase = input[index...].hasPrefix("-\"")
            if input[index] == "\"" || negatedPhrase {
                let start = input.index(index, offsetBy: negatedPhrase ? 2 : 1)
                let end = input[start...].firstIndex(of: "\"")
                let value = normalise(String(input[start..<(end ?? input.endIndex)]))
                index = end.map { input.index(after: $0) } ?? input.endIndex
                if !value.isEmpty {
                    tokens.append(negatedPhrase ? .excludeText(value) : .term(.phrase(value)))
                }
                continue
            }

            var end = index
            while end < input.endIndex, !input[end].isWhitespace, input[end] != "|" {
                end = input.index(after: end)
            }
            tokens.append(token(forWord: String(input[index..<end])))
            index = end
        }

        return tokens
    }

    private static func token(forWord word: String) -> Token {
        if word.hasPrefix("-#"), word.count > 2 {
            return .excludeTag(normalise(String(word.dropFirst(2))))
        }
        if word.hasPrefix("#"), word.count > 1 {
            return .term(.tag(normalise(String(word.dropFirst()))))
        }
        if word.hasPrefix("-"), word.count > 1 {
            return .excludeText(normalise(String(word.dropFirst())))
        }
        return .term(classify(word))
    }

    /// Classify a word by its wildcards. A word of only asterisks is a search for them.
    private static func classify(_ word: String) -> Term {
        let value = normalise(word.trimmingCharacters(in: CharacterSet(charactersIn: "*")))
        guard !value.isEmpty else { return .contains(word) }
        switch (word.hasPrefix("*"), word.hasSuffix("*")) {
        case (true, false): return .suffix(value)
        case (false, true): return .prefix(value)
        default: return .contains(value)
        }
    }

    /// The same form of a string, however it's composed and whatever its case.
    static func normalise(_ text: String) -> String {
        text.precomposedStringWithCanonicalMapping.lowercased()
    }

    // MARK: - Matching

    private struct Searchable {
        let content: String
        let tags: [String]
        let text: String
        let words: [Substring]

        init(_ note: DecryptedNote, needsWords: Bool) {
            content = normalise(note.content)
            tags = note.tags.map(normalise)
            text = content + " " + tags.joined(separator: " ")
            words = needsWords ? text.split { $0.isWhitespace || $0.isPunctuation } : []
        }

        func matches(_ term: Term) -> Bool {
            switch term {
            case .tag(let value): return tags.contains { $0.contains(value) }
            case .prefix(let value): return words.contains { $0.hasPrefix(value) }
            case .suffix(let value): return words.contains { $0.hasSuffix(value) }
            case .contains(let value), .phrase(let value): return text.contains(value)
            }
        }
    }

    private static func matchesQuery(
        note: DecryptedNote,
        query: ParsedQuery,
        archiveMode: Bool,
        colours: NoteColourSettings
    ) -> Bool {
        guard note.archived == (query.archived ?? archiveMode) else { return false }

        let needsWords = query.groups.joined().contains {
            switch $0 {
            case .prefix, .suffix: return true
            default: return false
            }
        }
        let searchable = Searchable(note, needsWords: needsWords)

        guard query.groups.allSatisfy({ $0.contains(where: searchable.matches) }),
              query.excludeText.allSatisfy({ !searchable.content.contains($0) }),
              query.excludeTags.allSatisfy({ term in !searchable.tags.contains { $0.contains(term) } }) else {
            return false
        }

        if query.hasAttachment, note.attachments.isEmpty { return false }

        // Date filters, by the UTC date as the web compares them
        let created = utcDay(note.createdAt)
        let modified = utcDay(note.modifiedAt)
        if let after = query.createdAfter, created < after { return false }
        if let before = query.createdBefore, created > before { return false }
        if let after = query.modifiedAfter, modified < after { return false }
        if let before = query.modifiedBefore, modified > before { return false }

        // Word count
        if let min = query.wordCountMin, note.wordCount < min { return false }
        if let max = query.wordCountMax, note.wordCount > max { return false }

        if !query.colors.isEmpty, !matchesColour(note, query: query, colours: colours) {
            return false
        }

        return true
    }

    private static func matchesColour(
        _ note: DecryptedNote,
        query: ParsedQuery,
        colours: NoteColourSettings
    ) -> Bool {
        let noteMatches = note.color.map { query.colors.contains(normalise($0)) } ?? false
        let tagMatches = note.tags.contains { tag in
            colours.colourKey(forTag: tag).map(query.colors.contains) ?? false
        }
        switch query.colorTarget {
        case .note: return noteMatches
        case .tag: return tagMatches
        case .both: return noteMatches || tagMatches
        }
    }

    private static func utcDay(_ date: Date) -> String {
        date.formatted(.iso8601.year().month().day())
    }
}
