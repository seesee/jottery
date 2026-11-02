import CryptoKit
import Foundation
import GRDB
import Testing

@testable import Jottery

/// `VaultReKeyService` moves a whole vault from one master key to another.
/// It replaces a loop that skipped (and only logged) anything it could not
/// re-encrypt, and swapped the vault's key separately afterwards: a partial
/// failure left notes under the old key and unreadable after the switch. Now
/// the vault must end up entirely on the new key, or be left exactly as it was.
@MainActor
struct VaultReKeyServiceTests {

    private let password = "notes-password-correct-horse"

    private func makeVault() async throws -> AppState {
        let state = try TestVaults.makeState()
        try await state.createVault(password: password)
        return state
    }

    private struct Fixture {
        let noteId: String
        let attachmentId: String
        let versionKey: String
        let searchId: String
    }

    /// One of everything the master key protects: a note with tags and an
    /// attachment (encrypted filename and blob), a version snapshot of it,
    /// and a saved search.
    private func populate(_ state: AppState) throws -> Fixture {
        let note = try #require(try state.createNote(content: "secret content", tags: ["alpha", "beta"]))
        try state.addAttachment(
            to: note.id, data: Data("attachment bytes".utf8), filename: "photo.png", mimeType: "image/png"
        )
        let record = try #require(try state.noteRepo?.getRaw(id: note.id))
        let attachments = try JSONDecoder().decode([AttachmentRef].self, from: Data(record.attachments.utf8))
        let attachmentId = try #require(attachments.first?.data)
        try #require(state.versionRepo).createVersion(from: record, reason: "manual")
        let versionKey = try #require(try state.versionRepo?.getVersions(noteId: note.id).first?.versionKey)
        try state.saveSearch(name: "Work", query: "#work")
        let searchId = try #require(state.savedSearches.first?.id)
        return Fixture(noteId: note.id, attachmentId: attachmentId, versionKey: versionKey, searchId: searchId)
    }

    private func database(_ state: AppState) throws -> DatabaseManager {
        try #require(state.db)
    }


    private func metadata(for key: Data, in state: AppState) throws -> EncryptionMetadata {
        let current = try #require(try state.encryptionRepo?.get())
        return try EnvelopeService.envelopeMetadata(for: key, localPassword: password, current: current)
    }

    private func decrypt(_ json: String, _ key: SymmetricKey) -> Data? {
        guard let encrypted = try? CryptoService.parseEncryptedJSON(json) else { return nil }
        return try? CryptoService.decrypt(encrypted, key: key)
    }

    private func text(_ json: String, _ key: SymmetricKey) -> String? {
        decrypt(json, key).map { String(decoding: $0, as: UTF8.self) }
    }

    // MARK: - Success

    @Test func reKeyMovesEveryEncryptedValueToTheNewKey() async throws {
        let state = try await makeVault()
        let fixture = try populate(state)
        let oldKey = try #require(state.keyManager.masterKey)
        let newKeyData = CryptoService.generateMasterKey()
        let newKey = SymmetricKey(data: newKeyData)
        let newMetadata = try metadata(for: newKeyData, in: state)

        let summary = try VaultReKeyService.reKey(
            database: database(state), from: oldKey, to: newKey, newMetadata: newMetadata
        )

        #expect(summary == VaultReKeyService.Summary(notes: 1, versions: 1, attachments: 1, savedSearches: 1))

        let note = try #require(try state.noteRepo?.getRaw(id: fixture.noteId))
        #expect(text(note.content, newKey) == "secret content")
        #expect(text(note.content, oldKey) == nil)
        #expect(text(note.tags, newKey) == #"["alpha","beta"]"#)
        let refs = try JSONDecoder().decode([AttachmentRef].self, from: Data(note.attachments.utf8))
        #expect(text(try #require(refs.first).filename, newKey) == "photo.png")
        #expect(note.needsSync)

        let blob = try #require(try state.attachmentRepo?.getBlob(id: fixture.attachmentId))
        #expect(decrypt(String(decoding: blob, as: UTF8.self), newKey) == Data("attachment bytes".utf8))
        #expect(decrypt(String(decoding: blob, as: UTF8.self), oldKey) == nil)

        let version = try #require(try state.versionRepo?.getVersions(noteId: fixture.noteId).first)
        #expect(text(version.content, newKey) == "secret content")
        #expect(text(version.tags, newKey) == #"["alpha","beta"]"#)
        // The old loop never re-encrypted a version's attachment filenames.
        let versionRefs = try JSONDecoder().decode([AttachmentRef].self, from: Data(version.attachments.utf8))
        #expect(text(try #require(versionRefs.first).filename, newKey) == "photo.png")

        let searches = try #require(try state.savedSearchRepo?.listAll(key: newKey))
        #expect(searches.map(\.name) == ["Work"])
        #expect(searches.map(\.query) == ["#work"])
        let searchRaw = try #require(try state.savedSearchRepo?.getRaw(id: fixture.searchId))
        #expect(searchRaw.needsSync)

        let stored = try #require(try state.encryptionRepo?.get())
        #expect(stored.localWrappedMaster == newMetadata.localWrappedMaster)
        #expect(stored.deviceSalt == newMetadata.deviceSalt)
        #expect(stored.verification.flatMap { text($0, newKey) } == EncryptionMetadata.verificationPlaintext)
    }

