import CryptoKit
import Foundation
import Testing

@testable import Jottery

/// Backups have to work on a device with nothing else: the file itself must
/// lead from the password to the key. Web, iOS and the TUI share this header.
@MainActor
struct BackupServiceTests {

    private let password = "notes-password-correct-horse"

    private func makeVault(password: String) async throws -> AppState {
        let state = try TestVaults.makeState()
        try await state.createVault(password: password)
        return state
    }

    private func temporaryFile() -> URL {
        FileManager.default.temporaryDirectory.appendingPathComponent("jottery-backup-test-\(UUID().uuidString).jottery-backup")
    }

    private func backup(of state: AppState) throws -> Data {
        let url = temporaryFile()
        defer { try? FileManager.default.removeItem(at: url) }
        try BackupService.writeBackup(
            to: url,
            noteRepo: #require(state.noteRepo),
            attachmentRepo: #require(state.attachmentRepo),
            savedSearchRepo: #require(state.savedSearchRepo),
            encryptionRepo: #require(state.encryptionRepo),
            key: #require(state.keyManager.masterKey)
        )
        return try Data(contentsOf: url)
    }

    @discardableResult
    private func restore(_ data: Data, password: String, into state: AppState) throws -> Int {
        try restoreResult(data, password: password, into: state).notes
    }

    private func restoreResult(_ data: Data, password: String, into state: AppState) throws -> BackupService.RestoreResult {
        let url = temporaryFile()
        try data.write(to: url)
        defer { try? FileManager.default.removeItem(at: url) }
        return try BackupService.restoreBackup(
            from: url,
            password: password,
            noteRepo: #require(state.noteRepo),
            versionRepo: #require(state.versionRepo),
            attachmentRepo: #require(state.attachmentRepo),
            savedSearchRepo: #require(state.savedSearchRepo),
            currentKey: #require(state.keyManager.masterKey)
        )
    }

    private func content(ofNote id: String, in state: AppState) throws -> String? {
        try #require(state.noteRepo).get(id: id, key: #require(state.keyManager.masterKey))?.content
    }

    /// A backup as older versions wrote it: nothing leading to the key.
    private func withOldHeader(_ data: Data) throws -> Data {
        var json = try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
        json["encryption"] = ["salt": "", "iterations": 0, "algorithm": "PBKDF2"]
        return try JSONSerialization.data(withJSONObject: json)
    }

    @Test func aBackupRestoresIntoADifferentVault() async throws {
        let source = try await makeVault(password: password)
        let note = try #require(try source.createNote(content: "kept safe in a backup"))
        let data = try backup(of: source)

        let other = try await makeVault(password: "a-different-vault-password")
        #expect(try restore(data, password: password, into: other) == 1)
        #expect(try content(ofNote: note.id, in: other) == "kept safe in a backup")
    }

    @Test func attachmentsSurviveTheRoundTrip() async throws {
        let source = try await makeVault(password: password)
        let note = try #require(try source.createNote(content: "with a file"))
        try source.addAttachment(to: note.id, data: Data("attachment bytes".utf8), filename: "file.txt", mimeType: "text/plain")
        let sourceKey = try #require(source.keyManager.masterKey)
        let attachmentId = try #require(try source.noteRepo?.get(id: note.id, key: sourceKey)?.attachments.first?.id)
        let data = try backup(of: source)

        let other = try await makeVault(password: "a-different-vault-password")
        try restore(data, password: password, into: other)

        let otherKey = try #require(other.keyManager.masterKey)
        let stored = try #require(try other.attachmentRepo?.getBlob(id: attachmentId))
        let encrypted = try CryptoService.parseEncryptedJSON(String(decoding: stored, as: UTF8.self))
        #expect(try CryptoService.decrypt(encrypted, key: otherKey) == Data("attachment bytes".utf8))
    }

    @Test func theWrongPasswordIsRefused() async throws {
        let source = try await makeVault(password: password)
        _ = try source.createNote(content: "secret")
        let data = try backup(of: source)

        let other = try await makeVault(password: "a-different-vault-password")
        #expect(throws: BackupError.wrongPassword) {
            try restore(data, password: "not-the-password", into: other)
        }
    }

    @Test func theHeaderCarriesTheWrappedKeyAsTheWebClientExpects() async throws {
        let source = try await makeVault(password: password)
        _ = try source.createNote(content: "hello")

        let json = try #require(try JSONSerialization.jsonObject(with: backup(of: source)) as? [String: Any])
        let encryption = try #require(json["encryption"] as? [String: Any])
        let metadata = try #require(try source.encryptionRepo?.get())

        #expect(encryption["envelopeVersion"] as? Int == metadata.envelopeVersion)
        #expect(encryption["deviceSalt"] as? String == metadata.deviceSalt)
        #expect(encryption["localWrappedMaster"] as? String == metadata.localWrappedMaster)
        #expect(encryption["algorithm"] as? String == "AES-256-GCM")
        #expect(encryption["salt"] == nil)
    }

    @Test func aLegacyVaultsBackupStillRoundTrips() async throws {
        let source = try await TestVaults.makeLegacyVault(password: password)
        let note = try #require(try source.createNote(content: "from a legacy vault"))
        let data = try backup(of: source)

        let other = try await makeVault(password: "a-different-vault-password")
        try restore(data, password: password, into: other)
        #expect(try content(ofNote: note.id, in: other) == "from a legacy vault")
    }

    @Test func anOlderBackupOpensOnTheVaultThatMadeIt() async throws {
        let source = try await makeVault(password: password)
        let note = try #require(try source.createNote(content: "original"))
        let data = try withOldHeader(backup(of: source))

        try #require(source.noteRepo).permanentlyDelete(id: note.id)

        #expect(try restore(data, password: password, into: source) == 1)
        #expect(try content(ofNote: note.id, in: source) == "original")
    }

    @Test func anOlderBackupOnAnotherVaultSaysWhyItCantOpen() async throws {
        let source = try await makeVault(password: password)
        _ = try source.createNote(content: "hello")
        let data = try withOldHeader(backup(of: source))

        let other = try await makeVault(password: "a-different-vault-password")
        #expect(throws: BackupError.keyNotInBackup) {
            try restore(data, password: password, into: other)
        }
    }
}
