import CryptoKit
import Foundation
import Testing

@testable import Jottery

@MainActor
struct ArchivedNoteLookupTests {

    private func makeUnlockedState() throws -> AppState {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-archived-lookup-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let state = AppState()
        try state.initialise(database: DatabaseManager(path: dir.appendingPathComponent("test.db").path))
        state.keyManager.unlockWithKeyData(Data(repeating: 7, count: 32))
        state.isLocked = false
        return state
    }

    @Test func displayedNoteResolvesArchivedNotes() throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "to archive"))
        try state.archiveNote(id: note.id)

        #expect(state.notes.first(where: { $0.id == note.id }) == nil)  // pins the trigger
        let resolved = try #require(state.displayedNote(id: note.id))
        #expect(resolved.content == "to archive")
        #expect(resolved.archived == true)
    }

    @Test func displayedNoteResolvesActiveNotes() throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "active"))
        #expect(state.displayedNote(id: note.id)?.content == "active")
    }
}
