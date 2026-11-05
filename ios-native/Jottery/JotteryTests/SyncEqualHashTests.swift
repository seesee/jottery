import CryptoKit
import Foundation
import Testing

@testable import Jottery

private final class PullStubURLProtocol: URLProtocol, @unchecked Sendable {
    nonisolated(unsafe) static var body = Data()

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        let response = HTTPURLResponse(
            url: request.url!, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: nil
        )!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: Self.body)
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}
}

/// Deleting, archiving, pinning, locking and changing attachments don't
/// recompute a note's content hash on every client, and older web clients
/// restore a version without doing so. The server stores those changes anyway,
/// so an equal hash can't mean the notes are the same: a pull has to look at
/// what those operations change, or other devices never see them.
@Suite(.serialized)
struct SyncEqualHashTests {

    private let key = SymmetricKey(data: Data(repeating: 7, count: 32))

    private struct Fixture {
        let repo: NoteRepository
        let service: SyncService
    }

    private func makeFixture() throws -> Fixture {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-equal-hash-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let db = try DatabaseManager(path: dir.appendingPathComponent("test.db").path)
        let versionRepo = VersionRepository(db: db)
        let repo = NoteRepository(db: db, versionRepo: versionRepo)

        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [PullStubURLProtocol.self]
        let client = SyncClient(endpoint: "https://sync.example.test", apiKey: "device-key", configuration: configuration)
        let service = SyncService(
            syncClient: client,
            noteRepo: repo,
            syncRepo: SyncRepository(db: db),
            versionRepo: versionRepo,
            attachmentRepo: AttachmentRepository(db: db),
            savedSearchRepo: SavedSearchRepository(db: db),
            key: key
        )
        return Fixture(repo: repo, service: service)
    }

    /// A note this device has fully synced.
    private func makeCleanNote(in fixture: Fixture) throws -> NoteRecord {
        let note = try fixture.repo.create(content: "original", tags: ["kept"], key: key)
        try fixture.repo.markSynced(id: note.id, syncedAt: Date().iso8601)
        return try #require(try fixture.repo.getRaw(id: note.id))
    }

    /// The note as the server would send it back, with the same hash.
    private func remote(
        of record: NoteRecord,
        content: String? = nil,
        attachments: [AttachmentRef]? = nil,
        pinned: Bool? = nil,
        archived: Bool? = nil,
        locked: Bool? = nil,
        deleted: Bool? = nil
    ) throws -> SyncNote {
        SyncNote(
            id: record.id,
            createdAt: record.createdAt,
            modifiedAt: Date().iso8601,
            content: content ?? record.content,
            tags: [try CryptoService.encryptTagForSync("kept", key: key)],
            attachments: attachments ?? [],
            pinned: pinned ?? record.pinned,
            archived: archived ?? record.archived,
            locked: locked ?? record.locked,
            deleted: deleted ?? record.deleted,
            deletedAt: (deleted ?? record.deleted) ? Date().iso8601 : nil,
            version: record.version + 1,
            wordWrap: record.wordWrap,
            syntaxLanguage: record.syntaxLanguage,
            showPreview: record.showPreview,
            color: record.color,
            contentHash: record.contentHash,
            parentHash: record.parentHash,
            hashChain: nil
        )
    }

    @discardableResult
    private func pull(_ note: SyncNote, into fixture: Fixture) async throws -> SyncChanges {
        PullStubURLProtocol.body = try JSONEncoder().encode(
            SyncPullResponse(notes: [note], attachments: [], versions: [], hasMore: false)
        )
        return try await fixture.service.pull()
    }

    @Test func aPinMadeElsewhereArrives() async throws {
        let fixture = try makeFixture()
        let record = try makeCleanNote(in: fixture)

        let changes = try await pull(remote(of: record, pinned: true), into: fixture)

        #expect(try #require(try fixture.repo.getRaw(id: record.id)).pinned)
        #expect(changes.updatedIds.contains(record.id))
    }

