import CryptoKit
import Foundation
import GRDB
import Testing

@testable import Jottery

/// A filename reaches the app from the server and from imported files, and the
/// app writes attachments to disk under it, so it must never be able to name
/// anything but a file of its own.
@MainActor
struct AttachmentFilenameTests {

    private let key = SymmetricKey(data: Data(repeating: 7, count: 32))

    @Test(arguments: [
        ("../../Library/Application Support/Jottery/jottery.db", "jottery.db"),
        ("..\\..\\evil.exe", "evil.exe"),
        ("/etc/passwd", "passwd"),
        ("photo.png", "photo.png"),
        ("a/b/c.txt", "c.txt"),
        ("trailing/", "trailing"),
        ("  spaced.txt  ", "spaced.txt"),
        ("bell\u{07}.txt", "bell.txt"),
        ("nul\u{0}.txt", "nul.txt"),
        ("invoice\u{202E}gpj.exe", "invoicegpj.exe"),
        ("Résumé 履歴書.pdf", "Résumé 履歴書.pdf"),
    ])
    func unsafeNamesAreReducedToOneComponent(raw: String, expected: String) {
        #expect(AttachmentFilename.safe(raw) == expected)
    }

    @Test(arguments: ["", ".", "..", "/", "../", "../..", "   ", "\u{0}"])
    func namesWithNothingLeftGetTheFallback(raw: String) {
        #expect(AttachmentFilename.safe(raw) == AttachmentFilename.fallback)
    }

    @Test func aVeryLongNameIsShortenedAndKeepsItsExtension() {
        let safe = AttachmentFilename.safe(String(repeating: "é", count: 500) + ".pdf")
        #expect(safe.utf8.count <= 200)
        #expect(safe.hasSuffix(".pdf"))
    }

    @Test func aNameThatFailsToDecryptIsNeverShownAsCiphertext() throws {
        let foreign = try TestVaults.foreignCiphertext()
        #expect(AttachmentFilename.display(stored: foreign, key: key) == L.attachmentsUnreadableName)
    }

    @Test func aDecryptedNameIsMadeSafeToo() throws {
        let stored = try CryptoService.serializeEncryptedJSON(CryptoService.encryptText("../escape.txt", key: key))
        #expect(AttachmentFilename.display(stored: stored, key: key) == "escape.txt")
    }

    // MARK: - In a vault

    private func makeUnlockedState() async throws -> AppState {
        let state = try TestVaults.makeState()
        try await state.createVault(password: "notes-password-correct-horse")
        return state
    }

    private func storedRefs(ofNote id: String, in state: AppState) throws -> [AttachmentRef] {
        let record = try #require(try state.noteRepo?.getRaw(id: id))
        return try JSONDecoder().decode([AttachmentRef].self, from: Data(record.attachments.utf8))
    }

    /// What a hostile pull or an older build leaves behind.
    private func setStoredFilename(_ filename: String, ofNote id: String, in state: AppState) throws {
        var refs = try storedRefs(ofNote: id, in: state)
        refs[0].filename = filename
        let json = String(decoding: try JSONEncoder().encode(refs), as: UTF8.self)
        try TestVaults.execute(state, "UPDATE notes SET attachments = ? WHERE id = ?", [json, id])
    }

    @Test func aHostileStoredNameIsSafeOnceRead() async throws {
        let state = try await makeUnlockedState()
        let key = try #require(state.keyManager.masterKey)
        let note = try #require(try state.createNote(content: "note"))
        try state.addAttachment(to: note.id, data: Data("x".utf8), filename: "a.txt", mimeType: "text/plain")
        try setStoredFilename("../../jottery.db", ofNote: note.id, in: state)

        let read = try #require(try state.noteRepo?.get(id: note.id, key: key))

        #expect(read.attachments.first?.filename == "jottery.db")
    }

    @Test func addingAnAttachmentCleansItsName() async throws {
        let state = try await makeUnlockedState()
        let key = try #require(state.keyManager.masterKey)
        let note = try #require(try state.createNote(content: "note"))

        try state.addAttachment(to: note.id, data: Data("x".utf8), filename: "../../evil.txt", mimeType: "text/plain")

        #expect(try state.noteRepo?.get(id: note.id, key: key)?.attachments.first?.filename == "evil.txt")
    }

    @Test func importingCleansAttachmentNames() async throws {
        let state = try await makeUnlockedState()
        let key = try #require(state.keyManager.masterKey)
        let exported = ExportService.ExportData(version: "1.0", exportDate: Date().iso8601, notes: [
            ExportService.ExportNote(
                id: CryptoService.generateUUID(),
                createdAt: Date().iso8601,
                modifiedAt: Date().iso8601,
                content: "imported",
                tags: [],
                attachments: [ExportService.ExportAttachment(
                    filename: "../../jottery.db", mimeType: "text/plain", data: Data("x".utf8).base64EncodedString()
                )],
                pinned: false, archived: false, locked: false,
                wordWrap: nil, syntaxLanguage: nil, showPreview: nil, color: nil
            ),
        ])

        _ = try ImportService.importNotes(
            exported, strategy: .skip,
            noteRepo: #require(state.noteRepo), attachmentRepo: #require(state.attachmentRepo), key: key
        )

        let id = exported.notes[0].id
        #expect(try state.noteRepo?.get(id: id, key: key)?.attachments.first?.filename == "jottery.db")
    }

    // MARK: - Filenames stay encrypted (jottery-7uzk)

