import Compression
import CryptoKit
import Foundation
import GRDB
import Testing

@testable import Jottery

/// A backup from one client has to restore on the other (jottery-ychf), and
/// iOS backups made before the two agreed on a format still have to open.
///
/// The web's own reader isn't available here, so what iOS writes is checked by
/// doing what the web client does, using nothing from `BackupService`. The two
/// were also tried against each other with real files when this was written.
@MainActor
struct BackupFormatTests {

    private let password = "notes-password-correct-horse"

    private func temporaryFile(_ contents: Data? = nil) throws -> URL {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-format-test-\(UUID().uuidString).jottery-backup")
        if let contents { try contents.write(to: url) }
        return url
    }

    private func makeVault() async throws -> AppState {
        let state = try TestVaults.makeState()
        try await state.createVault(password: password)
        return state
    }

    private func restore(_ url: URL, password: String? = nil, into state: AppState) throws -> BackupService.RestoreResult {
        try BackupService.restoreBackup(
            from: url,
            password: password ?? self.password,
            noteRepo: #require(state.noteRepo),
            versionRepo: #require(state.versionRepo),
            attachmentRepo: #require(state.attachmentRepo),
            savedSearchRepo: #require(state.savedSearchRepo),
            currentKey: #require(state.keyManager.masterKey)
        )
    }

    private func backup(of state: AppState) throws -> URL {
        let url = try temporaryFile()
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

    private func decryptBlob(_ data: Data, key: SymmetricKey) throws -> Data {
        try CryptoService.decrypt(CryptoService.parseEncryptedJSON(String(decoding: data, as: UTF8.self)), key: key)
    }

    // MARK: - From the TUI

    /// A real file, made by the TUI's own `jottery backup` from a vault with
    /// two notes: tags and an attachment on one, neither on the other. The TUI
    /// holds tags as one blob in an array, and attachments as an array of refs.
    private var tuiBackup: URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .appendingPathComponent("Fixtures/tui-v3.jottery-backup")
    }

    @Test func aTUIBackupRestoresWithEverythingInIt() async throws {
        let state = try await makeVault()

        let result = try restore(tuiBackup, password: "correct-horse-notes-pw", into: state)

        #expect(result == BackupService.RestoreResult(notes: 2, skipped: 0))
        try await state.loadNotes()
        let key = try #require(state.keyManager.masterKey)

        let tagged = try #require(state.notes.first { $0.content == "From the TUI with tags" })
        #expect(tagged.tags == ["tui", "two words"])
        #expect(tagged.pinned)
        let attachment = try #require(tagged.attachments.first)
        #expect(attachment.filename == "hello.txt")
        #expect(attachment.mimeType == "text/plain")
        let blob = try #require(try state.attachmentRepo?.getBlob(id: attachment.data))
        #expect(String(decoding: try decryptBlob(blob, key: key), as: UTF8.self) == "hello from the tui")

        let plain = try #require(state.notes.first { $0.content == "No tags here" })
        #expect(plain.tags.isEmpty)
        #expect(plain.attachments.isEmpty)
    }