    @Test func reKeyLeavesPlaintextFilenamesAndUnencryptedBlobsAlone() async throws {
        let state = try await makeVault()
        let fixture = try populate(state)
        let oldKey = try #require(state.keyManager.masterKey)
        let newKeyData = CryptoService.generateMasterKey()

        // Very old attachments may have a plaintext filename, and a backup
        // restore can store a blob it could not decrypt as-is. Neither is under
        // any key, so neither should block the re-key or be altered by it.
        var refs = try JSONDecoder().decode(
            [AttachmentRef].self,
            from: Data(try #require(try state.noteRepo?.getRaw(id: fixture.noteId)).attachments.utf8)
        )
        refs[0].filename = "plain-name.png"
        let plainRefs = String(decoding: try JSONEncoder().encode(refs), as: UTF8.self)
        try TestVaults.execute(state, "UPDATE notes SET attachments = ? WHERE id = ?", [plainRefs, fixture.noteId])
        try TestVaults.execute(state, "UPDATE attachments SET data = ? WHERE id = ?", [Data("raw bytes".utf8), fixture.attachmentId])

        let summary = try VaultReKeyService.reKey(
            database: database(state), from: oldKey, to: SymmetricKey(data: newKeyData),
            newMetadata: metadata(for: newKeyData, in: state)
        )

        #expect(summary.notes == 1)
        let note = try #require(try state.noteRepo?.getRaw(id: fixture.noteId))
        let after = try JSONDecoder().decode([AttachmentRef].self, from: Data(note.attachments.utf8))
        #expect(after.first?.filename == "plain-name.png")
        #expect(try state.attachmentRepo?.getBlob(id: fixture.attachmentId) == Data("raw bytes".utf8))
    }

    /// A vault that joined an account and synced before moving onto the
    /// account's key holds rows pulled under that key. Those must be left as
    /// they are, not treated as unreadable, or the move could never finish.
    @Test func reKeyLeavesValuesAlreadyOnTheNewKeyAlone() async throws {
        let state = try await makeVault()
        let fixture = try populate(state)
        let oldKey = try #require(state.keyManager.masterKey)
        let newKeyData = CryptoService.generateMasterKey()
        let newKey = SymmetricKey(data: newKeyData)

        let pulled = try #require(try state.createNote(content: "placeholder", tags: []))
        let pulledContent = try CryptoService.serializeEncryptedJSON(CryptoService.encryptText("from the account", key: newKey))
        let pulledTags = try CryptoService.serializeEncryptedJSON(CryptoService.encryptStringArray(["synced"], key: newKey))
        try TestVaults.execute(
            state, "UPDATE notes SET content = ?, tags = ? WHERE id = ?", [pulledContent, pulledTags, pulled.id]
        )

        let summary = try VaultReKeyService.reKey(
            database: database(state), from: oldKey, to: newKey,
            newMetadata: metadata(for: newKeyData, in: state)
        )

        #expect(summary.notes == 1)
        #expect(summary.alreadyOnNewKey == 1)
        let untouched = try #require(try state.noteRepo?.getRaw(id: pulled.id))
        #expect(untouched.content == pulledContent)
        #expect(text(untouched.content, newKey) == "from the account")
        let moved = try #require(try state.noteRepo?.getRaw(id: fixture.noteId))
        #expect(text(moved.content, newKey) == "secret content")
    }

    // MARK: - Face ID key check

    @Test func theLoadedKeyIsCheckedAgainstTheVault() async throws {
        let state = try await makeVault()
        #expect(state.loadedKeyMatchesVault())

        // What a stale Face ID copy of the key would load.
        state.keyManager.masterKey = SymmetricKey(size: .bits256)
        #expect(!state.loadedKeyMatchesVault())
    }

    // MARK: - Failure changes nothing

    /// A value encrypted under some other key cannot be moved. The whole re-key
    /// must then roll back, wherever that value is.
    @Test(arguments: ["notes", "note_versions", "attachments", "saved_searches"])
    func reKeyChangesNothingWhenAnyValueCannotBeDecrypted(table: String) async throws {
        let state = try await makeVault()
        let fixture = try populate(state)
        let oldKey = try #require(state.keyManager.masterKey)
        let foreign = try TestVaults.foreignCiphertext()
        let (sql, arguments): (String, StatementArguments) = switch table {
        case "notes": ("UPDATE notes SET content = ? WHERE id = ?", [foreign, fixture.noteId])
        case "note_versions": ("UPDATE note_versions SET tags = ? WHERE version_key = ?", [foreign, fixture.versionKey])
        case "attachments": ("UPDATE attachments SET data = ? WHERE id = ?", [Data(foreign.utf8), fixture.attachmentId])
        default: ("UPDATE saved_searches SET query = ? WHERE id = ?", [foreign, fixture.searchId])
        }
        try TestVaults.execute(state, sql, arguments)
        let newKeyData = CryptoService.generateMasterKey()
        let newMetadata = try metadata(for: newKeyData, in: state)
        let db = try database(state)
        let before = try TestVaults.snapshot(state)

        #expect(throws: VaultReKeyService.ReKeyError.self) {
            try VaultReKeyService.reKey(
                database: db, from: oldKey, to: SymmetricKey(data: newKeyData), newMetadata: newMetadata
            )
        }

        let after = try TestVaults.snapshot(state)
        for (name, rows) in before {
            #expect(after[name] == rows, "\(name) changed although the re-key failed")
        }
    }

