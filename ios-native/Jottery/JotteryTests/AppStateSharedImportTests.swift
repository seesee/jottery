import CryptoKit
import Foundation
import Testing

@testable import Jottery

@MainActor
struct AppStateSharedImportTests {

    private func makeUnlockedState() throws -> AppState {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-import-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let state = AppState()
        try state.initialise(database: DatabaseManager(path: dir.appendingPathComponent("test.db").path))
        state.keyManager.unlockWithKeyData(Data(repeating: 7, count: 32))
        state.isLocked = false
        return state
    }

    private func makeRoot() throws -> URL {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("import-root-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        return root
    }

    @Test func importsStagedItemAsNoteWithAttachment() throws {
        let state = try makeUnlockedState()
        let root = try makeRoot()
        try SharedInboxStore.write(
            text: "Shared from Safari",
            urls: ["https://example.com/article"],
            files: [(data: Data([0x89, 0x50, 0x4E, 0x47]), filename: "photo.png", mimeType: "image/png")],
            in: root
        )

        state.importSharedInboxItems(from: root)

        let note = try #require(state.notes.first)
        #expect(note.content.contains("Shared from Safari"))
        #expect(note.content.contains("https://example.com/article"))
        #expect(note.attachments.count == 1)
        #expect(note.attachments.first?.filename == "photo.png")
        // Item is consumed — a second import must not duplicate it.
        state.importSharedInboxItems(from: root)
        #expect(state.notes.count == 1)
    }

    @Test func importSkipsWhileLocked() throws {
        let state = try makeUnlockedState()
        let root = try makeRoot()
        try SharedInboxStore.write(text: "held", urls: [], files: [], in: root)

        state.isLocked = true
        state.importSharedInboxItems(from: root)

        #expect(state.notes.isEmpty)
        #expect(SharedInboxStore.pendingItems(in: root).count == 1)  // still staged
    }

    @Test func importPreservesNoteSelection() throws {
        let state = try makeUnlockedState()
        let existing = try #require(try state.createNote(content: "current work"))
        state.selectedNoteId = existing.id

        let root = try makeRoot()
        try SharedInboxStore.write(text: "incoming", urls: [], files: [], in: root)
        state.importSharedInboxItems(from: root)

        #expect(state.selectedNoteId == existing.id)
        #expect(state.notes.count == 2)
    }

    /// A partial failure (note created, then an attachment throws because its
    /// staged file is unreadable) must not duplicate the note when the item
    /// is retried on a later import pass.
    @Test func retryAfterPartialFailureDoesNotDuplicateNote() throws {
        let state = try makeUnlockedState()
        let root = try makeRoot()
        try SharedInboxStore.write(
            text: "Partial failure item",
            urls: [],
            files: [(data: Data([0x89, 0x50, 0x4E, 0x47]), filename: "photo.png", mimeType: "image/png")],
            in: root
        )

        // Sabotage the staged attachment so addAttachment throws mid-import,
        // simulating a crash/failure after the note was already created.
        let item = try #require(SharedInboxStore.pendingItems(in: root).first)
        let fileURL = item.directory.appendingPathComponent(item.manifest.files[0].storedName)
        try FileManager.default.removeItem(at: fileURL)

        state.importSharedInboxItems(from: root)
        #expect(state.notes.count == 1)
        #expect(SharedInboxStore.pendingItems(in: root).count == 1)  // left staged for retry

        // Retry: must resume onto the same note, not create a duplicate.
        state.importSharedInboxItems(from: root)
        #expect(state.notes.count == 1)
        let note = try #require(state.notes.first)
        #expect(note.content.contains("Partial failure item"))
        #expect(note.attachments.isEmpty)
        #expect(SharedInboxStore.pendingItems(in: root).count == 1)  // still staged — attachment still missing
    }

    // MARK: - Resuming (jottery-bp2j)

    /// Staged, with the note made and the attachment still to come.
    private func stageHalfImported(in state: AppState, root: URL) throws -> String {
        try SharedInboxStore.write(
            text: "Shared and then thought better of",
            urls: [],
            files: [(data: Data([0x01]), filename: "photo.png", mimeType: "image/png")],
            in: root
        )
        let item = try #require(SharedInboxStore.pendingItems(in: root).first)
        let note = try #require(try state.createNote(content: "Shared and then thought better of"))
        try SharedInboxStore.markImported(item.directory, noteId: note.id)
        return note.id
    }

    @Test func aNoteDeletedBetweenTriesIsLeftDeleted() throws {
        let state = try makeUnlockedState()
        let root = try makeRoot()
        let id = try stageHalfImported(in: state, root: root)
        try state.deleteNote(id: id)

        state.importSharedInboxItems(from: root)

        #expect(state.notes.isEmpty, "the note was deleted on purpose")
        let key = try #require(state.keyManager.masterKey)
        #expect(try state.noteRepo?.get(id: id, key: key)?.attachments.isEmpty == true)
        #expect(SharedInboxStore.pendingItems(in: root).isEmpty)
    }

    @Test func aNotePermanentlyDeletedBetweenTriesIsNotMadeAgain() throws {
        let state = try makeUnlockedState()
        let root = try makeRoot()
        let id = try stageHalfImported(in: state, root: root)
        try state.deleteNote(id: id)
        try state.permanentlyDeleteNote(id: id)

        state.importSharedInboxItems(from: root)

        #expect(state.notes.isEmpty)
        #expect(SharedInboxStore.pendingItems(in: root).isEmpty)
    }

    @Test func filesWithTheSameNameAllArrive() throws {
        let state = try makeUnlockedState()
        let root = try makeRoot()
        try SharedInboxStore.write(
            text: nil,
            urls: [],
            files: [
                (data: Data([0x01]), filename: "IMG.jpg", mimeType: "image/jpeg"),
                (data: Data([0x02]), filename: "IMG.jpg", mimeType: "image/jpeg"),
            ],
            in: root
        )
        // The first pass gets as far as the first file.
        let item = try #require(SharedInboxStore.pendingItems(in: root).first)
        let second = item.directory.appendingPathComponent(item.manifest.files[1].storedName)
        let secondData = try Data(contentsOf: second)
        try FileManager.default.removeItem(at: second)
        state.importSharedInboxItems(from: root)
        #expect(state.notes.first?.attachments.count == 1)

        try secondData.write(to: second)
        state.importSharedInboxItems(from: root)

        #expect(state.notes.count == 1)
        #expect(state.notes.first?.attachments.count == 2, "the second has the same name, and is another file")
        #expect(SharedInboxStore.pendingItems(in: root).isEmpty)
    }
}