    @Test func aDeletionMadeElsewhereArrives() async throws {
        let fixture = try makeFixture()
        let record = try makeCleanNote(in: fixture)

        try await pull(remote(of: record, deleted: true), into: fixture)

        #expect(try #require(try fixture.repo.getRaw(id: record.id)).deleted)
    }

    @Test func anArchiveMadeElsewhereArrives() async throws {
        let fixture = try makeFixture()
        let record = try makeCleanNote(in: fixture)

        try await pull(remote(of: record, archived: true), into: fixture)

        #expect(try #require(try fixture.repo.getRaw(id: record.id)).archived)
    }

    @Test func aLockMadeElsewhereArrives() async throws {
        let fixture = try makeFixture()
        let record = try makeCleanNote(in: fixture)

        try await pull(remote(of: record, locked: true), into: fixture)

        #expect(try #require(try fixture.repo.getRaw(id: record.id)).locked)
    }

    @Test func anAttachmentAddedElsewhereArrives() async throws {
        let fixture = try makeFixture()
        let record = try makeCleanNote(in: fixture)
        let ref = AttachmentRef(id: UUID().uuidString, filename: "{}", mimeType: "text/plain", size: 1, data: UUID().uuidString)

        try await pull(remote(of: record, attachments: [ref]), into: fixture)

        let stored = try #require(try fixture.repo.getRaw(id: record.id))
        #expect(stored.attachments.contains(ref.id))
    }

    /// Older web clients restore a version without recomputing the hash. A hash
    /// covers the ciphertext, so different ciphertext can't be the same note.
    @Test func aRestoredVersionArrives() async throws {
        let fixture = try makeFixture()
        let record = try makeCleanNote(in: fixture)
        let restored = try CryptoService.serializeEncryptedJSON(CryptoService.encryptText("restored", key: key))

        try await pull(remote(of: record, content: restored), into: fixture)

        #expect(try fixture.repo.get(id: record.id, key: key)?.content == "restored")
    }

    @Test func tagsSurviveAStateChange() async throws {
        let fixture = try makeFixture()
        let record = try makeCleanNote(in: fixture)

        try await pull(remote(of: record, pinned: true), into: fixture)

        #expect(try fixture.repo.get(id: record.id, key: key)?.tags == ["kept"])
    }

    @Test func aNoteThatHasntChangedIsLeftAlone() async throws {
        let fixture = try makeFixture()
        let record = try makeCleanNote(in: fixture)

        let changes = try await pull(remote(of: record), into: fixture)

        #expect(changes.updatedIds.isEmpty)
        #expect(try fixture.repo.getRaw(id: record.id)?.modifiedAt == record.modifiedAt)
    }

    /// Local changes still waiting to go up win: the next push carries them.
    @Test func anUnsyncedLocalChangeIsNotOverwritten() async throws {
        let fixture = try makeFixture()
        let record = try makeCleanNote(in: fixture)
        try fixture.repo.togglePin(id: record.id)

        let changes = try await pull(remote(of: record, pinned: false, archived: true), into: fixture)

        let stored = try #require(try fixture.repo.getRaw(id: record.id))
        #expect(stored.pinned)
        #expect(!stored.archived)
        #expect(stored.needsSync)
        #expect(changes.updatedIds.isEmpty)
    }

    /// `markSynced` compares `modified_at` to spot a change made during a push.
    /// A delete or restore that left it alone was marked synced and then undone
    /// by the pull that follows.
    @Test func aDeleteMadeDuringAPushStaysDeleted() async throws {
        let fixture = try makeFixture()
        let pushed = try makeCleanNote(in: fixture)
        let snapshot = pushed.modifiedAt
        try await Task.sleep(for: .milliseconds(5))

        try fixture.repo.softDelete(id: pushed.id)
        try fixture.repo.markSynced(id: pushed.id, syncedAt: Date().iso8601, ifModifiedAt: snapshot)
        try await pull(remote(of: pushed, deleted: false), into: fixture)

        let stored = try #require(try fixture.repo.getRaw(id: pushed.id))
        #expect(stored.deleted)
        #expect(stored.needsSync)
    }

