import Foundation
import Testing

@testable import Jottery

/// A permanent deletion waits in a queue until the next push. If the note comes
/// back first, from a backup or an import, sending it would delete the note
/// again, on every device.
@MainActor
struct StaleDeletionTests {

    private func makeVault() async throws -> AppState {
        let state = try TestVaults.makeState()
        try await state.createVault(password: UUID().uuidString)
        return state
    }

    @Test func aNoteRestoredFromABackupIsNotDeletedByTheNextPush() async throws {
        let state = try await makeVault()
        let noteRepo = try #require(state.noteRepo)
        let syncRepo = try #require(state.syncRepo)
        let note = try #require(try state.createNote(content: "worth keeping", tags: []))
        let backedUp = try #require(try noteRepo.getRaw(id: note.id))

        try noteRepo.permanentlyDelete(id: note.id)
        #expect(try syncRepo.getPendingDeletions().map(\.id) == [note.id])

        _ = try noteRepo.restore([backedUp])

        #expect(try syncRepo.getPendingDeletions().isEmpty)
    }

    @Test func aDeletionStillGoesUpWhenTheNoteStaysGone() async throws {
        let state = try await makeVault()
        let noteRepo = try #require(state.noteRepo)
        let syncRepo = try #require(state.syncRepo)
        let note = try #require(try state.createNote(content: "really gone", tags: []))

        try noteRepo.permanentlyDelete(id: note.id)

        #expect(try syncRepo.getPendingDeletions().map(\.id) == [note.id])
    }

    @Test func deletingARestoredNoteAgainQueuesItAgain() async throws {
        let state = try await makeVault()
        let noteRepo = try #require(state.noteRepo)
        let syncRepo = try #require(state.syncRepo)
        let note = try #require(try state.createNote(content: "changed my mind twice", tags: []))
        let backedUp = try #require(try noteRepo.getRaw(id: note.id))

        try noteRepo.permanentlyDelete(id: note.id)
        _ = try noteRepo.restore([backedUp])
        try noteRepo.permanentlyDelete(id: note.id)

        #expect(try syncRepo.getPendingDeletions().map(\.id) == [note.id])
    }
}