    // MARK: - AppState.switchVaultToMasterKey

    @Test func switchingTheVaultKeyKeepsEveryNoteReadable() async throws {
        let state = try await makeVault()
        _ = try populate(state)
        let newKeyData = CryptoService.generateMasterKey()

        let summary = try await state.switchVaultToMasterKey(newKeyData, notesPassword: password)

        #expect(summary.notes == 1)
        #expect(TestVaults.keyData(state) == newKeyData)
        #expect(state.notes.map(\.content) == ["secret content"])
        #expect(await state.verifyVaultPassword(password))

        // The notes password now unwraps the new key.
        state.lock()
        try await state.unlock(password: password)
        #expect(TestVaults.keyData(state) == newKeyData)
        #expect(state.notes.map(\.content) == ["secret content"])
    }

    @Test func aFailedKeySwitchKeepsTheOldKey() async throws {
        let state = try await makeVault()
        let fixture = try populate(state)
        let oldKeyData = try #require(TestVaults.keyData(state))
        try TestVaults.execute(
            state, "UPDATE saved_searches SET query = ? WHERE id = ?",
            [try TestVaults.foreignCiphertext(), fixture.searchId]
        )
        let before = try TestVaults.snapshot(state)

        await #expect(throws: VaultReKeyService.ReKeyError.self) {
            try await state.switchVaultToMasterKey(CryptoService.generateMasterKey(), notesPassword: password)
        }

        #expect(TestVaults.keyData(state) == oldKeyData)
        #expect(try TestVaults.snapshot(state) == before)
        state.lock()
        try await state.unlock(password: password)
        #expect(TestVaults.keyData(state) == oldKeyData)
        #expect(state.notes.map(\.content) == ["secret content"])
    }

    // MARK: - Legacy password change

    /// A legacy vault's key is derived from its password, so changing the
    /// password re-encrypts everything. That used the same skip-and-log loop.

    @Test func legacyPasswordChangeReEncryptsEverything() async throws {
        let state = try await TestVaults.makeLegacyVault(password: password)
        let fixture = try populate(state)
        let newPassword = "a-new-notes-password"

        try await state.changePassword(currentPassword: password, newPassword: newPassword)

        #expect(await state.verifyVaultPassword(newPassword))
        #expect(!(await state.verifyVaultPassword(password)))
        #expect(state.notes.map(\.content) == ["secret content"])
        let newKey = try #require(state.keyManager.masterKey)
        let version = try #require(try state.versionRepo?.getVersions(noteId: fixture.noteId).first)
        #expect(text(version.content, newKey) == "secret content")

        state.lock()
        try await state.unlock(password: newPassword)
        #expect(state.notes.map(\.content) == ["secret content"])
    }

    @Test func aLegacyPasswordChangeThatCannotReEncryptChangesNothing() async throws {
        let state = try await TestVaults.makeLegacyVault(password: password)
        let fixture = try populate(state)
        try TestVaults.execute(
            state, "UPDATE note_versions SET content = ? WHERE version_key = ?",
            [try TestVaults.foreignCiphertext(), fixture.versionKey]
        )
        let before = try TestVaults.snapshot(state)

        await #expect(throws: VaultReKeyService.ReKeyError.self) {
            try await state.changePassword(currentPassword: password, newPassword: "a-new-notes-password")
        }

        #expect(try TestVaults.snapshot(state) == before)
        #expect(await state.verifyVaultPassword(password))
        state.lock()
        try await state.unlock(password: password)
        #expect(state.notes.map(\.content) == ["secret content"])
    }
}
