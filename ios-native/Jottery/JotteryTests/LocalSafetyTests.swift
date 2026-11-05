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

    /// Before opening for a vault that's already there, and after for one that's
    /// new, because the first run has only just created the files.
    @Test func openingTheVaultsDatabaseProtectsItsFilesBothTimes() throws {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-protection-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let path = dir.appendingPathComponent("test.db").path
        _ = try DatabaseManager(path: path)

        var protected: [String] = []
        _ = try DatabaseManager(protecting: path) { protected.append($0) }

        #expect(protected.filter { $0 == path }.count == 2)
    }

    /// What's waiting in the shared inbox is plaintext, and a wipe that left it
    /// behind would keep what the user asked to have removed.
    @Test func wipingRemovesWhatTheShareExtensionLeft() throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-wipe-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        try SharedInboxStore.write(text: "shared", urls: [], files: [], in: root)
        let state = try makeUnlockedState()

        state.wipeAllData(sharedInboxRoot: root)

        #expect(SharedInboxStore.pendingItems(in: root).isEmpty)
    }

    // MARK: - Filenames at rest (R2-070, R2-092)

    @Test func anAttachmentsNameDoesntReachThePlaintextColumn() throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "with a file"))

        try state.addAttachment(to: note.id, data: Data("x".utf8), filename: "divorce-petition.pdf", mimeType: "application/pdf")

        let names = try state.db?.dbPool.read { try String.fetchAll($0, sql: "SELECT filename FROM attachments") }
        #expect(names == [""])
        let stored = try #require(try state.noteRepo?.getRaw(id: note.id)?.attachments)
        #expect(!stored.contains("divorce-petition"))
        #expect(state.displayedNote(id: note.id)?.attachments.first?.filename == "divorce-petition.pdf")
    }

    /// A database as an older build left it: one attachment with a filename, and
    /// `ios_009_local_sync_tracking` still to run.
    private func databaseBeforeSyncTracking(extra: (Database) throws -> Void = { _ in }) throws -> String {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-migration-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let path = dir.appendingPathComponent("test.db").path
        let db = try DatabaseManager(path: path)
        try db.dbPool.write { db in
            try db.execute(sql: """
                INSERT INTO attachments (id, filename, mime_type, size, data)
                VALUES ('a', 'passport-scan.jpg', 'image/jpeg', 1, x'00')
            """)
            try db.execute(sql: "ALTER TABLE attachments DROP COLUMN uploaded")
            try db.execute(sql: "ALTER TABLE notes DROP COLUMN local_deleted_at")
            try db.execute(sql: "ALTER TABLE notes DROP COLUMN from_inbox")
            try db.execute(sql: "DELETE FROM grdb_migrations WHERE identifier = 'ios_009_local_sync_tracking'")
            try extra(db)
        }
        try db.dbPool.close()
        return path
    }

    /// Attachment data lives in the row, so changing a row's size rewrites the data
    /// too. A migration runs at launch, and on a real vault that took long enough
    /// for iOS to kill the app, on every launch: the rewrite rolled back each time.
    /// A trigger shows it whatever the size of the data, with no timing involved.
    @Test func aMigrationNeverRewritesAttachments() throws {
        let path = try databaseBeforeSyncTracking { db in
            try db.execute(sql: "CREATE TABLE attachment_writes (id TEXT)")
            try db.execute(sql: """
                CREATE TRIGGER count_attachment_writes AFTER UPDATE ON attachments
                BEGIN INSERT INTO attachment_writes VALUES (new.id); END
            """)
        }

        let upgraded = try DatabaseManager(path: path)

        let writes = try upgraded.dbPool.read { try Int.fetchOne($0, sql: "SELECT COUNT(*) FROM attachment_writes") }
        #expect(writes == 0)
    }

    /// Runs the real migration over a name an older build left behind.
    @Test func namesAlreadyInTheColumnAreClearedAfterTheUpgrade() throws {
        let upgraded = try DatabaseManager(path: try databaseBeforeSyncTracking())
        let repo = AttachmentRepository(db: upgraded)

        #expect(try repo.clearOldFilenames(limit: 5) == 1)

        let names = try upgraded.dbPool.read { try String.fetchAll($0, sql: "SELECT filename FROM attachments") }
        #expect(names == [""])
    }

    @Test func oldNamesAreClearedAFewAtATime() throws {
        let db = try DatabaseManager(path: try databaseBeforeSyncTracking())
        try db.dbPool.write { db in
            for index in 0..<11 {
                try db.execute(sql: """
                    INSERT INTO attachments (id, filename, mime_type, size, data)
                    VALUES (?, 'scan.jpg', 'image/jpeg', 1, x'00')
                """, arguments: ["attachment-\(index)"])
            }
        }
        let repo = AttachmentRepository(db: db)

        #expect(try repo.clearOldFilenames(limit: 5) == 5)
        #expect(try repo.clearOldFilenames(limit: 5) == 5)
        #expect(try repo.clearOldFilenames(limit: 5) == 2)
        #expect(try repo.clearOldFilenames(limit: 5) == 0)
    }

    /// A note binned before this device kept its own time only has `deleted_at`,
    /// which a server chooses. Taking that as the time would let a note it dated
    /// long ago be purged at the first unlock after the upgrade (R2-168).
    @Test func aNoteAlreadyInTheBinGetsItsThirtyDaysFromTheUpgrade() throws {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-migration-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let path = dir.appendingPathComponent("test.db").path
        let key = SymmetricKey(data: Data(repeating: 7, count: 32))
        let id: String
        do {
            let db = try DatabaseManager(path: path)
            let versionRepo = VersionRepository(db: db)
            let repo = NoteRepository(db: db, versionRepo: versionRepo)
            let note = try repo.create(content: "binned by an older version", key: key)
            id = note.id
            try db.dbPool.write { db in
                try db.execute(
                    sql: "UPDATE notes SET deleted = 1, deleted_at = '2000-01-01T00:00:00Z', needs_sync = 0 WHERE id = ?",
                    arguments: [note.id]
                )
                try db.execute(sql: "ALTER TABLE attachments DROP COLUMN uploaded")
                try db.execute(sql: "ALTER TABLE notes DROP COLUMN local_deleted_at")
                try db.execute(sql: "ALTER TABLE notes DROP COLUMN from_inbox")
                try db.execute(sql: "DELETE FROM grdb_migrations WHERE identifier = 'ios_009_local_sync_tracking'")
            }
            try db.dbPool.close()
        }

        let upgraded = try DatabaseManager(path: path)
        let repo = NoteRepository(db: upgraded, versionRepo: VersionRepository(db: upgraded))

        #expect(try repo.purgeOldDeletedNotes() == 0, "purged at the first unlock, on the server's word")
        #expect(try repo.getRaw(id: id)?.deleted == true)
        #expect(try repo.purgeOldDeletedNotes(olderThan: -3600) == 1, "it does still get purged in the end")
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

    /// It's the only copy of the edit, so failing again mustn't lose it.
    @Test func aFlushThatFailsKeepsTheEdit() async throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "before"))
        var fields = NoteEditorFields(note: note)
        fields.content = "the edit"
        try await state.db?.dbPool.write { db in
            try db.execute(sql: "CREATE TRIGGER no_writes BEFORE UPDATE ON notes BEGIN SELECT RAISE(ABORT, 'disk full'); END")
        }
        state.saveEditorEdit(fields)

        #expect(!state.flushPendingEditorNote())

        #expect(state.pendingEditorNote?.content == "the edit")
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

