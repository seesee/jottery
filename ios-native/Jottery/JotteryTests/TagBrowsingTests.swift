import Foundation
import Testing

@testable import Jottery

/// Tags are encrypted, so these counts can only come from notes already decrypted.
@MainActor
struct TagBrowsingTests {

    private func makeUnlockedState() throws -> AppState {
        let state = try TestVaults.makeState()
        state.keyManager.unlockWithKeyData(Data(repeating: 7, count: 32))
        state.isLocked = false
        return state
    }

    private func makeNote(tags: [String], in state: AppState) throws -> DecryptedNote {
        try #require(try state.createNote(content: "text", tags: tags))
    }

    // MARK: - Counting

    @Test func eachTagIsCountedWithItsNotes() throws {
        let state = try makeUnlockedState()
        _ = try makeNote(tags: ["work", "home"], in: state)
        _ = try makeNote(tags: ["work"], in: state)

        #expect(TagCount.counts(in: state.notes) == [
            TagCount(tag: "home", count: 1),
            TagCount(tag: "work", count: 2),
        ])
    }

    @Test func theyAreInTheOrderAReaderExpects() throws {
        let state = try makeUnlockedState()
        for tag in ["zebra", "Apple", "mango"] {
            _ = try makeNote(tags: [tag], in: state)
        }

        #expect(TagCount.counts(in: state.notes).map(\.tag) == ["Apple", "mango", "zebra"])
    }

    @Test func aTagTwiceOnANoteCountsOnce() throws {
        let state = try makeUnlockedState()
        _ = try makeNote(tags: ["work", "work"], in: state)

        #expect(TagCount.counts(in: state.notes) == [TagCount(tag: "work", count: 1)])
    }

    /// A `t:` tag is how a note gets its title, not a tag anyone browses.
    @Test func titleTagsAreLeftOut() throws {
        let state = try makeUnlockedState()
        _ = try makeNote(tags: ["t:Shopping", "title:Plans", "work"], in: state)

        #expect(TagCount.counts(in: state.notes).map(\.tag) == ["work"])
    }

    @Test func aVaultWithNoTagsHasNone() throws {
        let state = try makeUnlockedState()
        _ = try makeNote(tags: [], in: state)

        #expect(TagCount.counts(in: state.notes).isEmpty)
    }

    /// A count says what tapping the tag will find, and that's in this view.
    @Test func archivedNotesCountInTheArchive() throws {
        let state = try makeUnlockedState()
        let archived = try makeNote(tags: ["old"], in: state)
        _ = try makeNote(tags: ["new"], in: state)
        try state.archiveNote(id: archived.id)

        #expect(TagCount.counts(in: state.displayedNotes).map(\.tag) == ["new"])

        state.showArchive = true
        #expect(TagCount.counts(in: state.displayedNotes).map(\.tag) == ["old"])
    }

    @Test func nothingIsLeftBehindTheLock() throws {
        let state = try makeUnlockedState()
        _ = try makeNote(tags: ["work"], in: state)

        state.lock()

        #expect(TagCount.counts(in: state.displayedNotes).isEmpty)
    }

    // MARK: - Browsing

    @Test func choosingATagSetsTheSearch() async throws {
        let state = try makeUnlockedState()
        let tagged = try makeNote(tags: ["work"], in: state)
        _ = try makeNote(tags: ["home"], in: state)

        state.browse(tag: "work")
        state.scheduleSearch()

        #expect(state.searchQuery == "#work")
        #expect(await waitUntil { state.filteredNotes.map(\.id) == [tagged.id] })
    }

    /// `#` stops at a space, so this has to go in word by word to find the note.
    @Test func aTagWithASpaceIsFound() async throws {
        let state = try makeUnlockedState()
        let tagged = try makeNote(tags: ["home office"], in: state)
        _ = try makeNote(tags: ["home"], in: state)

        state.browse(tag: "home office")
        state.scheduleSearch()

        #expect(await waitUntil { state.filteredNotes.map(\.id) == [tagged.id] })
    }

    // MARK: - About

    @Test func aboutCountsWhatsInTheVault() async throws {
        let state = try makeUnlockedState()
        let pinned = try makeNote(tags: [], in: state)
        _ = try makeNote(tags: [], in: state)
        let archived = try makeNote(tags: [], in: state)
        let binned = try makeNote(tags: [], in: state)
        try state.togglePin(id: pinned.id)
        try state.archiveNote(id: archived.id)
        try state.deleteNote(id: binned.id)

        #expect(state.notes.filter(\.pinned).count == 1)
        #expect(state.archivedNotes.count == 1)
        #expect(try #require(state.noteRepo).countInBin() == 1)
    }

    @Test func aNoteDeletedForGoodLeavesTheBin() throws {
        let state = try makeUnlockedState()
        let note = try makeNote(tags: [], in: state)
        try state.deleteNote(id: note.id)

        try state.permanentlyDeleteNote(id: note.id)

        #expect(try #require(state.noteRepo).countInBin() == 0)
    }
}
