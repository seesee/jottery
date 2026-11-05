import CryptoKit
import Foundation
import GRDB
import Testing

@testable import Jottery

/// Inbox requests, scoped to this file: one DELETE fails while `failDeletes` is set.
private final class InboxStubProtocol: URLProtocol, @unchecked Sendable {
    nonisolated(unsafe) static var failDeletes = false

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        let status = request.httpMethod == "DELETE" && Self.failDeletes ? 500 : 200
        let response = HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: "HTTP/1.1", headerFields: nil)!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: Data("{}".utf8))
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}
}

@Suite(.serialized)
@MainActor
struct LocalSafetyTests {

    private func makeUnlockedState() throws -> AppState {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-local-safety-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let state = AppState()
        try state.initialise(database: DatabaseManager(path: dir.appendingPathComponent("test.db").path))
        state.keyManager.unlockWithKeyData(Data(repeating: 7, count: 32))
        state.isLocked = false
        return state
    }

    // MARK: - Wiping from the lock screen (R2-094)

    @Test func theLockScreenCantWipeWithoutTheDeviceOwner() async throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "keep me"))

        let wiped = await state.wipeAfterProvingOwnership(authenticate: { _ in false })

        #expect(!wiped)
        #expect(try state.noteRepo?.getRaw(id: note.id) != nil)
    }

    @Test func aVaultWithoutSyncCountsEveryNoteAsOnlyHere() throws {
        let state = try makeUnlockedState()
        let synced = try #require(try state.createNote(content: "one"))
        _ = try state.createNote(content: "two")
        try state.noteRepo?.markSynced(id: synced.id, syncedAt: Date().iso8601)

        #expect(state.notesOnlyOnThisDevice() == 2)

        state.settings.syncEnabled = true
        #expect(state.notesOnlyOnThisDevice() == 1)
    }

    @Test func theWarningSaysHowManyNotesWouldBeLost() {
        #expect(L.unlockDeleteConfirmMessage(unsyncedNotes: 0) == L.unlockDeleteConfirmMessage)
        #expect(L.unlockDeleteConfirmMessage(unsyncedNotes: 3).contains("3"))
    }

    // MARK: - The database at rest (R2-091)

    @Test func theDatabaseIsOnlyReadableWhileOpen() throws {
        #expect(DatabaseManager.fileProtection == .completeUnlessOpen)

        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-protection-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let path = dir.appendingPathComponent("test.db").path
        let db = try DatabaseManager(path: path)
        try db.dbPool.write { try $0.execute(sql: "UPDATE settings SET theme = 'dark'") }

        var protected: [String] = []
        try DatabaseManager.protectFiles(ofDatabaseAt: path) { protected.append($0) }

        #expect(protected == [dir.path, path, path + "-wal", path + "-shm"])
    }

    // MARK: - The inbox (jottery-0szd)

    private func makeInboxState() throws -> (AppState, InboxItem) {
        let state = try makeUnlockedState()
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [InboxStubProtocol.self]
        state.syncClient = SyncClient(endpoint: "https://sync.example.test", apiKey: "key", configuration: configuration)
        let item = InboxItem(id: UUID().uuidString, content: "from the inbox", tags: ["web"], createdAt: Date().iso8601)
        state.inboxItems = [item]
        return (state, item)
    }

    /// The item used to vanish here and return on the next load, and accepting
    /// it again made a second note.
    @Test func acceptingAgainAfterAFailedServerDeleteMakesNoSecondNote() async throws {
        let (state, item) = try makeInboxState()
        InboxStubProtocol.failDeletes = true

        await #expect(throws: (any Error).self) { try await state.acceptInboxItem(item) }

        #expect(state.inboxItems.map(\.id) == [item.id])
        #expect(state.notes.count == 1)

        InboxStubProtocol.failDeletes = false
        try await state.acceptInboxItem(item)

        #expect(state.inboxItems.isEmpty)
        #expect(state.notes.count == 1)
        #expect(try state.noteRepo?.countActive() == 1)
    }

    @Test func anAcceptedItemIsMarkedAsComingFromTheInbox() async throws {
        let (state, item) = try makeInboxState()
        InboxStubProtocol.failDeletes = false

        try await state.acceptInboxItem(item)

        let record = try #require(try state.noteRepo?.getRaw(id: item.id))
        #expect(record.fromInbox)
        #expect(!record.showPreview)
    }

    /// Whoever turns the preview on here has decided to trust the note.
    @Test func turningThePreviewOnHereClearsTheInboxMark() async throws {
        let (state, item) = try makeInboxState()
        InboxStubProtocol.failDeletes = false
        try await state.acceptInboxItem(item)
        var note = try #require(state.notes.first)

        note.showPreview = true
        try state.saveNote(note)

        #expect(try state.noteRepo?.getRaw(id: item.id)?.fromInbox == false)
    }

    // MARK: - Saves that fail (jottery-2y75)

    @Test func aFailedEditorSaveKeepsTheEditAndSaysSo() async throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "before"))
        var fields = NoteEditorFields(note: note)
        fields.content = "the edit"
        try await state.db?.dbPool.write { db in
            try db.execute(sql: "CREATE TRIGGER no_writes BEFORE UPDATE ON notes BEGIN SELECT RAISE(ABORT, 'disk full'); END")
        }

        let saved = state.saveEditorEdit(fields)

        #expect(!saved)
        #expect(state.pendingEditorNote?.content == "the edit")
        #expect(state.userErrorMessage == L.errorCouldntSaveNote)
    }

    @Test func aRetriedSaveGoesThroughOnceTheProblemIsGone() async throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "before"))
        var fields = NoteEditorFields(note: note)
        fields.content = "the edit"
        try await state.db?.dbPool.write { db in
            try db.execute(sql: "CREATE TRIGGER no_writes BEFORE UPDATE ON notes BEGIN SELECT RAISE(ABORT, 'disk full'); END")
        }
        state.saveEditorEdit(fields)
        try await state.db?.dbPool.write { try $0.execute(sql: "DROP TRIGGER no_writes") }

        #expect(state.flushPendingEditorNote())

        let key = try #require(state.keyManager.masterKey)
        #expect(try state.noteRepo?.get(id: note.id, key: key)?.content == "the edit")
        #expect(state.pendingEditorNote == nil)
    }

    @Test func aSuccessfulSaveClearsWhatWasStaged() throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "before"))
        state.pendingEditorNote = note
        var fields = NoteEditorFields(note: note)
        fields.content = "the edit"

        #expect(state.saveEditorEdit(fields))

        #expect(state.pendingEditorNote == nil)
    }

    // MARK: - Settings (jottery-weuk)

    @Test func theSortChoiceIsKept() throws {
        let state = try makeUnlockedState()

        state.setSortOrder(.alpha)

        #expect(state.sortOrder == .alpha)
        #expect(try state.settingsRepo?.get().sort == .alpha)
    }

    @Test func eachThemeHasItsColourScheme() {
        #expect(ThemeMode.dark.colorScheme == .dark)
        #expect(ThemeMode.light.colorScheme == .light)
        #expect(ThemeMode.auto.colorScheme == nil)
    }
}
