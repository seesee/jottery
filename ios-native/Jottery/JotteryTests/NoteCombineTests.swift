import CryptoKit
import Foundation
import GRDB
import Testing

@testable import Jottery

/// Combine makes one note from several and sends the originals to the bin. The
/// new note owns copies of their attachments (jottery-kxxx): sharing ids with a
/// note in the bin means the bin's clean-up, or removing an attachment from either
/// note, takes the file from the other.
@MainActor
struct NoteCombineTests {

    private func makeUnlockedState() throws -> AppState {
        let state = try TestVaults.makeState()
        state.keyManager.unlockWithKeyData(Data(repeating: 7, count: 32))
        state.isLocked = false
        return state
    }

    /// Notes made one at a time, with `createdAt` in the order given.
    private func makeNotes(_ contents: [String], in state: AppState) throws -> [DecryptedNote] {
        var made: [DecryptedNote] = []
        for (index, content) in contents.enumerated() {
            let note = try #require(try state.createNote(content: content))
            let created = Date(timeIntervalSince1970: 1_700_000_000 + Double(index) * 60).iso8601
            try TestVaults.execute(state, "UPDATE notes SET created_at = ? WHERE id = ?", [created, note.id])
            made.append(note)
        }
        try reload(state)
        return made
    }

    private func reload(_ state: AppState) throws {
        let key = try #require(state.keyManager.masterKey)
        state.notes = try #require(state.noteRepo).listActive(key: key)
    }

    private func addAttachment(named name: String, to note: DecryptedNote, in state: AppState) throws -> AttachmentRef {
        try state.addAttachment(to: note.id, data: Data(name.utf8), filename: name, mimeType: "text/plain")
        let key = try #require(state.keyManager.masterKey)
        let repo = try #require(state.noteRepo)
        return try #require(try repo.get(id: note.id, key: key)?.attachments.last)
    }

    private func stored(_ id: String, in state: AppState) throws -> DecryptedNote {
        let key = try #require(state.keyManager.masterKey)
        let repo = try #require(state.noteRepo)
        return try #require(try repo.get(id: id, key: key))
    }

    private func combine(_ notes: [DecryptedNote], in state: AppState, mergeTags: Bool = true) async throws -> DecryptedNote {
        let id = try await state.combineNotes(ids: Set(notes.map(\.id)), mergeTags: mergeTags)
        return try stored(try #require(id), in: state)
    }

    // MARK: - What the New Note Holds

    @Test func textIsJoinedOldestFirst() async throws {
        let state = try makeUnlockedState()
        let notes = try makeNotes(["first", "second", "third"], in: state)

        let combined = try await combine(notes.reversed(), in: state)

        #expect(combined.content == "first\n\n---\n\nsecond\n\n---\n\nthird")
        #expect(combined.syntaxLanguage == "markdown")
        #expect(combined.needsSync)
    }

    @Test func originalsGoToTheBinAndTheNewNoteOpens() async throws {
        let state = try makeUnlockedState()
        let notes = try makeNotes(["one", "two"], in: state)
        state.selectedNoteId = notes[0].id

        let combined = try await combine(notes, in: state)

        for note in notes {
            #expect(try stored(note.id, in: state).deleted)
        }
        #expect(state.notes.map(\.id) == [combined.id])
        #expect(state.selectedNoteId == combined.id)
    }

    @Test func mergingTagsKeepsEachOnce() async throws {
        let state = try makeUnlockedState()
        let notes = try makeNotes(["one", "two"], in: state)
        try state.saveNote({ var note = notes[0]; note.tags = ["a", "b"]; return note }())
        try state.saveNote({ var note = notes[1]; note.tags = ["b", "c"]; return note }())
        // Saving put back these copies, from before `makeNotes` set the dates.
        try reload(state)

        let combined = try await combine(notes, in: state)

        #expect(combined.tags == ["a", "b", "c"])
    }

    @Test func withoutMergingOnlyTheOldestNotesTagsStay() async throws {
        let state = try makeUnlockedState()
        let notes = try makeNotes(["one", "two"], in: state)
        try state.saveNote({ var note = notes[0]; note.tags = ["a"]; return note }())
        try state.saveNote({ var note = notes[1]; note.tags = ["b"]; return note }())
        // Saving put back these copies, from before `makeNotes` set the dates.
        try reload(state)

        let combined = try await combine(notes, in: state, mergeTags: false)

        #expect(combined.tags == ["a"])
    }

    @Test func anEditStillWaitingToSaveIsCombined() async throws {
        let state = try makeUnlockedState()
        let notes = try makeNotes(["one", "two"], in: state)
        var edited = notes[1]
        edited.content = "typed just now"
        state.pendingEditorNote = edited

        let combined = try await combine(notes, in: state)

        #expect(combined.content == "one\n\n---\n\ntyped just now")
        #expect(state.pendingEditorNote == nil)
    }

    // MARK: - Who Can Be Combined

    @Test func lockedNotesAreLeftOut() async throws {
        let state = try makeUnlockedState()
        let notes = try makeNotes(["one", "locked", "three"], in: state)
        try state.toggleLock(id: notes[1].id)

        let combined = try await combine(notes, in: state)

        #expect(combined.content == "one\n\n---\n\nthree")
        #expect(try !stored(notes[1].id, in: state).deleted)
    }

    @Test func archivedNotesAreLeftOut() async throws {
        let state = try makeUnlockedState()
        let notes = try makeNotes(["one", "archived", "three"], in: state)
        try state.archiveNote(id: notes[1].id)

        let combined = try await combine(notes, in: state)

        #expect(combined.content == "one\n\n---\n\nthree")
        #expect(try !stored(notes[1].id, in: state).deleted)
    }

    @Test func oneCombinableNoteIsNotEnough() async throws {
        let state = try makeUnlockedState()
        let notes = try makeNotes(["one", "locked"], in: state)
        try state.toggleLock(id: notes[1].id)

        await #expect(throws: NoteCombining.Failure.tooFewNotes) {
            try await state.combineNotes(ids: Set(notes.map(\.id)), mergeTags: true)
        }

        #expect(state.notes.count == 2)
        #expect(try !stored(notes[0].id, in: state).deleted)
    }

