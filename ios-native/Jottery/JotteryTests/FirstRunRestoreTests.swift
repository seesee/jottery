import CryptoKit
import Foundation
import GRDB
import Testing

@testable import Jottery

/// Up from this file to the repository root, which the simulator can read.
private let repositoryRoot = URL(fileURLWithPath: #filePath)
    .deletingLastPathComponent().deletingLastPathComponent()
    .deletingLastPathComponent().deletingLastPathComponent()

private let tuiBackup = repositoryRoot.appendingPathComponent("tui/tests/fixtures/tui-v3.jottery-backup")
private let androidFolder = repositoryRoot.appendingPathComponent("android-native/app/src/test/resources/backup")
private let androidLegacy = androidFolder.appendingPathComponent("android-v3-legacy.jottery-backup")
private let androidEnvelope = androidFolder.appendingPathComponent("android-v3-envelope.jottery-backup")
private let webLegacy = androidFolder.appendingPathComponent("web-v3-legacy.json")
private let webEnvelope = androidFolder.appendingPathComponent("web-v3-envelope.json")

private let androidLegacyPassword = "notes-password-correct-horse"
private let androidEnvelopePassword = "notes-password-battery-staple"
private let webPassword = "fixture-password-web"

/// A restore onto a device with no vault: the vault is made from the backup, or
/// not at all (R2-150).
/// Lets a restore's progress callback cancel the task it's running in.
private final class RestoreHandle: @unchecked Sendable {
    private let lock = NSLock()
    private var task: Task<BackupService.RestoreResult, Error>?

    func hold(_ task: Task<BackupService.RestoreResult, Error>) {
        lock.withLock { self.task = task }
    }

    func cancel() {
        lock.withLock { task }?.cancel()
    }
}

@MainActor
struct FirstRunRestoreTests {

    private func tuiPassword() throws -> String {
        try String(contentsOf: tuiBackup.appendingPathExtension("password"), encoding: .utf8)
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private func rowCounts(_ state: AppState) throws -> [String: Int] {
        try #require(state.db).dbPool.read { db in
            var counts: [String: Int] = [:]
            for table in ["notes", "note_versions", "attachments", "saved_searches", "encryption_metadata"] {
                counts[table] = try Int.fetchOne(db, sql: "SELECT COUNT(*) FROM \(table)")
            }
            return counts
        }
    }

    /// The first-run screen as it was: nothing written, and a new vault still works.
    private func expectUntouched(_ state: AppState) async throws {
        #expect(state.isFirstLaunch)
        #expect(state.isLocked)
        #expect(state.keyManager.masterKey == nil)
        #expect(try rowCounts(state).values.allSatisfy { $0 == 0 })
        #expect(try stagingIsEmpty(state))

        try await state.createVault(password: "a-new-vault-\(UUID().uuidString)")
        #expect(!state.isFirstLaunch)
        #expect(state.notes.isEmpty)
    }

    private func stagingIsEmpty(_ state: AppState) throws -> Bool {
        let folder = VaultFromBackup.stagingFolder(besides: try #require(state.db))
        return !FileManager.default.fileExists(atPath: folder.path)
    }

    private func vaultKey(_ state: AppState) throws -> SymmetricKey {
        try #require(state.keyManager.masterKey)
    }

    /// A backup of a vault with `notes` notes, made here, as a file.
    private func makeBackup(password: String, notes: Int = 2) async throws -> URL {
        let state = try TestVaults.makeState()
        try await state.createVault(password: password)
        for index in 0..<notes {
            _ = try state.createNote(content: "Note \(index)", tags: ["made-here"])
        }
        return try backupFile(of: state)
    }

    private func backupFile(of state: AppState) throws -> URL {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-first-run-\(UUID().uuidString).jottery-backup")
        try BackupService.writeBackup(
            to: url,
            noteRepo: #require(state.noteRepo),
            attachmentRepo: #require(state.attachmentRepo),
            savedSearchRepo: #require(state.savedSearchRepo),
            encryptionRepo: #require(state.encryptionRepo),
            key: #require(state.keyManager.masterKey)
        )
        return url
    }

    // MARK: - Real files from other clients

    @Test func aTUIBackupBecomesTheVault() async throws {
        let state = try TestVaults.makeState()

        let result = try await state.restoreVault(fromBackup: tuiBackup, password: tuiPassword())

        #expect(result.notes == 2)
        #expect(result.skipped == 0)
        #expect(!state.isFirstLaunch)
        #expect(!state.isLocked)
        let note = try #require(state.notes.first { $0.content.contains("A note from the TUI") })
        #expect(Set(note.tags) == ["work", "cross-client"])
        let attachment = try #require(note.attachments.first)
        #expect(attachment.filename == "hello.txt")
        let blob = try #require(try state.attachmentRepo?.getBlob(id: attachment.data))
        let stored = try CryptoService.parseEncryptedJSON(String(decoding: blob, as: UTF8.self))
        let bytes = try CryptoService.decrypt(stored, key: vaultKey(state))
        #expect(String(decoding: bytes, as: UTF8.self) == "attachment body")
        try state.loadArchivedNotes()
        #expect(state.archivedNotes.contains { $0.content.contains("An archived note") })
    }

    private func checkAndroidVault(_ url: URL, password: String) async throws {
        let state = try TestVaults.makeState()

        let result = try await state.restoreVault(fromBackup: url, password: password)

        #expect(result.notes == 2)
        #expect(result.skipped == 0)
        let note = try #require(state.notes.first { $0.content == "From Android, edited once" })
        #expect(note.tags == ["android", "fixture"])
        #expect(note.attachments.first?.filename == "hello.txt")
        let versions = try #require(try state.versionRepo?.getVersions(noteId: note.id))
        #expect(versions.count == 1)
        let searches = try #require(try state.savedSearchRepo?.listAll(key: vaultKey(state)))
        #expect(searches.map(\.name) == ["Android search"])
    }

    @Test func anAndroidEnvelopeBackupBecomesTheVault() async throws {
        try await checkAndroidVault(androidEnvelope, password: androidEnvelopePassword)
    }

    @Test func anAndroidLegacyBackupBecomesTheVault() async throws {
        try await checkAndroidVault(androidLegacy, password: androidLegacyPassword)
    }

    @Test func aWebEnvelopeBackupBecomesTheVault() async throws {
        let state = try TestVaults.makeState()
        let result = try await state.restoreVault(fromBackup: webEnvelope, password: webPassword)
        #expect(result.notes == 1)
        #expect(state.notes.first?.content == "Written on the web")
    }

    @Test func aWebLegacyBackupBecomesTheVault() async throws {
        let state = try TestVaults.makeState()
        let result = try await state.restoreVault(fromBackup: webLegacy, password: webPassword)
        #expect(result.notes == 1)
        #expect(state.notes.first?.content == "Written on the web")
    }

    // MARK: - The vault that comes out

    /// The whole point of an envelope backup: this vault holds the backup's own
    /// master key, so it can join the account that key belongs to. A key of its
    /// own would look like a different vault to the server.
    @Test func theVaultKeepsTheBackupsMasterKey() async throws {
        let first = try TestVaults.makeState()
        let second = try TestVaults.makeState()

        try await first.restoreVault(fromBackup: androidEnvelope, password: androidEnvelopePassword)
        try await second.restoreVault(fromBackup: androidEnvelope, password: androidEnvelopePassword)

        let key = try #require(TestVaults.keyData(first))
        #expect(key == TestVaults.keyData(second))
        #expect(key == (try BackupService.masterKey(of: androidEnvelope, password: androidEnvelopePassword)))
    }

    /// A legacy backup's key comes from its password and salt. The vault stores
    /// it wrapped, as every new vault does, and it stays the same key.
    @Test func aLegacyBackupsKeyMovesIntoAnEnvelopeVault() async throws {
        let state = try TestVaults.makeState()

        try await state.restoreVault(fromBackup: androidLegacy, password: androidLegacyPassword)

        let metadata = try #require(try state.encryptionRepo?.get())
        #expect(metadata.envelopeVersion == 1)
        #expect(metadata.salt.isEmpty)
        #expect(metadata.iterations == 0)
        #expect(TestVaults.keyData(state) == (try BackupService.masterKey(of: androidLegacy, password: androidLegacyPassword)))
    }

    @Test(arguments: [
        (androidEnvelope, androidEnvelopePassword),
        (androidLegacy, androidLegacyPassword),
        (webEnvelope, webPassword),
    ])
    func theBackupsPasswordUnlocksTheVault(file: URL, password: String) async throws {
        let state = try TestVaults.makeState()
        try await state.restoreVault(fromBackup: file, password: password)
        let notes = state.notes.map(\.content).sorted()
        #expect(!notes.isEmpty)

        state.lock()
        #expect(state.isLocked)
        await #expect(throws: AppStateError.self) {
            try await state.unlock(password: "not-the-password")
        }
        #expect(try await state.unlock(password: password))

        #expect(state.notes.map(\.content).sorted() == notes)
        #expect(state.loadedKeyMatchesVault())
    }

    /// An existing password has to keep working, however short. The rules are
    /// for passwords being chosen, and this one was chosen long ago.
    @Test func aShortPasswordFromAnOlderVaultStillRestores() async throws {
        let file = try await makeBackup(password: "abc")
        defer { try? FileManager.default.removeItem(at: file) }
        let state = try TestVaults.makeState()

        let result = try await state.restoreVault(fromBackup: file, password: "abc")

        #expect(result.notes == 2)
        #expect(!state.isFirstLaunch)
    }

    /// Everything is queued to go up, should this device connect to an account.
    @Test func restoredNotesWaitToBeSynced() async throws {
        let state = try TestVaults.makeState()

        try await state.restoreVault(fromBackup: tuiBackup, password: tuiPassword())

        let unsynced = try #require(try state.noteRepo?.countUnsynced())
        #expect(unsynced > 0)
    }

    @Test func nothingIsLeftBehindAfterASuccess() async throws {
        let state = try TestVaults.makeState()

        try await state.restoreVault(fromBackup: tuiBackup, password: tuiPassword())

        #expect(try stagingIsEmpty(state))
    }

    // MARK: - Failures leave the first-run screen as it was

    @Test(arguments: [tuiBackup, androidEnvelope, androidLegacy])
    func theWrongPasswordMakesNoVault(file: URL) async throws {
        let state = try TestVaults.makeState()

        await #expect(throws: BackupError.wrongPassword) {
            try await state.restoreVault(fromBackup: file, password: "not-the-password")
        }

        try await expectUntouched(state)
    }

    @Test func aFileThatIsntABackupMakesNoVault() async throws {
        let file = FileManager.default.temporaryDirectory.appendingPathComponent("not-a-backup-\(UUID().uuidString).json")
        try Data(#"{"hello":"world"}"#.utf8).write(to: file)
        defer { try? FileManager.default.removeItem(at: file) }
        let state = try TestVaults.makeState()

        await #expect(throws: BackupError.invalidFormat) {
            try await state.restoreVault(fromBackup: file, password: "anything")
        }

        try await expectUntouched(state)
    }

    /// The first batch opens, so the password is right, and the damage comes
    /// after notes have been written. None of them may stay.
    @Test func damageLaterInTheFileMakesNoVault() async throws {
        let text = try String(contentsOf: androidEnvelope, encoding: .utf8)
        var object = try #require(JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any])
        var batches = try #require(object["batches"] as? [[String: Any]])
        let data = try #require(batches[1]["data"] as? String)
        var parts = data.split(separator: ".", omittingEmptySubsequences: false).map(String.init)
        parts[3] = String(parts[3].reversed())
        batches[1]["data"] = parts.joined(separator: ".")
        object["batches"] = batches
        let file = FileManager.default.temporaryDirectory.appendingPathComponent("damaged-\(UUID().uuidString).jottery-backup")
        try JSONSerialization.data(withJSONObject: object).write(to: file)
        defer { try? FileManager.default.removeItem(at: file) }
        let state = try TestVaults.makeState()

        await #expect(throws: (any Error).self) {
            try await state.restoreVault(fromBackup: file, password: androidEnvelopePassword)
        }

        try await expectUntouched(state)
    }

    @Test func aBackupWithNoNotesMakesNoVault() async throws {
        let password = "a-password-\(UUID().uuidString)"
        let file = try await makeBackup(password: password, notes: 0)
        defer { try? FileManager.default.removeItem(at: file) }
        let state = try TestVaults.makeState()

        await #expect(throws: BackupError.nothingToRestore) {
            try await state.restoreVault(fromBackup: file, password: password)
        }

        try await expectUntouched(state)
    }

    /// With no batches there's nothing to prove a legacy password against, so
    /// any password would seem to work, and leave a vault it can't be told from.
    @Test func anEmptyLegacyBackupMakesNoVaultWhateverThePassword() async throws {
        let empty = try await TestVaults.makeLegacyVault(password: "the-real-one-\(UUID().uuidString)")
        let file = try backupFile(of: empty)
        defer { try? FileManager.default.removeItem(at: file) }
        let state = try TestVaults.makeState()

        await #expect(throws: BackupError.nothingToRestore) {
            try await state.restoreVault(fromBackup: file, password: "not-the-password")
        }

        try await expectUntouched(state)
    }

    /// The password proves itself, but there are still no notes to make a vault of.
    @Test func aBackupOfOnlyASavedSearchMakesNoVault() async throws {
        let password = "a-password-\(UUID().uuidString)"
        let source = try TestVaults.makeState()
        try await source.createVault(password: password)
        try source.saveSearch(name: "Just a search", query: "#nothing")
        let file = try backupFile(of: source)
        defer { try? FileManager.default.removeItem(at: file) }
        let state = try TestVaults.makeState()

        await #expect(throws: BackupError.nothingToRestore) {
            try await state.restoreVault(fromBackup: file, password: password)
        }

        try await expectUntouched(state)
    }

    /// The last step can fail too, with most of the vault across: a full disk,
    /// here a row in the way. The transaction takes all of it back.
    @Test func aFailureAtTheLastStepTakesEverythingBack() async throws {
        let reference = try TestVaults.makeState()
        try await reference.restoreVault(fromBackup: androidEnvelope, password: androidEnvelopePassword)

        let state = try TestVaults.makeState()
        try TestVaults.execute(state, """
            INSERT INTO saved_searches (id, name, query, display_order, created_at, modified_at)
            SELECT ?, 'x', 'x', 0, 'x', 'x'
        """, [try #require(try reference.savedSearchRepo?.listAll(key: vaultKey(reference)).first?.id)])

        await #expect(throws: (any Error).self) {
            try await state.restoreVault(fromBackup: androidEnvelope, password: androidEnvelopePassword)
        }

        #expect(state.isFirstLaunch)
        let counts = try rowCounts(state)
        #expect(counts["encryption_metadata"] == 0)
        #expect(counts["notes"] == 0)
        #expect(counts["saved_searches"] == 1)
        #expect(try stagingIsEmpty(state))
    }

    @Test func aCancelledRestoreMakesNoVault() async throws {
        let state = try TestVaults.makeState()
        let password = try tuiPassword()
        let handle = RestoreHandle()

        let restore = Task {
            try await state.restoreVault(fromBackup: tuiBackup, password: password) { _ in
                // Mid-way, with notes already in the staged vault.
                handle.cancel()
            }
        }
        handle.hold(restore)

        await #expect(throws: CancellationError.self) {
            try await restore.value
        }

        try await expectUntouched(state)
    }

    @Test func aVaultThatExistsIsNeverReplaced() async throws {
        let state = try TestVaults.makeState()
        try await state.createVault(password: "the-vault-already-here")
        _ = try state.createNote(content: "Mine", tags: [])
        let key = TestVaults.keyData(state)

        await #expect(throws: VaultFromBackup.RestoreError.vaultExists) {
            try await state.restoreVault(fromBackup: tuiBackup, password: tuiPassword())
        }

        #expect(TestVaults.keyData(state) == key)
        #expect(try rowCounts(state)["notes"] == 1)
    }

    /// The app can be killed at any point. A staged restore holds nothing the
    /// app reads, and the next launch clears it away.
    @Test func aRestoreCutShortByTheAppDyingIsClearedAtLaunch() async throws {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-test-vault-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let path = dir.appendingPathComponent("test.db").path
        let database = try DatabaseManager(path: path)
        let folder = VaultFromBackup.stagingFolder(besides: database)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        try Data("left over".utf8).write(to: folder.appendingPathComponent("staged.db"))

        let state = AppState()
        try state.initialise(database: database)

        #expect(state.isFirstLaunch)
        #expect(!FileManager.default.fileExists(atPath: folder.path))
    }
}
