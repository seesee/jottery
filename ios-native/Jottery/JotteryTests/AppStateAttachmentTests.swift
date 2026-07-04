import CryptoKit
import Foundation
import Testing

@testable import Jottery

@MainActor
struct AppStateAttachmentTests {

    /// AppState wired to a throwaway on-disk database with an in-memory key.
    private func makeUnlockedState() throws -> AppState {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let state = AppState()
        try state.initialise(database: DatabaseManager(path: dir.appendingPathComponent("test.db").path))
        state.keyManager.unlockWithKeyData(Data(repeating: 7, count: 32))
        state.isLocked = false
        return state
    }

    private func makeTempFile(named name: String) throws -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("\(UUID().uuidString)-\(name)")
        try Data([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]).write(to: url)
        return url
    }

    @Test func addAttachmentRefreshesInMemoryNote() throws {
        let state = try makeUnlockedState()
        let created = try #require(try state.createNote(content: "hello"))
        let before = try #require(state.notes.first { $0.id == created.id })

        let file = try makeTempFile(named: "pasted.png")
        try state.addAttachment(to: created.id, url: file, filename: "pasted.png", mimeType: "image/png")

        let after = try #require(state.notes.first { $0.id == created.id })
        #expect(after.attachments.count == 1)
        #expect(after.attachments.first?.filename == "pasted.png")
        // SwiftUI diffs NoteEditorView's `note` input via DecryptedNote's
        // Equatable — the updated copy must compare as changed or the
        // attachment list never re-renders.
        #expect(after != before)
    }

    @Test func removeAttachmentRefreshesInMemoryNote() throws {
        let state = try makeUnlockedState()
        let created = try #require(try state.createNote(content: "hello"))
        let file = try makeTempFile(named: "doc.pdf")
        try state.addAttachment(to: created.id, url: file, filename: "doc.pdf", mimeType: "application/pdf")

        let before = try #require(state.notes.first { $0.id == created.id })
        let attachmentId = try #require(before.attachments.first?.id)

        try state.removeAttachment(from: created.id, attachmentId: attachmentId)

        let after = try #require(state.notes.first { $0.id == created.id })
        #expect(after.attachments.isEmpty)
        #expect(after != before)
    }
}
