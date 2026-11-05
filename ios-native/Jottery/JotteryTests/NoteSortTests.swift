import Foundation
import Testing

@testable import Jottery

/// What the sort has to keep doing now a title is built once per note (jottery-4rfb).
struct NoteSortTests {

    private func note(_ content: String, pinned: Bool = false) -> DecryptedNote {
        DecryptedNote(
            id: UUID().uuidString, createdAt: Date(), modifiedAt: Date(), syncedAt: nil,
            content: content, tags: [], attachments: [], pinned: pinned,
            archived: false, archivedAt: nil, locked: false, lockedAt: nil,
            deleted: false, deletedAt: nil, version: 0, wordWrap: true,
            syntaxLanguage: "markdown", showPreview: false, color: nil,
            contentHash: nil, parentHash: nil, hashChain: [], needsSync: false, decryptedAt: Date()
        )
    }

    @Test func alphabeticalIgnoresCase() {
        let sorted = SearchService.sort(notes: [note("banana"), note("Apple"), note("cherry")], order: .alpha)

        #expect(sorted.map(\.content) == ["Apple", "banana", "cherry"])
    }

    @Test func pinnedNotesStayFirstAlphabetically() {
        let sorted = SearchService.sort(notes: [note("Apple"), note("Zebra", pinned: true)], order: .alpha)

        #expect(sorted.map(\.content) == ["Zebra", "Apple"])
    }

    @Test func aTitleTagIsWhatSorts() {
        var titled = note("zzz")
        titled.tags = ["t:Aardvark"]

        let sorted = SearchService.sort(notes: [note("Middle"), titled], order: .alpha)

        #expect(sorted.first?.title == "Aardvark")
    }
}
