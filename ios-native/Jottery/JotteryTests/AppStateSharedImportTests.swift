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
}