/// Working on many notes at once (jottery-6jn7).
@MainActor
struct BulkOperationTests {

    private func makeState() throws -> (AppState, [DecryptedNote]) {
        let state = try TestVaults.makeState()
        state.keyManager.unlockWithKeyData(Data(repeating: 7, count: 32))
        state.isLocked = false
        let notes = try (0..<3).compactMap { try state.createNote(content: "note \($0)", tags: ["old"]) }
        return (state, notes)
    }

    @Test func manyNotesGoToTheBinTogether() async throws {
        let (state, notes) = try makeState()
        state.selectedNoteId = notes[0].id

        try await state.deleteNotes(ids: [notes[0].id, notes[1].id])

        #expect(state.notes.map(\.id) == [notes[2].id])
        #expect(state.selectedNoteId == nil)
        #expect(try state.noteRepo?.getRaw(id: notes[0].id)?.localDeletedAt != nil)
    }

    @Test func manyNotesAreArchivedTogether() async throws {
        let (state, notes) = try makeState()

        try await state.archiveNotes(ids: Set(notes.map(\.id)))

        #expect(state.notes.isEmpty)
        #expect(state.archivedNotes.count == 3)
    }

    /// One transaction, so a failure can't leave some of them changed.
    @Test func aBatchThatFailsChangesNothing() async throws {
        let (state, notes) = try makeState()
        try await state.db?.dbPool.write { db in
            try db.execute(sql: """
                CREATE TRIGGER no_archive BEFORE UPDATE ON notes WHEN NEW.id = '\(notes[2].id)'
                BEGIN SELECT RAISE(ABORT, 'disk full'); END
            """)
        }

        await #expect(throws: (any Error).self) {
            try await state.archiveNotes(ids: Set(notes.map(\.id)))
        }