    @Test func aTUIBackupRefusesTheWrongPassword() async throws {
        let state = try await makeVault()

        #expect(throws: BackupError.wrongPassword) {
            try restore(tuiBackup, password: "not-the-password", into: state)
        }
    }

    // MARK: - Restoring over what's here (jottery-tx3i)

    @Test func aRestoreKeepsANoteEditedSinceTheBackup() async throws {
        let state = try await makeVault()
        let note = try #require(try state.createNote(content: "in the backup"))
        let url = try backup(of: state)
        var edited = note
        edited.content = "edited since"
        try state.saveNote(edited)

        let result = try restore(url, into: state)

        #expect(result.notes == 0)
        #expect(result.keptNewer == 1)
        let key = try #require(state.keyManager.masterKey)
        #expect(try state.noteRepo?.get(id: note.id, key: key)?.content == "edited since")
    }

    @Test func aRestoreBringsBackADeletedNoteAndTakesANewerCopy() async throws {
        let state = try await makeVault()
        let gone = try #require(try state.createNote(content: "deleted since"))
        let stale = try #require(try state.createNote(content: "newer in the backup"))
        let url = try backup(of: state)
        try #require(state.noteRepo).permanentlyDelete(id: gone.id)
        try TestVaults.execute(state, "UPDATE notes SET modified_at = ? WHERE id = ?", ["2020-01-01T00:00:00.000Z", stale.id])

        let result = try restore(url, into: state)

        #expect(result.notes == 2)
        #expect(result.keptNewer == 0)
    }

    /// A vault whose copy of a note differs from the backup's, with these times.
    private func twoCopies(backupTime: String, hereTime: String) async throws -> (state: AppState, id: String, url: URL) {
        let state = try await makeVault()
        let note = try #require(try state.createNote(content: "in the backup"))
        try TestVaults.execute(state, "UPDATE notes SET modified_at = ? WHERE id = ?", [backupTime, note.id])
        let url = try backup(of: state)
        var edited = note
        edited.content = "here"
        try state.saveNote(edited)
        try TestVaults.execute(state, "UPDATE notes SET modified_at = ? WHERE id = ?", [hereTime, note.id])
        return (state, note.id, url)
    }

    private func content(of id: String, in state: AppState) throws -> String? {
        try state.noteRepo?.get(id: id, key: #require(state.keyManager.masterKey))?.content
    }

    @Test func aNewerCopyInTheBackupReplacesWhatsHere() async throws {
        let (state, id, url) = try await twoCopies(
            backupTime: "2026-03-01T10:00:00.000Z", hereTime: "2026-03-01T09:00:00.000Z"
        )

        let result = try restore(url, into: state)

        #expect(result.notes == 1)
        #expect(result.keptNewer == 0)
        #expect(try content(of: id, in: state) == "in the backup")
    }

    /// Clients don't all write times alike. As text, 10:00+01:00 sorts after
    /// 09:30Z, but it's half an hour earlier.
    @Test func timesAreComparedAsTimesAndNotAsText() async throws {
        let (state, id, url) = try await twoCopies(
            backupTime: "2026-03-01T09:30:00.000Z", hereTime: "2026-03-01T10:00:00.000+01:00"
        )

        let result = try restore(url, into: state)

        #expect(result.notes == 1)
        #expect(try content(of: id, in: state) == "in the backup")
    }

    /// Nothing says the backup's copy is the better one, and this one is in use.
    @Test func aNoteExactlyAsNewAsTheBackupsStaysAsItIs() async throws {
        let time = "2026-03-01T10:00:00.000Z"
        let (state, id, url) = try await twoCopies(backupTime: time, hereTime: time)

        let result = try restore(url, into: state)

        #expect(result.notes == 0)
        #expect(result.keptNewer == 1)
        #expect(try content(of: id, in: state) == "here")
    }

    /// Ids are never reused for different contents. Writing it again would change
    /// nothing, except to send the server a blob it already has.
    @Test func aBlobAlreadyHereIsLeftAsItIs() async throws {
        let state = try await makeVault()
        let note = try #require(try state.createNote(content: "with a file"))
        try state.addAttachment(to: note.id, data: Data("the file".utf8), filename: "a.txt", mimeType: "text/plain")
        let repo = try #require(state.attachmentRepo)
        let key = try #require(state.keyManager.masterKey)
        let saved = try #require(try state.noteRepo?.get(id: note.id, key: key))
        let blobId = try #require(saved.attachments.first).data
        try repo.markUploaded(ids: [blobId])
        let before = try #require(try repo.getBlob(id: blobId))
        let url = try backup(of: state)

        _ = try restore(url, into: state)

        #expect(try repo.getBlob(id: blobId) == before)
        #expect(try repo.getBlobToUpload(id: blobId) == nil)
    }

    /// One bad write used to leave an unknown part of the batch behind.
    /// A note restored into the bin has to leave it like any other. With no time of
    /// its own here the purge can't see it, and it would stay there for ever.
    @Test func aNoteRestoredIntoTheBinStartsItsThirtyDaysNow() async throws {
        let state = try await makeVault()
        let repo = try #require(state.noteRepo)
        let note = try #require(try state.createNote(content: "binned before the backup"))
        try repo.softDelete(id: note.id)
        try TestVaults.execute(
            state, "UPDATE notes SET local_deleted_at = NULL, deleted_at = '2000-01-01T00:00:00Z' WHERE id = ?", [note.id]
        )
        let file = try backup(of: state)
        try repo.permanentlyDelete(id: note.id)

        _ = try restore(file, into: state)
        try TestVaults.execute(state, "UPDATE notes SET needs_sync = 0 WHERE id = ?", [note.id])

        #expect(try repo.getRaw(id: note.id)?.localDeletedAt != nil, "or the purge never sees it")
        #expect(try repo.purgeOldDeletedNotes() == 0, "the backup's date isn't this device's")
        #expect(try repo.purgeOldDeletedNotes(olderThan: -3600) == 1)
    }

    @Test func aBatchThatCantBeWrittenLeavesNothingBehind() async throws {
        let source = try await makeVault()
        for index in 0..<3 { _ = try source.createNote(content: "note \(index)") }
        let url = try backup(of: source)

        let state = try await makeVault()
        let db = try #require(state.db)
        try await db.dbPool.write { database in
            try database.execute(sql: """
                CREATE TRIGGER fail_the_last BEFORE INSERT ON notes
                WHEN (SELECT COUNT(*) FROM notes) >= 2
                BEGIN SELECT RAISE(ABORT, 'disk full'); END
            """)
        }

        #expect(throws: (any Error).self) {
            try restore(url, password: password, into: state)
        }

        #expect(try state.noteRepo?.countActive() == 0)
    }

    // MARK: - From the web

    /// A real file, written by `createBatchedBackup` and `downloadBackup` in the
    /// web client: see `webBackupFixture`.
    @Test func aWebBackupRestoresWithEverythingInIt() async throws {
        let state = try await makeVault()
        let url = try temporaryFile(Data(webBackupFixture.utf8))

        let result = try restore(url, into: state)

        #expect(result == BackupService.RestoreResult(notes: 3, skipped: 0))
        try await state.loadNotes()
        let key = try #require(state.keyManager.masterKey)

        let withFile = try #require(state.notes.first { $0.content.hasPrefix("# From the web") })
        #expect(withFile.content == "# From the web\nwith a file")
        #expect(withFile.tags.sorted() == ["two tags", "web"])
        #expect(withFile.pinned)
        #expect(withFile.color == "blue")
        #expect(state.notes.first { $0.content == "no tags at all" }?.tags == [])
        #expect(state.notes.first { $0.content == "one tag" }?.tags == ["single"])

        let attachment = try #require(withFile.attachments.first)
        #expect(attachment.filename == "from-web.txt")
        let blob = try #require(try state.attachmentRepo?.getBlob(id: attachment.data))
        #expect(try decryptBlob(blob, key: key) == Data("attachment bytes".utf8))

        let versions = try #require(try state.versionRepo?.getVersions(noteId: withFile.id))
        #expect(versions.count == 1)
        let versionContent = try CryptoService.parseEncryptedJSON(versions[0].content)
        #expect(try CryptoService.decryptText(versionContent, key: key) == withFile.content)

        #expect(state.savedSearches.map(\.name) == ["Web search"])
        #expect(state.savedSearches.map(\.query) == ["#web"])
    }

    @Test func aWebBackupRefusesTheWrongPassword() async throws {
        let state = try await makeVault()
        let url = try temporaryFile(Data(webBackupFixture.utf8))

        #expect(throws: BackupError.wrongPassword) {
            _ = try restore(url, password: "not-the-password", into: state)
        }
    }

    // MARK: - For the web

    private func makeFullVault() async throws -> AppState {
        let state = try await makeVault()
        let note = try #require(try state.createNote(content: "From iOS\nwith a file", tags: ["ios", "two tags"]))
        try state.addAttachment(to: note.id, data: Data("attachment bytes from iOS".utf8), filename: "from-ios.txt", mimeType: "text/plain")
        _ = try state.createNote(content: "no tags at all", tags: [])
        let key = try #require(state.keyManager.masterKey)
        var edited = try #require(try state.noteRepo?.get(id: note.id, key: key))
        edited.content = "From iOS\nwith a file"
        try state.saveNote(edited)  // the first edit snapshots a version
        try state.saveSearch(name: "iOS search", query: "#ios")
        return state
    }

    /// The JSON array inside a batch, opened the way `jose` and `pako` do.
    private func items(in batch: [String: Any], key: SymmetricKey) throws -> [[String: Any]] {
        let jwe = try #require(batch["data"] as? String)
        let parts = jwe.split(separator: ".", omittingEmptySubsequences: false)
        #expect(parts.count == 5)
        #expect(parts[1].isEmpty)
        func decode(_ part: Substring) throws -> Data {
            var text = part.replacingOccurrences(of: "-", with: "+").replacingOccurrences(of: "_", with: "/")
            text += String(repeating: "=", count: (4 - text.count % 4) % 4)
            return try #require(Data(base64Encoded: text))
        }
        let header = try JSONSerialization.jsonObject(with: decode(parts[0])) as? [String: String]
        #expect(header == ["alg": "dir", "enc": "A256GCM"])

        // RFC 7516: the encoded protected header is the authenticated data.
        let box = try AES.GCM.SealedBox(
            nonce: AES.GCM.Nonce(data: decode(parts[2])), ciphertext: decode(parts[3]), tag: decode(parts[4])
        )
        let compressed = try AES.GCM.open(box, using: key, authenticating: Data(parts[0].utf8))

        // RFC 1950: pako refuses anything without the zlib wrapper.
        #expect(compressed.first == 0x78)
        let deflated = Data(compressed.dropFirst(2).dropLast(4))
        var output = Data(count: 1 << 20)
        let size = deflated.withUnsafeBytes { source in
            output.withUnsafeMutableBytes { destination in
                compression_decode_buffer(
                    destination.bindMemory(to: UInt8.self).baseAddress!, 1 << 20,
                    source.bindMemory(to: UInt8.self).baseAddress!, deflated.count, nil, COMPRESSION_ZLIB
                )
            }
        }
        #expect(size > 0)
        return try #require(try JSONSerialization.jsonObject(with: output.prefix(size)) as? [[String: Any]])
    }

    @Test func theFileIsLaidOutAsTheWebsStreamingReaderExpects() async throws {
        let state = try await makeFullVault()
        let text = try String(contentsOf: backup(of: state), encoding: .utf8)

        // It takes the first "version" and "type" it finds, and a batch's type
        // is one too, so the header has to come first.
        #expect(text.hasPrefix("{\n\"version\":\"3.0\",\n\"type\":\"jottery-encrypted-backup\",\n\"createdAt\":"))
        let lines = text.components(separatedBy: "\n")
        #expect(lines[4].hasPrefix("\"encryption\":{"))
        #expect(lines[5] == "\"batches\":[")
        #expect(lines[lines.count - 2].hasPrefix("\"summary\":{"))
        #expect(lines.last == "}")
        // It reads the summary from the last kilobyte.
        #expect(text.suffix(1024).contains("\"summary\""))
    }

    @Test func itemsHaveTheWebsShape() async throws {
        let state = try await makeFullVault()
        let key = try #require(state.keyManager.masterKey)
        let json = try #require(try JSONSerialization.jsonObject(with: Data(contentsOf: backup(of: state))) as? [String: Any])
        let batches = try #require(json["batches"] as? [[String: Any]])
        func first(_ type: String) throws -> [[String: Any]] {
            let batch = try #require(batches.first { $0["type"] as? String == type })
            return try items(in: batch, key: key)
        }

        let note = try #require(try first("notes").first { ($0["attachments"] as? [Any])?.isEmpty == false })
        let tags = try #require(note["tags"] as? [String])
        #expect(tags.count == 1)  // one blob holding every tag
        let decryptedTags = try CryptoService.decryptStringArray(CryptoService.parseEncryptedJSON(tags[0]), key: key)
        #expect(decryptedTags.sorted() == ["ios", "two tags"])
        #expect(note["hashChain"] is [String])
        #expect(note["pinned"] as? Bool == false)
        #expect(note["id"] is String)

        let ref = try #require((note["attachments"] as? [[String: Any]])?.first)
        let filename = try #require(ref["filename"] as? String)
        #expect(AttachmentFilename.isEncrypted(filename))

        let attachment = try #require(try first("attachments").first)
        let encodedBlob = try #require(attachment["blob"] as? String)
        let blob = try #require(Data(base64Encoded: encodedBlob))
        #expect(try decryptBlob(blob, key: key) == Data("attachment bytes from iOS".utf8))

        let version = try #require(try first("versions").first)
        #expect(version["tags"] is [String])
        #expect(version["attachments"] is [Any])

        let search = try #require(try first("saved_searches").first)
        #expect(search["order"] is Int)
        let encryptedName = try #require(search["name"] as? String)
        let name = try CryptoService.parseEncryptedJSON(encryptedName)
        #expect(try CryptoService.decryptText(name, key: key) == "iOS search")
    }

    @Test func theSummaryCountsWhatsInTheFile() async throws {
        let state = try await makeFullVault()
        let json = try #require(try JSONSerialization.jsonObject(with: Data(contentsOf: backup(of: state))) as? [String: Any])
        let summary = try #require(json["summary"] as? [String: Int])
        let batches = try #require(json["batches"] as? [[String: Any]])
        func count(_ type: String) -> Int {
            batches.filter { $0["type"] as? String == type }.compactMap { $0["count"] as? Int }.reduce(0, +)
        }

        #expect(summary["notes"] == 2)
        #expect(summary["notes"] == count("notes"))
        #expect(summary["attachments"] == count("attachments"))
        #expect(summary["versions"] == count("versions"))
        #expect(summary["savedSearches"] == 1)
        #expect(summary["savedSearches"] == count("saved_searches"))
    }

    @Test func theFilenameIsOneTheWebWillOpen() {
        #expect(BackupService.defaultFilename().hasSuffix(".jottery-backup"))
    }

    @Test func savedSearchesAndVersionsSurviveARoundTrip() async throws {
        let source = try await makeFullVault()
        let url = try backup(of: source)
        let note = try #require(source.notes.first { !$0.attachments.isEmpty })

        let other = try TestVaults.makeState()
        try await other.createVault(password: "a-different-vault-password")
        let result = try restore(url, into: other)
        try await other.loadNotes()

        #expect(result == BackupService.RestoreResult(notes: 2, skipped: 0))
        #expect(other.savedSearches.map(\.name) == ["iOS search"])
        let key = try #require(other.keyManager.masterKey)
        let version = try #require(try other.versionRepo?.getVersions(noteId: note.id).first)
        let refs = try JSONDecoder().decode([AttachmentRef].self, from: Data(version.attachments.utf8))
        for ref in refs {
            #expect(AttachmentFilename.display(stored: ref.filename, key: key) == "from-ios.txt")
        }
    }

    // MARK: - Streaming

    /// Blobs close a batch by size, so a few huge ones never meet in memory.
    @Test func largeAttachmentsAreSplitAcrossBatches() async throws {
        let state = try await makeVault()
        let note = try #require(try state.createNote(content: "big"))
        for index in 0..<3 {
            try state.addAttachment(to: note.id, data: Data(repeating: 7, count: 2048), filename: "\(index).bin", mimeType: "application/octet-stream")
        }
        let url = try temporaryFile()

        try BackupService.writeBackup(
            to: url,
            noteRepo: #require(state.noteRepo),
            attachmentRepo: #require(state.attachmentRepo),
            savedSearchRepo: #require(state.savedSearchRepo),
            encryptionRepo: #require(state.encryptionRepo),
            key: #require(state.keyManager.masterKey),
            maxAttachmentBatchBytes: 1024
        )

        let index = try JSONFileScanner.scan(url, array: "batches")
        let handle = try FileHandle(forReadingFrom: url)
        defer { try? handle.close() }
        let batches = try index.elements.map {
            try JSONDecoder().decode(BackupService.BatchRecord.self, from: JSONFileScanner.read($0, from: handle))
        }
        #expect(batches.filter { $0.type == "attachments" }.map(\.index) == [0, 1, 2])

        let other = try TestVaults.makeState()
        try await other.createVault(password: "a-different-vault-password")
        #expect(try restore(url, into: other).skipped == 0)
        #expect(try other.attachmentRepo?.listBlobIds().count == 3)
    }

    @Test func progressReachesTheEnd() async throws {
        let state = try await makeFullVault()
        let url = try temporaryFile()
        let updates = ProgressLog()

        try BackupService.writeBackup(
            to: url,
            noteRepo: #require(state.noteRepo),
            attachmentRepo: #require(state.attachmentRepo),
            savedSearchRepo: #require(state.savedSearchRepo),
            encryptionRepo: #require(state.encryptionRepo),
            key: #require(state.keyManager.masterKey),
            progress: { updates.record($0) }
        )

        let last = try #require(updates.last)
        #expect(last.completed == last.total)
        #expect(last.total > 0)
    }

    @Test func aCancelledBackupStops() async throws {
        let state = try await makeFullVault()
        let url = try temporaryFile()
        let noteRepo = try #require(state.noteRepo)
        let attachmentRepo = try #require(state.attachmentRepo)
        let savedSearchRepo = try #require(state.savedSearchRepo)
        let encryptionRepo = try #require(state.encryptionRepo)
        let key = try #require(state.keyManager.masterKey)

        let task = Task.detached {
            withUnsafeCurrentTask { $0?.cancel() }
            try BackupService.writeBackup(
                to: url, noteRepo: noteRepo, attachmentRepo: attachmentRepo,
                savedSearchRepo: savedSearchRepo, encryptionRepo: encryptionRepo, key: key
            )
        }

        await #expect(throws: CancellationError.self) {
            _ = try await task.value
        }
    }

    // MARK: - Damaged files

    @Test(arguments: ["", "not json", "{\"type\":\"jottery-encrypted-backup\",\"batches\":[", "{\"type\":\"something-else\",\"version\":\"3.0\",\"encryption\":{\"algorithm\":\"x\"},\"batches\":[]}"])
    func whatIsntABackupIsRefused(text: String) async throws {
        let state = try await makeVault()
        let url = try temporaryFile(Data(text.utf8))

        #expect(throws: BackupError.invalidFormat) {
            _ = try restore(url, into: state)
        }
    }

    /// The file chooses the rounds, before any password is tried (R2-106). Above
    /// UInt32's range it crashed the app, and just below it froze for hours.
    @Test(arguments: [10_000_001, 4_294_967_295, 4_294_967_296, 9_007_199_254_740_991])
    func anAbsurdIterationCountIsRefused(iterations: Int) async throws {
        let state = try await makeVault()
        let header = #"{"version":"3.0","type":"jottery-encrypted-backup","createdAt":"2026-11-05T00:00:00Z","#
            + #""encryption":{"salt":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=","iterations":\#(iterations),"algorithm":"AES-256-GCM"},"#
            + #""batches":[{"type":"notes","index":0,"count":1,"data":"x.y.z.a.b"}],"#
            + #""summary":{"notes":1,"attachments":0,"versions":0,"savedSearches":0}}"#
        let url = try temporaryFile(Data(header.utf8))

        #expect(throws: BackupError.invalidFormat) {
            try restore(url, into: state)
        }
    }

    @Test func aNoteThatCantBeReadIsCountedAndTheRestRestore() async throws {
        let source = try await makeVault()
        let good = try #require(try source.createNote(content: "good"))
        let bad = try #require(try source.createNote(content: "bad"))
        try TestVaults.execute(source, "UPDATE notes SET content = ? WHERE id = ?", [try TestVaults.foreignCiphertext(), bad.id])
        let url = try backup(of: source)

        let other = try TestVaults.makeState()
        try await other.createVault(password: "a-different-vault-password")
        let result = try restore(url, into: other)

        #expect(result == BackupService.RestoreResult(notes: 1, skipped: 1))
        let key = try #require(other.keyManager.masterKey)
        #expect(try other.noteRepo?.get(id: good.id, key: key)?.content == "good")
        #expect(try other.noteRepo?.getRaw(id: bad.id) == nil)
    }

    // MARK: - Older iOS backups

    /// A batch as iOS sealed it before the formats were aligned: raw DEFLATE,
    /// and no authenticated data.
    private func oldBatch(_ items: [[String: Any]], key: SymmetricKey) throws -> String {
        let json = try JSONSerialization.data(withJSONObject: items)
        var compressed = Data(count: json.count + 512)
        let capacity = compressed.count
        let size = json.withUnsafeBytes { source in
            compressed.withUnsafeMutableBytes { destination in
                compression_encode_buffer(
                    destination.bindMemory(to: UInt8.self).baseAddress!, capacity,
                    source.bindMemory(to: UInt8.self).baseAddress!, json.count, nil, COMPRESSION_ZLIB
                )
            }
        }
        let sealed = try AES.GCM.seal(compressed.prefix(size), using: key)
        func encode(_ data: Data) -> String {
            data.base64EncodedString()
                .replacingOccurrences(of: "+", with: "-")
                .replacingOccurrences(of: "/", with: "_")
                .replacingOccurrences(of: "=", with: "")
        }
        let header = encode(Data(#"{"alg":"dir","enc":"A256GCM"}"#.utf8))
        return "\(header)..\(encode(Data(sealed.nonce))).\(encode(sealed.ciphertext)).\(encode(sealed.tag))"
    }

    /// A whole file as those builds wrote it: sorted keys, so batches first,
    /// with `tags`, `attachments` and `hashChain` held as strings.
    private func oldBackup(of state: AppState) throws -> Data {
        let key = try #require(state.keyManager.masterKey)
        let metadata = try #require(try state.encryptionRepo?.get())
        let database = try #require(state.db)
        let records = try database.dbPool.read { try NoteRecord.fetchAll($0) }
        let notes: [[String: Any]] = records.map { record in
            var item: [String: Any] = [
                "id": record.id, "createdAt": record.createdAt, "modifiedAt": record.modifiedAt,
                "content": record.content, "tags": record.tags, "attachments": record.attachments,
                "pinned": record.pinned, "archived": record.archived, "locked": record.locked,
                "deleted": record.deleted, "version": record.version, "wordWrap": record.wordWrap,
                "syntaxLanguage": record.syntaxLanguage, "showPreview": record.showPreview,
            ]
            item["contentHash"] = record.contentHash
            item["hashChain"] = record.hashChain
            return item
        }
        let blobIds = try #require(try state.attachmentRepo?.listBlobIds())
        let attachmentRepo = try #require(state.attachmentRepo)
        let attachments: [[String: Any]] = try blobIds.map { id in
            let blob = try #require(try attachmentRepo.getBlob(id: id))
            return ["id": id, "blob": blob.base64EncodedString()]
        }
        var batches: [[String: Any]] = [
            ["type": "notes", "index": 0, "count": notes.count, "data": try oldBatch(notes, key: key)],
        ]
        if !attachments.isEmpty {
            batches.append(["type": "attachments", "index": 0, "count": attachments.count, "data": try oldBatch(attachments, key: key)])
        }
        let deviceSalt = try #require(metadata.deviceSalt)
        let localWrappedMaster = try #require(metadata.localWrappedMaster)
        return try JSONSerialization.data(withJSONObject: [
            "batches": batches,
            "createdAt": Date().iso8601,
            "encryption": [
                "envelopeVersion": 1,
                "deviceSalt": deviceSalt,
                "localWrappedMaster": localWrappedMaster,
                "wrappingKdfVersion": 1,
                "algorithm": metadata.algorithm,
            ] as [String: Any],
            "summary": ["attachments": attachments.count, "notes": notes.count, "savedSearches": 0, "versions": 0],
            "type": "jottery-encrypted-backup",
            "version": "3.0",
        ] as [String: Any], options: [.sortedKeys])
    }

    @Test func anOlderIOSBackupStillRestores() async throws {
        let source = try await makeVault()
        let note = try #require(try source.createNote(content: "from an older build", tags: ["old", "backup"]))
        try source.addAttachment(to: note.id, data: Data("old bytes".utf8), filename: "old.txt", mimeType: "text/plain")
        let url = try temporaryFile(try oldBackup(of: source))

        let other = try TestVaults.makeState()
        try await other.createVault(password: "a-different-vault-password")
        let result = try restore(url, into: other)

        #expect(result == BackupService.RestoreResult(notes: 1, skipped: 0))
        let key = try #require(other.keyManager.masterKey)
        let restored = try #require(try other.noteRepo?.get(id: note.id, key: key))
        #expect(restored.content == "from an older build")
        #expect(restored.tags.sorted() == ["backup", "old"])
        #expect(restored.attachments.first?.filename == "old.txt")
        let blobId = try #require(restored.attachments.first?.data)
        let blob = try #require(try other.attachmentRepo?.getBlob(id: blobId))
        #expect(try decryptBlob(blob, key: key) == Data("old bytes".utf8))
    }

    @Test func anOlderIOSBackupRefusesTheWrongPassword() async throws {
        let source = try await makeVault()
        _ = try source.createNote(content: "secret")
        let url = try temporaryFile(try oldBackup(of: source))

        let other = try await makeVault()
        #expect(throws: BackupError.wrongPassword) {
            _ = try restore(url, password: "not-the-password", into: other)
        }
    }
}