    // MARK: - Attachments

    @Test func attachmentsAreCopiedUnderNewIds() async throws {
        let state = try makeUnlockedState()
        let notes = try makeNotes(["one", "two"], in: state)
        let first = try addAttachment(named: "a.txt", to: notes[0], in: state)
        let second = try addAttachment(named: "b.txt", to: notes[1], in: state)
        let blobs = try #require(state.attachmentRepo)

        let combined = try await combine(notes, in: state)

        #expect(combined.attachments.map(\.filename) == ["a.txt", "b.txt"])
        #expect(Set(combined.attachments.map(\.id)).isDisjoint(with: [first.id, second.id]))
        for (copy, original) in zip(combined.attachments, [first, second]) {
            #expect(copy.data == copy.id)
            #expect(copy.size == original.size)
            #expect(try blobs.getBlob(id: copy.data) == blobs.getBlob(id: original.data))
        }
    }

    /// The server is told about blobs it doesn't hold, and these are new to it.
    @Test func copiesAreQueuedForUpload() async throws {
        let state = try makeUnlockedState()
        let notes = try makeNotes(["one", "two"], in: state)
        let original = try addAttachment(named: "a.txt", to: notes[0], in: state)
        let blobs = try #require(state.attachmentRepo)
        try blobs.markUploaded(ids: [original.data])

        let combined = try await combine(notes, in: state)

        let copy = try #require(combined.attachments.first)
        #expect(try blobs.getBlobToUpload(id: copy.data) != nil)
    }

    /// `update` keeps what's stored, which has to stay encrypted.
    @Test func filenamesStayEncrypted() async throws {
        let state = try makeUnlockedState()
        let notes = try makeNotes(["one", "two"], in: state)
        _ = try addAttachment(named: "secret-name.txt", to: notes[0], in: state)

        let combined = try await combine(notes, in: state)

        let repo = try #require(state.noteRepo)
        let raw = try #require(try repo.getRaw(id: combined.id))
        #expect(!raw.attachments.contains("secret-name"))
    }

    /// jottery-kxxx: emptying the bin took the combined note's files with it.
    @Test func emptyingTheBinLeavesTheCombinedNotesFiles() async throws {
        let state = try makeUnlockedState()
        let notes = try makeNotes(["one", "two"], in: state)
        _ = try addAttachment(named: "a.txt", to: notes[0], in: state)
        let combined = try await combine(notes, in: state)

        for note in notes {
            try state.permanentlyDeleteNote(id: note.id)
        }

        let blobs = try #require(state.attachmentRepo)
        let copy = try #require(combined.attachments.first)
        #expect(try blobs.hasBlob(id: copy.data))
    }

