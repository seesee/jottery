import CryptoKit
import Foundation
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

/// Real backups, written by the other clients and kept in their own folders, so a
/// change to any client's writer shows up here (jottery-ychf). Each is restored
/// into a vault with a key of its own, which is what a real restore has to do.
@MainActor
struct CrossClientBackupTests {

    private func tuiPassword() throws -> String {
        try String(contentsOf: tuiBackup.appendingPathExtension("password"), encoding: .utf8)
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// A vault that shares nothing with the backup, apart from the password in some cases.
    private func makeVault() async throws -> AppState {
        let state = try TestVaults.makeState()
        try await state.createVault(password: "a-vault-of-its-own-\(UUID().uuidString)")
        return state
    }

    private func restore(_ url: URL, password: String, into state: AppState) throws -> BackupService.RestoreResult {
        try BackupService.restoreBackup(
            from: url,
            password: password,
            noteRepo: #require(state.noteRepo),
            versionRepo: #require(state.versionRepo),
            attachmentRepo: #require(state.attachmentRepo),
            savedSearchRepo: #require(state.savedSearchRepo),
            currentKey: #require(state.keyManager.masterKey)
        )
    }

    private func attachmentText(_ attachment: AttachmentRef, in state: AppState) throws -> String {
        let key = try #require(state.keyManager.masterKey)
        let blob = try #require(try state.attachmentRepo?.getBlob(id: attachment.data))
        let stored = try CryptoService.parseEncryptedJSON(String(decoding: blob, as: UTF8.self))
        return String(decoding: try CryptoService.decrypt(stored, key: key), as: UTF8.self)
    }

    // MARK: - The TUI

    @Test func aTUIBackupRestoresWithEverythingInIt() async throws {
        let state = try await makeVault()

        let result = try restore(tuiBackup, password: tuiPassword(), into: state)

        #expect(result.skipped == 0)
        #expect(result.notes == 2)
        try await state.loadNotes()
        try state.loadArchivedNotes()

        let note = try #require(state.notes.first { $0.content.contains("A note from the TUI") })
        #expect(note.content == "A note from the TUI, with a café and 日本語\n")
        #expect(Set(note.tags) == ["work", "cross-client"])
        let attachment = try #require(note.attachments.first)
        #expect(attachment.filename == "hello.txt")
        #expect(try attachmentText(attachment, in: state) == "attachment body")

        let archived = try #require(state.archivedNotes.first { $0.content.contains("An archived note") })
        #expect(archived.archived)
    }

    @Test func aTUIBackupRefusesTheWrongPassword() async throws {
        let state = try await makeVault()

        #expect(throws: BackupError.wrongPassword) {
            try restore(tuiBackup, password: "not-the-password", into: state)
        }
    }

    // MARK: - Android

    /// Both Android files hold the same vault: one from before a password change
    /// moved it to an envelope, one from after.
    private func checkAndroidBackup(_ url: URL, password: String) async throws {
        let state = try await makeVault()

        let result = try restore(url, password: password, into: state)

        #expect(result.skipped == 0)
        #expect(result.notes == 2)
        try await state.loadNotes()
        let key = try #require(state.keyManager.masterKey)

        let note = try #require(state.notes.first { $0.content == "From Android, edited once" })
        #expect(note.tags == ["android", "fixture"])
        #expect(note.pinned)
        let attachment = try #require(note.attachments.first)
        #expect(attachment.filename == "hello.txt")
        #expect(attachment.mimeType == "text/plain")
        #expect(try attachmentText(attachment, in: state) == "attachment bytes")

        let plain = try #require(state.notes.first { $0.content == "A note with no tags at all" })
        #expect(plain.tags.isEmpty)
        #expect(plain.attachments.isEmpty)

        let versions = try #require(try state.versionRepo?.getVersions(noteId: note.id))
        let version = try #require(versions.first)
        let content = try CryptoService.parseEncryptedJSON(version.content)
        #expect(String(decoding: try CryptoService.decrypt(content, key: key), as: UTF8.self) == "From Android")

        let searches = try #require(try state.savedSearchRepo?.listAll(key: key))
        #expect(searches.map(\.name) == ["Android search"])
        #expect(searches.map(\.query) == ["#android"])
    }

    @Test func anAndroidBackupFromALegacyVaultRestores() async throws {
        try await checkAndroidBackup(androidLegacy, password: "notes-password-correct-horse")
    }

    @Test func anAndroidBackupFromAnEnvelopeVaultRestores() async throws {
        try await checkAndroidBackup(androidEnvelope, password: "notes-password-battery-staple")
    }

    @Test(arguments: [androidLegacy, androidEnvelope])
    func anAndroidBackupRefusesTheWrongPassword(url: URL) async throws {
        let state = try await makeVault()

        #expect(throws: BackupError.wrongPassword) {
            try restore(url, password: "not-the-password", into: state)
        }
    }

    /// What a restore wrote has to survive a round trip: a backup made here from
    /// the restored vault still holds the other client's notes, whole.
    @Test func anotherClientsNotesSurviveABackupMadeHere() async throws {
        let password = "a-vault-of-its-own-\(UUID().uuidString)"
        let first = try TestVaults.makeState()
        try await first.createVault(password: password)
        _ = try restore(androidEnvelope, password: "notes-password-battery-staple", into: first)

        let file = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-cross-client-\(UUID().uuidString).jottery-backup")
        defer { try? FileManager.default.removeItem(at: file) }
        try BackupService.writeBackup(
            to: file,
            noteRepo: #require(first.noteRepo),
            attachmentRepo: #require(first.attachmentRepo),
            savedSearchRepo: #require(first.savedSearchRepo),
            encryptionRepo: #require(first.encryptionRepo),
            key: #require(first.keyManager.masterKey)
        )

        let second = try await makeVault()
        let result = try restore(file, password: password, into: second)

        #expect(result.skipped == 0)
        #expect(result.notes == 2)
        try await second.loadNotes()
        let note = try #require(second.notes.first { $0.content == "From Android, edited once" })
        #expect(note.tags == ["android", "fixture"])
        let attachment = try #require(note.attachments.first)
        #expect(attachment.filename == "hello.txt")
        #expect(try attachmentText(attachment, in: second) == "attachment bytes")
    }
}