private final class ProgressLog: @unchecked Sendable {
    private let lock = NSLock()
    private var updates: [BackupService.Progress] = []

    func record(_ update: BackupService.Progress) {
        lock.withLock { updates.append(update) }
    }

    var last: BackupService.Progress? {
        lock.withLock { updates.last }
    }
}

/// Written by the web client's `createBatchedBackup` and `downloadBackup`, under
/// the notes password used above. Holds three notes (one with two tags, a file,
/// a pin and a colour; one with no tags; one with a single tag), a version
/// snapshot, a saved search, and the settings batch iOS has no use for.
private let webBackupFixture = #"""
{
"version":"3.0",
"type":"jottery-encrypted-backup",
"createdAt":"2026-11-05T02:33:39.972Z",
"encryption":{"envelopeVersion":1,"deviceSalt":"6dS05+p+737MMsEG9fbULgnf9iLGWCYO9mX4PaHpS64=","localWrappedMaster":"{\"ciphertext\":\"rNsa9fBHP60hdpWnoGVEB/F1q6VvJ0VtE+06vgaMH8DRTVPpkuGIXa814eY7vHyH\",\"iv\":\"H876DM20xeZ9SNHC\"}","wrappingKdfVersion":1,"createdAt":"2026-11-05T02:33:39.958Z","algorithm":"AES-256-GCM"},
"batches":[
{"type":"notes","index":0,"count":3,"data":"eyJhbGciOiJkaXIiLCJlbmMiOiJBMjU2R0NNIn0..LbLfI6F1d6ugqUvz.m0XV2DU1W4wsNODJdcf-o6FJTXAKQSU9HmzU2MKXQUlrlGo2KkCNFB8PWtcnFgLSbx4_N1lDwYZBFH4U47KAa5D2VDdV73zpZm9XduIsQiwAdH8XLbRSPKG8hL7M9P5PFIOx-o4FEKfztTIN1rMJYgfZe379-utpd7dY6Nxz2AAh-hvGs6e1GlnKTTj0gkxgtLps0ZaFO4-QP0baojVlOAzdQSOKad3DLCenGwc0H78RtH8BUotw70eVQpncnTlPpt-R884Ce44qlauNJMy-6-qNQtmJR8iQHOWZl2i21mCbmbKNNwPTgnTxXZvDcAuqpl8cSFn3xVTHKX6KyrwUnA34OjuXe7V1XaRmwzDEIJeY2q4tDmr1hs41HTRJQ2kTlsWtC0KWsXpb3B1TzWJ76Z5Cp0srGwi6B04a9Mh35NuWScHZIwHVmavntugrAf9Imss2_lobUJ7GnO4DE-AfzWvzEnWNw0llT8_BJqIo7JeoefU-zy7UZFa4GGllWMEy3gXHaN6EjjfXaF8d9gyGMc8DhIVKmem9JSfgjGs_OMM6v15AQhbBfuKjrO_JlZ_stidGCaXqsVjxobE7srxScCciTafCAHKlrMaIdwCo1zIDmOCpxcNPjfdXvuP4G9gTwWjmfNlofm8MBbEOKeq35CHoiZtuIX6mHgNmHNzL5Fb1KPgmOZs2TccfoaaM6ignitPInebbxUNtrTcjzswSzeX0M-2nPYvB_tx4aXAldyk9HWsIw8RtS1FOTnysPtgb8HGXaP8_1ZsZ124hzD3eRx1VQCkY58zSW9iGlmgaNCRxtZOVmBP0DSabTOAjHJSZjOUcm9rWz-S74Ft4p2oRZzfrgjkclXUGlXDflTT4tMkQzuW9sHeMb1kMlcbYcKzJf53jjwBsS53pGyDk3PJiT4MwQJJ3DjDFy4ze5uULTYx2ArQgGkY1LrhqSwRwWmD1TFFkq5ty9xFz7Lz9O8hwPGK9eOnKDMrUk3vJRGtpi_tmT9S_xP7jLaUsM1utIga3WX6nkmsmrZ8ixqC4vplYGPElYbMFLEOdXS3srGbAy05HeYe_r2RSNKKR1HnYyXiDvx5WU3UocnI3iW5kmQxvJjXoMib_hvr7aGmyPaIkcpW0gaD30SqPRYwXZ_Vk5dcUfc_wesZHDIxl5YVyLXoa1r_squJZL1eJClvKfGUxAj4fwsFBr5wZBryWCNOhvsk7.Xtd06GrLW2vbldiZtVm5Pw"},
{"type":"attachments","index":0,"count":1,"data":"eyJhbGciOiJkaXIiLCJlbmMiOiJBMjU2R0NNIn0..bnGCiteVDhYlk5-4.TAnCeSBiSUPBoXd77zT70BEKFAU6LQeaegzHqMjjLgX3SpTuFuRQboFbcvyBXDFoRVmhzh1cZtelyGTqy3mT5g02OJGDA_QH-GfNkQZmjMEUdURkihS02iM9IWkDbj0q5O1uv78tXmcknHecq7rEfeFEqRBy-IjaqOZEJfCHBquomo2LT2SUECtrjE7iBpK7hGAA48vBWj4VTZAeAmOik09ahlHrpyKd.zULIpYEPYSAhG-JnzJ8kzQ"},
{"type":"versions","index":0,"count":1,"data":"eyJhbGciOiJkaXIiLCJlbmMiOiJBMjU2R0NNIn0..HERglc4393MN6cWU.PeJncIDr0p92n79Gmnab7UtlMZ9Ah3vcfFCMKYlUCk9hDDh2k7_tcnnEkZVRJWqH4DODLwuj2F2jU-K9xrE7wqR3N2KRY1uQDnRuMgKdKWtI0YSLQnCVrAprwYA3wS7hL4rFP8q84Fag4a7JrkH7t_IWWFb1u66cEvBX17OTPc1e-DOZOCOPc5vZYOD5chKpmeP34bw5aUwtYnSagzsqsJbCJfAtG0TkrjukM9FUEaCQgm78wNfCa-euWsXvixReju3671v3EFy7ycbgHsd18QehkB6SrqY6R-nj7pmBOct_3AXJjbWdKgpJD6nGsfkajQZzdLbP9V_L0d65L56NP7aqzIKLnO9jVuUwD14J-njXPFSPryHlAjzFqNYz4eDm050bPCAqnPk6QvE70AX1MLu6oVYFItTy2KXhZ-IHs6t8ksIU1VmQ2cGSlqKtAuQYyz1d2H06To7uZ-iX_FivHpTJrHprhJpLE2doNCCKP0MiCI3AabhQGeUKra1n-V1auNtVlISS6sTLwwWifF25tjDwOSDkbzivtJPvUmstMENp5X7fEgrgrDGbAlSKmLIBUo9Axfqjw06y5NDc2DeigIZ5DnZATM893UyKV5VE.kSvRWT-5_2NXlLSueuCR6g"},
{"type":"saved_searches","index":0,"count":1,"data":"eyJhbGciOiJkaXIiLCJlbmMiOiJBMjU2R0NNIn0..xeM_2yxFFviLVV4l.icQP5idiBwUuAGZRA_TN4cM6_2I75mRhKC10Ik-dDAywvOjniltxoGmzqM0a_TxR98m4lXEqEdMCrZQ59l1alb1BHbFVKPgCPAyVz_65Fw95nK6g2sqgjLYBDRaEgehy_G3ocLjNl_xaS-RbsAjbfkiIpDrmHqqwSA9PwmMRWl3qeK55FUiluFoqh00md11TkJ4Ge7Tekt4Korvcz2JPkCTDeKiVvSyVVwwzu8JeejrabBnsa4Zabp2FTpTZhsoS3yryaxP8X82WXIkYoR4m9Mst7-wYXTX_YHnlDyQ0DAIFmGjZBJkpNw3gwxAO4oTTr9tqvJuMidz4tF6i-6zX6teDZltXti4T2zM1dA.VEW7AO06ZSMVoqJbgtg-Bw"},
{"type":"settings","index":0,"count":1,"data":"eyJhbGciOiJkaXIiLCJlbmMiOiJBMjU2R0NNIn0..s1j-PwbHxd_Y54sa.KIZnRx43MRCxfvdNZMGDetT4M8QRjbHw9xLU1S7bm7TxfosgsqZ2y84QAGWZJ0nExEznYQNO7fw-txihDE0vuWfv24IPp_4nKo0thXtRbExMCvb2UNkeoXXQocKqZ6oC6st6n0eIK1v1iJBllgs6-RqV35eY-uieOn1i-Zzhm5bdfnsI6h1Bn85b8TPkMfFyxhkWQZRbWadTuquE50qJPrfpthBIeq0EbzzPBTbNMn33PxzYDbiwqU6FD7Myt6xvcT-5oWkMpiDfTErA2IWuVidBK6tqnmcPM1oTUfAfx97IZLhWaCvVoHkepQW2OuXQOF0lBU5h-Lc7NH-iEU7MoKjw9uH5E--wzvP_8eBvs7xY7YJT4Rzm-Bi14OyOasgAFE33XmOQCCHAlkbtPa0rH-k5OqN3LAbHaCC56WaJn2IJUKQBUf9mZGYaNoz3rJVZoPexJ3EtSOXy24vw57cgWVDCUDvdCDXZaxz6Kjbp4sRlGz4jg3-BRCMknnxytn-vn3EqwX-ndFU51uUM-F29l2RbOHtJioM7NuzdAriuZL_M6kicCDd82cbv8OiLpewvLTMgRHMPBqr9Gx2K9bzXuhH5mmnjga0XJoECmg0Fn4cGf-UQxQzzjbCf6rakAK7dGBKTh6GnyfRGCEOd0c0iy7-DqMbYOeL43eAToTvFvgz32vmzjiJaElmrQsrYXTpxRKkNUoIlKK6q-Dsu873_7_J2XxzWm2hMtm4vcJlyohtwbiHTR69nUWgD1SDiNO7B0aX_uGH_mbdON5Ivc1M-stm3VopX77_8-FQFgorvFyFoLTOE25ZIeLzbg1czMYpnulbKo8a6fBkZHKD2TXlhYmtgtZ48fELZb46VSag5DtTrH2BgHVcY_0yP49Q8UyUV0IkA0LwrgYmpfFJHjhMq6xKd7V2w-3s-UAH5gQYbrQKGKL-ERVvGCbMzsg-3Q1VZjuQydmqeoUI-mMPNRT3RJOSKhIfEpk3GrmckqNICy4wU6QnIEhYNFC-OBUaaUugYLmaZxkfs2i5qoJsVgP2cCTMsZT7T_qeHlTb8srTSYfJRdTt0rORnd9LTyTmBisPZs1TfEpDTGuanwS4IKxFyP7nAYPhXyzz8giM-zKLkach1TgkOqu9T3YT_wE96Jfe4S21C1lGpDiHuNLGOOjnmMMd1IhjKHZ0D8Zua3OXKZWrlVtZrN-odJVqdbCqobY0.FZ6miWqlRjhbGr96ReyZwQ"}
],
"summary":{"notes":3,"attachments":1,"versions":1,"savedSearches":1}
}
"""#
