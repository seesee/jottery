import CryptoKit
import Foundation
import Testing

@testable import Jottery

/// Pins the contract for `AppState.allTags` (Task 4 of the iOS UX-hardening
/// plan, jottery-fxmq): the memoised set stays correct across every path
/// that publishes `notes`/`archivedNotes`, without recomputing on every
/// access (TagInputView reads `allTags` per keystroke — a per-access
/// `flatMap` over every note would make that O(notes) per character typed).
/// The no-per-access-recompute property is by construction: `allTags` is
/// rebuilt only from `didSet` on `notes`/`archivedNotes`, so a plain read
/// is an O(1) property access. These tests assert correctness of the
/// rebuilt value; the perf property isn't independently testable without
/// instrumenting the getter, so it's documented here instead.
@MainActor
struct AppStateAllTagsTests {

    private func makeUnlockedState() throws -> AppState {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-all-tags-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let state = AppState()
        try state.initialise(database: DatabaseManager(path: dir.appendingPathComponent("test.db").path))
        state.keyManager.unlockWithKeyData(Data(repeating: 7, count: 32))
        state.isLocked = false
        return state
    }

    @Test func allTagsReflectsSaveNoteTagChange() throws {
        let state = try makeUnlockedState()
        var note = try #require(try state.createNote(content: "note", tags: ["alpha"]))
        #expect(state.allTags == ["alpha"])

        note.tags = ["alpha", "beta"]
        try state.saveNote(note)

        #expect(state.allTags == ["alpha", "beta"])
    }

    @Test func allTagsReflectsApplySyncChangesRefresh() async throws {
        let state = try makeUnlockedState()
        let noteRepo = try #require(state.noteRepo)
        let key = try #require(state.keyManager.masterKey)

        let note = try noteRepo.create(content: "note", tags: ["gamma"], key: key)
        try await state.loadNotes()
        #expect(state.allTags == ["gamma"])

        var updated = note
        updated.tags = ["gamma", "delta"]
        try noteRepo.update(updated, key: key)

        await state.applySyncChanges(SyncChanges(updatedIds: [note.id]))

        #expect(state.allTags == ["delta", "gamma"])
    }

    @Test func allTagsIncludesArchivedNoteTags() throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "note", tags: ["epsilon"]))
        #expect(state.allTags == ["epsilon"])

        try state.archiveNote(id: note.id)

        // archiveNote reloads archivedNotes via loadArchivedNotes() — the
        // tag must still be visible once the note moves into the archive.
        #expect(state.allTags == ["epsilon"])
    }

    @Test func allTagsDropsTagAfterDelete() throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "note", tags: ["zeta"]))
        #expect(state.allTags == ["zeta"])

        try state.deleteNote(id: note.id)

        #expect(state.allTags.isEmpty)
    }

    @Test func allTagsResetsOnLock() throws {
        let state = try makeUnlockedState()
        _ = try #require(try state.createNote(content: "note", tags: ["eta"]))
        #expect(state.allTags == ["eta"])

        state.lock()

        #expect(state.allTags.isEmpty)
    }
}
