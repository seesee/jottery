import Foundation
import Testing

@testable import Jottery

/// docs/search-query-fixture.json, which every client runs, so that a saved
/// search means the same thing everywhere.
enum SearchFixture {
    struct File: Decodable {
        struct Note: Decodable {
            let id: String
            let content: String
            let tags: [String]
        }
        let notes: [Note]
        let cases: [Case]
    }

    struct Case: Decodable, Sendable, CustomTestStringConvertible {
        let query: String
        let expected: [String]
        var testDescription: String { "\"\(query)\"" }
    }

    static let file: File? = {
        // Up from this file to the repository root, which the simulator can read.
        let root = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let url = root.appendingPathComponent("docs/search-query-fixture.json")
        return (try? Data(contentsOf: url)).flatMap { try? JSONDecoder().decode(File.self, from: $0) }
    }()

    static var cases: [Case] { file?.cases ?? [] }
}

private func makeNote(
    id: String = UUID().uuidString,
    content: String = "",
    tags: [String] = [],
    archived: Bool = false,
    color: String? = nil,
    created: Date = Date(),
    modified: Date = Date(),
    attachments: [AttachmentRef] = []
) -> DecryptedNote {
    DecryptedNote(
        id: id, createdAt: created, modifiedAt: modified, syncedAt: nil, content: content, tags: tags,
        attachments: attachments, pinned: false, archived: archived, archivedAt: nil, locked: false,
        lockedAt: nil, deleted: false, deletedAt: nil, version: 1, wordWrap: true, syntaxLanguage: "plain",
        showPreview: false, color: color, contentHash: nil, parentHash: nil, hashChain: [], needsSync: false,
        decryptedAt: Date()
    )
}

struct SearchQueryFixtureTests {

    private let notes = (SearchFixture.file?.notes ?? []).map { makeNote(id: $0.id, content: $0.content, tags: $0.tags) }

    private func ids(_ query: String, in notes: [DecryptedNote], archiveMode: Bool = false) -> [String] {
        SearchService.filter(notes: notes, query: query, archiveMode: archiveMode).map(\.id).sorted()
    }

    /// Guards the parameterised test below, which would pass with no cases.
    @Test func theFixtureIsLoaded() {
        #expect(SearchFixture.cases.count >= 46)
    }

    @Test(arguments: SearchFixture.cases)
    func queryFindsWhatTheFixtureSays(_ fixture: SearchFixture.Case) {
        #expect(ids(fixture.query, in: notes) == fixture.expected.sorted())
    }

    @Test func aGroupCanMixATagAndText() {
        let tagged = makeNote(id: "tagged", tags: ["a"])
        let text = makeNote(id: "text", content: "a dog")
        let neither = makeNote(id: "neither", content: "a cat")

        #expect(ids("#a | dog", in: [tagged, text, neither]) == ["tagged", "text"])
    }

    @Test func aGroupKeepsEveryTermAroundIt() {
        let both = makeNote(id: "both", content: "dog", tags: ["work"])
        let untagged = makeNote(id: "untagged", content: "cat")

        // The tag and `dog` used to be dropped, finding every note with `cat`.
        #expect(ids("#work dog | cat", in: [both, untagged]) == ["both"])
    }

    @Test func aTagMatchesAnyTagContainingIt() {
        #expect(ids("#proj", in: [makeNote(id: "n", tags: ["my-project"])]) == ["n"])
        #expect(ids("-#proj", in: [makeNote(id: "n", tags: ["my-project"])]) == [])
    }

    // MARK: - Archived

    private var archive: [DecryptedNote] {
        [makeNote(id: "active", content: "dog"), makeNote(id: "stored", content: "dog", archived: true)]
    }

    @Test func archivedReachesArchivedNotesFromTheMainList() {
        #expect(ids("archived", in: archive) == ["stored"])
        #expect(ids("archived:true", in: archive) == ["stored"])
        #expect(ids("archived:yes dog", in: archive) == ["stored"])
        #expect(ids("archive", in: archive) == ["stored"])
    }

    @Test func archivedFalseReachesActiveNotesFromTheArchive() {
        #expect(ids("archived:false", in: archive, archiveMode: true) == ["active"])
        #expect(ids("archived:no dog", in: archive, archiveMode: true) == ["active"])
    }

    @Test func withoutTheOperatorOnlyTheCurrentListCounts() {
        #expect(ids("dog", in: archive) == ["active"])
        #expect(ids("dog", in: archive, archiveMode: true) == ["stored"])
    }

    // MARK: - Dates

    private func day(_ value: String) -> Date { Date(iso8601: value + "T12:00:00Z") ?? Date() }

    @Test func dateBoundsIncludeTheDayNamed() {
        let note = makeNote(id: "n", created: day("2026-03-10"), modified: day("2026-03-12"))

        #expect(ids("created:<2026-03-10", in: [note]) == ["n"])
        #expect(ids("created:>2026-03-10", in: [note]) == ["n"])
        #expect(ids("created:2026-03-10..2026-03-10", in: [note]) == ["n"])
        #expect(ids("modified:<2026-03-11", in: [note]) == [])
        #expect(ids("modified:>2026-03-13", in: [note]) == [])
    }

    @Test func wordCountsAndAttachments() {
        let note = makeNote(id: "n", content: "one two three")

        #expect(ids("words:>2", in: [note]) == ["n"])
        #expect(ids("words:<2", in: [note]) == [])
        #expect(ids("words:1..3", in: [note]) == ["n"])
        #expect(ids("has:attachment", in: [note]) == [])
    }

    @Test func modifiersCombineWithAGroup() {
        let plain = makeNote(id: "plain", content: "dog")
        let attached = makeNote(id: "attached", content: "dog", attachments: [
            AttachmentRef(id: "a", filename: "f", mimeType: "text/plain", size: 1, data: "a")
        ])

        #expect(ids("dog | cat has:attachment", in: [plain, attached]) == ["attached"])
    }

    // MARK: - Colours

    @Test func noteAndTagQualifiersArentSearchedFor() {
        let red = makeNote(id: "red", content: "plain text", color: "red")

        // "note" used to be left behind as text the note had to contain.
        #expect(ids("color:red note", in: [red]) == ["red"])
        #expect(ids("colour:red", in: [red]) == ["red"])
        #expect(ids("color:blue", in: [red]) == [])
    }

    /// With no tag given a colour, `tag` has nothing to go by.
    @Test func aTagColourFindsNothingWhenNoTagHasOne() {
        #expect(ids("color:red tag", in: [makeNote(id: "red", color: "red")]) == [])
    }

    @Test func aCategoryIsAColourByName() {
        let red = makeNote(id: "red", color: "red")

        #expect(ids("category:Red", in: [red]) == ["red"])
        #expect(ids("category:Red note", in: [red]) == ["red"])
        #expect(ids("category:Blue", in: [red]) == [])
    }

    /// A name this device doesn't know filters nothing, as on the web.
    @Test func anUnknownCategoryIsIgnored() {
        #expect(ids("category:Work", in: [makeNote(id: "n")]) == ["n"])
    }
}