        try await state.loadNotes()
        #expect(state.archivedNotes.isEmpty)
    }

    @Test func aChangeReachesEveryNoteAndSkipsTheOnesItDoesntApplyTo() async throws {
        let (state, notes) = try makeState()
        let modified = try #require(try state.noteRepo?.getRaw(id: notes[2].id)?.contentHash)

        let failures = try await state.updateNotes(ids: Set(notes.map(\.id))) { note in
            guard note.content != "note 2" else { return false }
            note.tags.append("new")
            return true
        }

        #expect(failures == 0)
        #expect(state.notes.filter { $0.tags.contains("new") }.count == 2)
        #expect(try state.noteRepo?.getRaw(id: notes[2].id)?.contentHash == modified)
    }
}

/// Moving the vault onto a new key off the main thread (jottery-emoe).
@MainActor
@Suite(.serialized)
struct ReKeyInProgressTests {

    private func makeVault() async throws -> AppState {
        let state = try TestVaults.makeState()
        try await state.createVault(password: "notes-password-correct-horse")
        return state
    }

    @Test func nothingCanBeSavedWhileTheVaultIsBeingReEncrypted() async throws {
        let state = try await makeVault()
        let note = try #require(try state.createNote(content: "before"))

        state.isReKeying = true
        defer { state.isReKeying = false }

        #expect(throws: AppStateError.self) { try state.saveNote(note) }
        #expect(throws: AppStateError.self) { try state.createNote(content: "new") }
    }

    /// Saved under the old key, it would be unreadable after the switch.
    @Test func anEditHeldBackDuringAReKeyIsSavedUnderTheNewKey() async throws {
        let state = try await makeVault()
        let note = try #require(try state.createNote(content: "before"))
        var fields = NoteEditorFields(note: note)
        fields.content = "held back"
        let gate = AsyncGate()
        state.willReKey = { await gate.wait() }

        let keySwitch = Task {
            try await state.switchVaultToMasterKey(CryptoService.generateMasterKey(), notesPassword: "notes-password-correct-horse")
        }
        await waitUntil { await gate.waitingCount == 1 }
        #expect(!state.saveEditorEdit(fields))
        await gate.open()
        try await keySwitch.value

        let key = try #require(state.keyManager.masterKey)
        #expect(try state.noteRepo?.get(id: note.id, key: key)?.content == "held back")
        #expect(state.pendingEditorNote == nil)
    }

    @Test func aReKeyLeavesTheVaultUnlockedAndNoLongerBusy() async throws {
        let state = try await makeVault()
        _ = try state.createNote(content: "kept")

        try await state.switchVaultToMasterKey(CryptoService.generateMasterKey(), notesPassword: "notes-password-correct-horse")

        #expect(!state.isReKeying)
        #expect(state.notes.map(\.content) == ["kept"])
    }

    /// The app can lock while this runs off the main thread, and the new key
    /// mustn't then appear in memory behind the lock screen.
    @Test func lockingDuringAReKeyLeavesNoKeyBehind() async throws {
        let state = try await makeVault()
        _ = try state.createNote(content: "kept")
        let gate = AsyncGate()
        state.willReKey = { await gate.wait() }

        let keySwitch = Task {
            try await state.switchVaultToMasterKey(CryptoService.generateMasterKey(), notesPassword: "notes-password-correct-horse")
        }
        await waitUntil { await gate.waitingCount == 1 }
        #expect(state.isReKeying)
        state.lock()
        await gate.open()
        try await keySwitch.value

        #expect(state.keyManager.masterKey == nil)
        #expect(state.isLocked)

        try await state.unlock(password: "notes-password-correct-horse")
        #expect(state.notes.map(\.content) == ["kept"])
    }
}

/// Key bytes in `Data` (jottery-dm2m).
struct KeyWipeTests {
    @Test func wipingOverwritesEveryByte() {
        var bytes = Data(repeating: 7, count: 32)

        bytes.wipe()

        #expect(bytes == Data(repeating: 0, count: 32))
    }
}
