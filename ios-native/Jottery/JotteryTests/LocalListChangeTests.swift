import Foundation
import Testing

@testable import Jottery

/// A refresh reads the database, decrypts off the main thread, then publishes. If a
/// note changed here in between, what it publishes is older than the lists, and
/// would quietly undo the change on screen (jottery-twmx).
@MainActor
struct LocalListChangeTests {

    private func makeState() async throws -> AppState {
        let state = try TestVaults.makeState()
        try await state.createVault(password: "notes-password-correct-horse")
        return state
    }

    /// Runs `change` as a refresh that started before it comes to publish.
    private func expectItSupersedesARefresh(
        _ state: AppState,
        sourceLocation: SourceLocation = #_sourceLocation,
        _ change: () throws -> Void
    ) rethrows {
        let startedBefore = state.nextLoadTicket()
        try change()
        let published = state.publishIfNewest(ticket: startedBefore) {}
        #expect(!published, "a stale refresh would publish over this change", sourceLocation: sourceLocation)
    }

    @Test func creatingANoteSupersedesARunningRefresh() async throws {
        let state = try await makeState()
        try expectItSupersedesARefresh(state) { _ = try state.createNote(content: "new") }
    }

    @Test func savingANoteSupersedesARunningRefresh() async throws {
        let state = try await makeState()
        var note = try #require(try state.createNote(content: "before"))
        note.content = "after"
        try expectItSupersedesARefresh(state) { try state.saveNote(note) }
    }

    @Test func deletingANoteSupersedesARunningRefresh() async throws {
        let state = try await makeState()
        let note = try #require(try state.createNote(content: "doomed"))
        try expectItSupersedesARefresh(state) { try state.deleteNote(id: note.id) }
    }

    @Test func pinningSupersedesARunningRefresh() async throws {
        let state = try await makeState()
        let note = try #require(try state.createNote(content: "pin me"))
        try expectItSupersedesARefresh(state) { try state.togglePin(id: note.id) }
    }

    @Test func archivingSupersedesARunningRefresh() async throws {
        let state = try await makeState()
        let note = try #require(try state.createNote(content: "archive me"))
        try expectItSupersedesARefresh(state) { try state.archiveNote(id: note.id) }
    }

    @Test func lockingSupersedesARunningRefresh() async throws {
        let state = try await makeState()
        let note = try #require(try state.createNote(content: "lock me"))
        try expectItSupersedesARefresh(state) { try state.toggleLock(id: note.id) }
    }

    @Test func duplicatingSupersedesARunningRefresh() async throws {
        let state = try await makeState()
        let note = try #require(try state.createNote(content: "copy me"))
        try expectItSupersedesARefresh(state) { try state.duplicateNote(id: note.id) }
    }

    // MARK: - A note lives in one list or the other (jottery-lv81)

    private func archived(_ state: AppState, content: String = "archived") throws -> DecryptedNote {
        let note = try #require(try state.createNote(content: content))
        try state.archiveNote(id: note.id)
        return try #require(state.archivedNotes.first { $0.id == note.id })
    }

    @Test func savingAnArchivedNoteUpdatesTheArchive() async throws {
        let state = try await makeState()
        var note = try archived(state)
        note.content = "edited in the archive"

        try state.saveNote(note)

        #expect(state.archivedNotes.first { $0.id == note.id }?.content == "edited in the archive")
    }

    @Test func pinningAnArchivedNoteUpdatesTheArchive() async throws {
        let state = try await makeState()
        let note = try archived(state)

        try state.togglePin(id: note.id)

        #expect(state.archivedNotes.first { $0.id == note.id }?.pinned == true)
    }

    @Test func deletingAnArchivedNoteTakesItFromTheArchive() async throws {
        let state = try await makeState()
        let note = try archived(state)

        try state.deleteNote(id: note.id)

        #expect(!state.archivedNotes.contains { $0.id == note.id })
    }
}

/// Duplicate used to keep only the content and tags (jottery-lv81).
@MainActor
struct DuplicateNoteTests {

    private let password = "notes-password-correct-horse"

    private func makeState() async throws -> AppState {
        let state = try TestVaults.makeState()
        try await state.createVault(password: password)
        return state
    }

    private func duplicate(of note: DecryptedNote, in state: AppState) throws -> DecryptedNote {
        try state.duplicateNote(id: note.id)
        return try #require(state.notes.first { $0.id != note.id && $0.id == state.selectedNoteId })
    }

