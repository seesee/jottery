import Foundation
import Testing

@testable import Jottery

private let marker = "zebra-marker-\(UUID().uuidString)"

struct RecentSearchesTests {

    @Test func theNewestComesFirst() {
        var recent = RecentSearches()
        recent.record("first")
        recent.record("second")
        #expect(recent.queries == ["second", "first"])
    }

    @Test func aRepeatMovesUpRatherThanAppearingTwice() {
        var recent = RecentSearches()
        recent.record("first")
        recent.record("second")
        recent.record("FIRST")
        #expect(recent.queries == ["FIRST", "second"])
    }

    @Test func spaceAroundAQueryIsNotKept() {
        var recent = RecentSearches()
        recent.record("  dog  ")
        #expect(recent.queries == ["dog"])
    }

    @Test(arguments: ["", " ", "a", "#", "  a  "])
    func tooLittleToBeWorthRepeatingIsNotKept(query: String) {
        var recent = RecentSearches()
        recent.record(query)
        #expect(recent.queries.isEmpty)
    }

    @Test func onlyTheLatestAreKept() {
        var recent = RecentSearches()
        for index in 0..<(RecentSearches.limit + 5) {
            recent.record("query \(index)")
        }
        #expect(recent.queries.count == RecentSearches.limit)
        #expect(recent.queries.first == "query \(RecentSearches.limit + 4)")
    }
}

struct SearchTagCompletionTests {

    private let tags = ["homework", "play", "work", "workshop"]

    private func completions(_ query: String, tags: [String]? = nil) -> [String] {
        SearchSuggestions.tagCompletions(for: query, tags: tags ?? self.tags).map(\.completion)
    }

    @Test func tagsStartingWithWhatsTypedComeFirst() {
        #expect(completions("#wo") == ["#work", "#workshop", "#homework"])
    }

    @Test func matchingIgnoresCase() {
        #expect(completions("#PL") == ["#play"])
    }

    @Test func aHashAloneOffersTags() {
        #expect(completions("#") == ["#homework", "#play", "#work", "#workshop"])
    }

    @Test func theListIsShort() {
        let many = (0..<50).map { "tag\($0)" }
        #expect(completions("#ta", tags: many).count == SearchSuggestions.tagLimit)
    }

    @Test func whatComesBeforeTheTagStays() {
        #expect(completions("dog #pl") == ["dog #play"])
        #expect(completions("dog | #pl") == ["dog | #play"])
        #expect(completions("dog |#pl") == ["dog |#play"])
    }

    @Test func anExcludedTagIsCompletedToo() {
        #expect(completions("-#pl") == ["-#play"])
    }

    /// Suggestions cover the results, so a finished tag has to get out of the way.
    @Test func aFinishedTagOffersNothing() {
        #expect(completions("#work").isEmpty)
        #expect(completions("#WORK").isEmpty)
    }

    @Test func plainWordsOfferNothing() {
        #expect(completions("wor").isEmpty)
        #expect(completions("#work ").isEmpty)
    }

    @Test func aHashInsideAPhraseIsNotATag() {
        #expect(completions("\"issue #wo").isEmpty)
    }

    /// `t:` and `title:` tags give a note its title, and aren't tags to a reader.
    @Test func titleTagsAreLeftOut() {
        #expect(completions("#t", tags: ["t:Shopping", "title:Plans", "tea"]) == ["#tea"])
    }

    /// `#` ends at a space, so a tag with one is searched for word by word.
    @Test func aTagWithASpaceIsFoundByEachOfItsWords() {
        #expect(completions("#ho", tags: ["home office"]) == ["#home #office"])
    }
}

struct SearchExampleTests {

    @Test func everyExampleIsAQueryTheSearchUnderstands() {
        for example in SearchSuggestions.examples(firstTag: "work") {
            let parsed = SearchService.parseQuery(example.query)
            #expect(parsed != SearchService.ParsedQuery(), "\(example.query) parsed to nothing")
        }
    }

    @Test func theTagExampleUsesARealTag() {
        #expect(SearchSuggestions.examples(firstTag: "work").first?.query == "#work")
        #expect(SearchSuggestions.examples(firstTag: nil).first?.query == "#tag")
    }