    @Test func removingAnAttachmentFromTheNewNoteLeavesTheOriginalsFile() async throws {
        let state = try makeUnlockedState()
        let notes = try makeNotes(["one", "two"], in: state)
        let original = try addAttachment(named: "a.txt", to: notes[0], in: state)
        let combined = try await combine(notes, in: state)
        let copy = try #require(combined.attachments.first)

        try state.removeAttachment(from: combined.id, attachmentId: copy.id)

        let blobs = try #require(state.attachmentRepo)
        #expect(try blobs.hasBlob(id: original.data))
    }

    /// Images are written into the text by attachment id, so they follow the copy.
    @Test func referencesInTheTextFollowTheCopies() async throws {
        let state = try makeUnlockedState()
        let notes = try makeNotes(["one", "two"], in: state)
        let original = try addAttachment(named: "a.png", to: notes[0], in: state)
        var note = try stored(notes[0].id, in: state)
        note.content = "![](attachment:\(original.id))"
        try state.saveNote(note)

        let combined = try await combine(notes, in: state)

        let copy = try #require(combined.attachments.first)
        #expect(combined.content.hasPrefix("![](attachment:\(copy.id))"))
        #expect(!combined.content.contains(original.id))
    }

    /// Notes combined by a client that shared ids can hold the same attachment.
    @Test func anAttachmentTwoNotesShareIsCopiedOnce() async throws {
        let state = try makeUnlockedState()
        let notes = try makeNotes(["one", "two"], in: state)
        let shared = try addAttachment(named: "a.txt", to: notes[0], in: state)
        let repo = try #require(state.noteRepo)
        let raw = try #require(try repo.getRaw(id: notes[0].id))
        try TestVaults.execute(state, "UPDATE notes SET attachments = ? WHERE id = ?", [raw.attachments, notes[1].id])
        try reload(state)

        let combined = try await combine(notes, in: state)

        #expect(combined.attachments.count == 1)
        #expect(combined.attachments.first?.id != shared.id)
    }

    /// A file that hasn't reached this device can't be copied, and the bin would
    /// eventually take the only reference to it. So nothing changes.
    @Test func aMissingFileStopsTheCombineAndChangesNothing() async throws {
        let state = try makeUnlockedState()
        let notes = try makeNotes(["one", "two"], in: state)
        let present = try addAttachment(named: "a.txt", to: notes[0], in: state)
        let missing = try addAttachment(named: "b.txt", to: notes[1], in: state)
        let blobs = try #require(state.attachmentRepo)
        try blobs.deleteBlob(id: missing.data)
        let before = try blobs.listBlobIds()

        await #expect(throws: NoteCombining.Failure.attachmentNotOnDevice) {
            try await state.combineNotes(ids: Set(notes.map(\.id)), mergeTags: true)
        }

        #expect(state.notes.count == 2)
        for note in notes {
            #expect(try !stored(note.id, in: state).deleted)
        }
        #expect(try blobs.listBlobIds() == before)
        #expect(try blobs.hasBlob(id: present.data))
    }

    /// A hash that left the attachments out would make the note look changed to
    /// the next client that recomputes it.
    @Test func theHashCoversTheAttachments() async throws {
        let state = try makeUnlockedState()
        let notes = try makeNotes(["one", "two"], in: state)
        _ = try addAttachment(named: "a.txt", to: notes[0], in: state)

        let combined = try await combine(notes, in: state)

        let repo = try #require(state.noteRepo)
        let raw = try #require(try repo.getRaw(id: combined.id))
        let expected = HashChainService.computeContentHash(
            encryptedContent: raw.content,
            encryptedTags: [],
            attachmentIds: combined.attachments.map(\.id).sorted(),
            pinned: false, archived: false, locked: false,
            syntaxLanguage: "markdown", wordWrap: true, showPreview: false, color: nil
        )
        #expect(raw.contentHash == expected)
    }

    @Test func aVaultBeingReKeyedCombinesNothing() async throws {
        let state = try makeUnlockedState()
        let notes = try makeNotes(["one", "two"], in: state)
        state.isReKeying = true

        await #expect(throws: AppStateError.self) {
            try await state.combineNotes(ids: Set(notes.map(\.id)), mergeTags: true)
        }

        #expect(state.notes.count == 2)
    }
}