    /// A note's in-memory copy holds decrypted filenames. Saving it wrote those
    /// back as they were, and sync sent them to the server.
    @Test func editingANoteLeavesItsAttachmentNamesEncrypted() async throws {
        let state = try await makeUnlockedState()
        let key = try #require(state.keyManager.masterKey)
        let note = try #require(try state.createNote(content: "before"))
        try state.addAttachment(to: note.id, data: Data("x".utf8), filename: "passport-scan.jpg", mimeType: "image/jpeg")

        var edited = try #require(try state.noteRepo?.get(id: note.id, key: key))
        #expect(edited.attachments.first?.filename == "passport-scan.jpg")
        edited.content = "after"
        try state.saveNote(edited)

        let stored = try #require(try storedRefs(ofNote: note.id, in: state).first)
        #expect(AttachmentFilename.isEncrypted(stored.filename))
        #expect(!stored.filename.contains("passport-scan"))
        #expect(try state.noteRepo?.get(id: note.id, key: key)?.attachments.first?.filename == "passport-scan.jpg")
    }

    /// The editor's copy can be older than the note: a save mustn't bring back
    /// an attachment removed meanwhile, or drop one added.
    @Test func savingAStaleCopyKeepsTheStoredAttachments() async throws {
        let state = try await makeUnlockedState()
        let key = try #require(state.keyManager.masterKey)
        let note = try #require(try state.createNote(content: "before"))
        var stale = try #require(try state.noteRepo?.get(id: note.id, key: key))
        try state.addAttachment(to: note.id, data: Data("x".utf8), filename: "added.txt", mimeType: "text/plain")

        stale.content = "after"
        try state.saveNote(stale)

        #expect(try state.noteRepo?.get(id: note.id, key: key)?.attachments.map(\.filename) == ["added.txt"])
    }

    @Test func theHashStillCoversTheStoredAttachments() async throws {
        let state = try await makeUnlockedState()
        let key = try #require(state.keyManager.masterKey)
        let note = try #require(try state.createNote(content: "before"))
        var stale = try #require(try state.noteRepo?.get(id: note.id, key: key))
        try state.addAttachment(to: note.id, data: Data("x".utf8), filename: "a.txt", mimeType: "text/plain")

        // Same content, encrypted again: only the hash inputs are of interest.
        stale.content = "after"
        try #require(state.noteRepo).update(stale, key: key)

        let record = try #require(try state.noteRepo?.getRaw(id: note.id))
        #expect(record.contentHash != nil)
        #expect(try storedRefs(ofNote: note.id, in: state).count == 1)
    }

    @Test func theRepairPassEncryptsPlaintextNamesAndQueuesAPush() async throws {
        let state = try await makeUnlockedState()
        let key = try #require(state.keyManager.masterKey)
        let noteRepo = try #require(state.noteRepo)
        let note = try #require(try state.createNote(content: "note"))
        try state.addAttachment(to: note.id, data: Data("x".utf8), filename: "a.txt", mimeType: "text/plain")
        try setStoredFilename("passport-scan.jpg", ofNote: note.id, in: state)
        try noteRepo.markSynced(id: note.id, syncedAt: Date().iso8601)
        let modifiedAt = try #require(try noteRepo.getRaw(id: note.id)).modifiedAt

        #expect(try noteRepo.encryptPlaintextAttachmentFilenames(key: key) == 1)

        let record = try #require(try noteRepo.getRaw(id: note.id))
        #expect(record.needsSync)
        #expect(record.modifiedAt == modifiedAt)
        let stored = try #require(try storedRefs(ofNote: note.id, in: state).first)
        #expect(AttachmentFilename.isEncrypted(stored.filename))
        #expect(try noteRepo.get(id: note.id, key: key)?.attachments.first?.filename == "passport-scan.jpg")
    }

    @Test func theRepairPassCoversVersionSnapshots() async throws {
        let state = try await makeUnlockedState()
        let key = try #require(state.keyManager.masterKey)
        let noteRepo = try #require(state.noteRepo)
        let note = try #require(try state.createNote(content: "note"))
        try state.addAttachment(to: note.id, data: Data("x".utf8), filename: "a.txt", mimeType: "text/plain")
        try noteRepo.markSynced(id: note.id, syncedAt: Date().iso8601)
        try setStoredFilename("passport-scan.jpg", ofNote: note.id, in: state)
        try #require(state.versionRepo).createVersion(from: #require(try noteRepo.getRaw(id: note.id)), reason: "sync")
        var refs = try storedRefs(ofNote: note.id, in: state)
        refs[0].filename = try AttachmentFilename.encrypted("passport-scan.jpg", key: key)
        let json = String(decoding: try JSONEncoder().encode(refs), as: UTF8.self)
        try TestVaults.execute(state, "UPDATE notes SET attachments = ?, needs_sync = 0 WHERE id = ?", [json, note.id])

        #expect(try noteRepo.encryptPlaintextAttachmentFilenames(key: key) == 1)

        let version = try #require(try state.versionRepo?.getVersions(noteId: note.id).first)
        #expect(!version.attachments.contains("passport-scan"))
        // Versions go up with their note, so the note has to go too.
        #expect(try #require(try noteRepo.getRaw(id: note.id)).needsSync)
    }

    @Test func theRepairPassLeavesEncryptedNamesAlone() async throws {
        let state = try await makeUnlockedState()
        let key = try #require(state.keyManager.masterKey)
        let noteRepo = try #require(state.noteRepo)
        let note = try #require(try state.createNote(content: "note"))
        try state.addAttachment(to: note.id, data: Data("x".utf8), filename: "a.txt", mimeType: "text/plain")
        try noteRepo.markSynced(id: note.id, syncedAt: Date().iso8601)
        let before = try #require(try noteRepo.getRaw(id: note.id)).attachments

        #expect(try noteRepo.encryptPlaintextAttachmentFilenames(key: key) == 0)

        let record = try #require(try noteRepo.getRaw(id: note.id))
        #expect(record.attachments == before)
        #expect(!record.needsSync)
    }
}