    @Test func aCopyKeepsItsTypeAndColour() async throws {
        let state = try await makeState()
        var note = try #require(try state.createNote(content: "2 + 2", tags: ["maths"]))
        note.syntaxLanguage = "calc"
        note.color = "green"
        note.wordWrap = false
        note.showPreview = true
        try state.saveNote(note)

        let copy = try duplicate(of: note, in: state)

        #expect(copy.syntaxLanguage == "calc")
        #expect(copy.color == "green")
        #expect(copy.wordWrap == false)
        #expect(copy.showPreview == true)
        #expect(copy.content == "2 + 2")
        #expect(copy.tags == ["maths"])
    }

    /// As on the web: a copy starts out as an ordinary note.
    @Test func aCopyIsntPinned() async throws {
        let state = try await makeState()
        let note = try #require(try state.createNote(content: "pinned"))
        try state.togglePin(id: note.id)

        let copy = try duplicate(of: note, in: state)

        #expect(!copy.pinned)
    }

    @Test func aCopyHasItsOwnAttachments() async throws {
        let state = try await makeState()
        let note = try #require(try state.createNote(content: "with a file"))
        try state.addAttachment(to: note.id, data: Data("the file".utf8), filename: "file.txt", mimeType: "text/plain")
        let original = try #require(state.notes.first { $0.id == note.id })
        let ref = try #require(original.attachments.first)

        let copy = try duplicate(of: original, in: state)

        let copied = try #require(copy.attachments.first)
        #expect(copied.filename == "file.txt")
        #expect(copied.id != ref.id, "sharing an id means removing it from one takes it from both")

        try state.removeAttachment(from: original.id, attachmentId: ref.id)

        let key = try #require(state.keyManager.masterKey)
        let blob = try #require(try state.attachmentRepo?.getBlob(id: copied.data))
        let stored = try CryptoService.parseEncryptedJSON(String(decoding: blob, as: UTF8.self))
        #expect(String(decoding: try CryptoService.decrypt(stored, key: key), as: UTF8.self) == "the file")
    }

    /// An image in the text names its attachment, so it has to name the copy, or
    /// it breaks as soon as the original is removed.
    @Test func aCopysTextPointsAtItsOwnAttachments() async throws {
        let state = try await makeState()
        let note = try #require(try state.createNote(content: "placeholder"))
        try state.addAttachment(to: note.id, data: Data("picture".utf8), filename: "a.png", mimeType: "image/png")
        var original = try #require(state.notes.first { $0.id == note.id })
        let ref = try #require(original.attachments.first)
        original.content = "![a](attachment:\(ref.id))"
        try state.saveNote(original)

        let copy = try duplicate(of: original, in: state)

        let copied = try #require(copy.attachments.first)
        #expect(copy.content == "![a](attachment:\(copied.id))")
    }

    @Test func aCopyHoldsWhatsInTheEditorNow() async throws {
        let state = try await makeState()
        var note = try #require(try state.createNote(content: "as saved"))
        note.content = "typed but not saved yet"
        state.pendingEditorNote = note

        let copy = try duplicate(of: note, in: state)

        #expect(copy.content == "typed but not saved yet")
    }

    /// The hash covers the type and attachments, so it has to be of the copy as it is.
    @Test func aCopysHashCoversWhatWasCopied() async throws {
        let state = try await makeState()
        var note = try #require(try state.createNote(content: "same text"))
        let plainCopy = try duplicate(of: note, in: state)
        note.syntaxLanguage = "calc"
        try state.saveNote(note)

        let calcCopy = try duplicate(of: note, in: state)

        #expect(calcCopy.contentHash != nil)
        #expect(calcCopy.contentHash != plainCopy.contentHash)
    }
}

/// Swiping the import sheet away skipped the reload, which only the toolbar button
/// ran, and any failure was dropped (jottery-8d6d).
@MainActor
struct ImportReloadTests {

    @Test func theListsCatchUpAfterAnImport() async throws {
        let state = try TestVaults.makeState()
        try await state.createVault(password: "notes-password-correct-horse")
        let repo = try #require(state.noteRepo)
        let key = try #require(state.keyManager.masterKey)
        _ = try repo.create(content: "imported behind the list's back", key: key)
        #expect(state.notes.isEmpty)

        await state.reloadAfterImport()

        #expect(state.notes.count == 1)
    }

    @Test func aReloadThatFailsIsReported() async throws {
        let state = try TestVaults.makeState()
        try await state.createVault(password: "notes-password-correct-horse")
        // A note that won't decrypt is skipped, not an error, so the read has to fail.
        try TestVaults.execute(state, "ALTER TABLE notes RENAME TO notes_gone", [])

        await state.reloadAfterImport()

        #expect(state.userErrorMessage != nil, "the list is out of date, and the user should know")
    }
}