    @Test func theDateExampleIsThisYears() throws {
        let today = try #require(Date(iso8601: "2031-06-15T12:00:00.000Z"))
        let queries = SearchSuggestions.examples(firstTag: nil, today: today).map(\.query)
        #expect(queries.contains("created:>2031-01-01"))
    }
}

@MainActor
struct SearchHintTests {

    private func makeUnlockedState(in dir: URL? = nil) throws -> AppState {
        let state = try TestVaults.makeState()
        state.keyManager.unlockWithKeyData(Data(repeating: 7, count: 32))
        state.isLocked = false
        return state
    }

    @Test func aSearchThatFindsNothingHereCountsTheArchive() async throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "the zebra crossing"))
        _ = try #require(try state.createNote(content: "something else"))
        try state.archiveNote(id: note.id)

        state.searchQuery = "zebra"
        state.scheduleSearch()

        #expect(await waitUntil { state.matchesInOtherView == 1 })
        #expect(state.filteredNotes.isEmpty)
    }

    @Test func theArchiveCountsTheNotesToo() async throws {
        let state = try makeUnlockedState()
        _ = try #require(try state.createNote(content: "the zebra crossing"))
        state.showArchive = true

        state.searchQuery = "zebra"
        state.scheduleSearch()

        #expect(await waitUntil { state.matchesInOtherView == 1 })
    }

    @Test func aSearchThatFindsSomethingHasNoHint() async throws {
        let state = try makeUnlockedState()
        let archived = try #require(try state.createNote(content: "zebra one"))
        try state.archiveNote(id: archived.id)
        _ = try #require(try state.createNote(content: "zebra two"))

        state.searchQuery = "zebra"
        state.scheduleSearch()

        #expect(await waitUntil { state.filteredNotes.count == 1 })
        #expect(state.matchesInOtherView == 0)
    }

    @Test func clearingTheSearchClearsTheHint() async throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "zebra"))
        try state.archiveNote(id: note.id)
        state.searchQuery = "zebra"
        state.scheduleSearch()
        #expect(await waitUntil { state.matchesInOtherView == 1 })

        state.searchQuery = ""
        state.scheduleSearch()

        #expect(state.matchesInOtherView == 0)
    }
}

/// jottery-ajpu: a recent search says what someone was looking for, so it gets the
/// same protection as a note. Here that means it's never written anywhere.
@MainActor
struct RecentSearchPrivacyTests {

    private func makeUnlockedState(at dir: URL) throws -> AppState {
        let state = AppState()
        try state.initialise(database: DatabaseManager(path: dir.appendingPathComponent("test.db").path))
        state.keyManager.unlockWithKeyData(Data(repeating: 7, count: 32))
        state.isLocked = false
        return state
    }

    private func makeDirectory() throws -> URL {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-recent-search-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }

    @Test func recentSearchesEndWithTheLock() throws {
        let state = try makeUnlockedState(at: makeDirectory())
        state.searchQuery = "private matter"
        state.recordRecentSearch()
        #expect(state.recentSearches.queries == ["private matter"])

        state.lock()

        #expect(state.recentSearches.queries.isEmpty)
    }

    @Test func recentSearchesAreNotWrittenDown() throws {
        let dir = try makeDirectory()
        let state = try makeUnlockedState(at: dir)
        var settings = state.settings
        settings.sortOrder = "oldest"
        state.searchQuery = marker
        state.recordRecentSearch()
        // Everything that does get written, with the search in place.
        try state.updateSettings(settings)
        _ = try state.createNote(content: "a note")
        state.lock()

        let files = try #require(FileManager.default.enumerator(at: dir, includingPropertiesForKeys: nil))
        for case let file as URL in files {
            let data = try Data(contentsOf: file)
            #expect(data.range(of: Data(marker.utf8)) == nil, "\(file.lastPathComponent) holds the search")
        }
        #expect(!UserDefaults.standard.dictionaryRepresentation().description.contains(marker))
    }

    @Test func aSearchIsOnlyRememberedOnceItsUsed() throws {
        let state = try makeUnlockedState(at: makeDirectory())
        state.searchQuery = "half typed"
        state.scheduleSearch()
        #expect(state.recentSearches.queries.isEmpty)
    }
}
