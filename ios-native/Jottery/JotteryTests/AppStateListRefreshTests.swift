import CryptoKit
import Foundation
import Testing

@testable import Jottery

@MainActor
struct AppStateListRefreshTests {

    private func makeUnlockedState() throws -> AppState {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-list-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let state = AppState()
        try state.initialise(database: DatabaseManager(path: dir.appendingPathComponent("test.db").path))
        state.keyManager.unlockWithKeyData(Data(repeating: 7, count: 32))
        state.isLocked = false
        return state
    }

    @Test func saveNoteUpdatesFilteredNotes() throws {
        let state = try makeUnlockedState()
        var note = try #require(try state.createNote(content: "original title"))
        state.scheduleSearch()  // baseline — the view does this when the count changes
        #expect(state.filteredNotes.first?.content == "original title")

        note.content = "updated title"
        try state.saveNote(note)

        // The note list renders filteredNotes — an edit must be visible
        // there without waiting for a manual refresh or sync.
        #expect(state.filteredNotes.first?.content == "updated title")
    }

    @Test func togglePinUpdatesFilteredNotes() throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "pin me"))
        state.scheduleSearch()
        #expect(state.filteredNotes.first?.pinned == false)

        try state.togglePin(id: note.id)

        #expect(state.filteredNotes.first?.pinned == true)
    }

    @Test func addAttachmentUpdatesFilteredNotes() throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "with file"))
        state.scheduleSearch()
        #expect(state.filteredNotes.first?.attachments.isEmpty == true)

        try state.addAttachment(
            to: note.id,
            data: Data([0x89, 0x50, 0x4E, 0x47]),
            filename: "photo.png",
            mimeType: "image/png"
        )

        #expect(state.filteredNotes.first?.attachments.count == 1)
    }
}