    @Test func aRestoreMadeDuringAPushStaysRestored() async throws {
        let fixture = try makeFixture()
        let record = try makeCleanNote(in: fixture)
        try fixture.repo.softDelete(id: record.id)
        let deleted = try #require(try fixture.repo.getRaw(id: record.id))
        try fixture.repo.markSynced(id: record.id, syncedAt: Date().iso8601)
        try await Task.sleep(for: .milliseconds(5))

        try fixture.repo.restore(id: record.id)
        try fixture.repo.markSynced(id: record.id, syncedAt: Date().iso8601, ifModifiedAt: deleted.modifiedAt)

        #expect(try #require(try fixture.repo.getRaw(id: record.id)).needsSync)
    }
}

/// Older builds pushed attachment filenames in plaintext, so a pull can bring
/// them in. A server can also send any name it likes in that field. In the same
/// suite because the stub's response is shared, and suites run side by side.
extension SyncEqualHashTests {

    private func pullNote(withFilename filename: String) async throws -> (repo: NoteRepository, id: String) {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-pulled-filename-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let db = try DatabaseManager(path: dir.appendingPathComponent("test.db").path)
        let versionRepo = VersionRepository(db: db)
        let repo = NoteRepository(db: db, versionRepo: versionRepo)
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [PullStubURLProtocol.self]
        let service = SyncService(
            syncClient: SyncClient(endpoint: "https://sync.example.test", apiKey: "device-key", configuration: configuration),
            noteRepo: repo,
            syncRepo: SyncRepository(db: db),
            versionRepo: versionRepo,
            attachmentRepo: AttachmentRepository(db: db),
            savedSearchRepo: SavedSearchRepository(db: db),
            key: key
        )

        let id = CryptoService.generateUUID()
        let now = Date().iso8601
        let content = try CryptoService.serializeEncryptedJSON(CryptoService.encryptText("pulled", key: key))
        let attachmentId = CryptoService.generateUUID()
        let note = SyncNote(
            id: id, createdAt: now, modifiedAt: now, content: content, tags: [],
            attachments: [AttachmentRef(id: attachmentId, filename: filename, mimeType: "text/plain", size: 1, data: attachmentId)],
            pinned: false, archived: false, deleted: false, version: 1
        )
        PullStubURLProtocol.body = try JSONEncoder().encode(
            SyncPullResponse(notes: [note], attachments: [], versions: [], hasMore: false)
        )
        try await service.pull()
        return (repo, id)
    }

    @Test func aPlaintextNameIsEncryptedAndPushedBack() async throws {
        let (repo, id) = try await pullNote(withFilename: "passport-scan.jpg")

        let record = try #require(try repo.getRaw(id: id))
        #expect(!record.attachments.contains("passport-scan"))
        #expect(record.needsSync)
        #expect(try repo.get(id: id, key: key)?.attachments.first?.filename == "passport-scan.jpg")
    }

    @Test func aHostileNameCantEscapeOnceStored() async throws {
        let (repo, id) = try await pullNote(withFilename: "../../Library/Application Support/Jottery/jottery.db")

        #expect(try repo.get(id: id, key: key)?.attachments.first?.filename == "jottery.db")
    }

    @Test func anEncryptedNameIsStoredAsItCame() async throws {
        let filename = try AttachmentFilename.encrypted("fine.txt", key: key)
        let (repo, id) = try await pullNote(withFilename: filename)

        let record = try #require(try repo.getRaw(id: id))
        #expect(record.attachments.contains("ciphertext"))
        #expect(!record.needsSync)
    }
}
