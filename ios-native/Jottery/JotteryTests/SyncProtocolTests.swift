import CryptoKit
import Foundation
import GRDB
import Testing

@testable import Jottery

/// A sync server that answers from a script and records what it's asked.
/// Scoped to this file, as swift-testing runs top-level suites concurrently.
private final class ScriptedServerProtocol: URLProtocol, @unchecked Sendable {

    struct Reply {
        var status = 200
        var body = Data("{}".utf8)
        var headers: [String: String] = [:]
    }

    nonisolated(unsafe) static var pullRequests: [SyncPullRequest] = []
    nonisolated(unsafe) static var pushRequests: [SyncPushRequest] = []
    /// Each pull takes the next one. When they run out, pulls get an empty page.
    nonisolated(unsafe) static var pullReplies: [Reply] = []
    nonisolated(unsafe) static var pushHandler: (SyncPushRequest) -> Reply = { acceptEverything($0) }

    static func reset() {
        pullRequests = []
        pushRequests = []
        pullReplies = []
        pushHandler = { acceptEverything($0) }
    }

    static func acceptEverything(_ request: SyncPushRequest) -> Reply {
        let response = SyncPushResponse(
            accepted: request.notes.map { SyncAccepted(id: $0.id, serverVersion: 1, syncedAt: nil) },
            rejected: []
        )
        return Reply(body: (try? JSONEncoder().encode(response)) ?? Data())
    }

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        let reply: Reply
        let path = request.url?.path ?? ""
        let body = request.httpBody ?? Self.read(request.httpBodyStream)
        if path.hasSuffix("sync/pull"), let pull = try? JSONDecoder().decode(SyncPullRequest.self, from: body) {
            Self.pullRequests.append(pull)
            reply = Self.pullReplies.isEmpty ? Self.emptyPage : Self.pullReplies.removeFirst()
        } else if path.hasSuffix("sync/push"), let push = try? JSONDecoder().decode(SyncPushRequest.self, from: body) {
            Self.pushRequests.append(push)
            reply = Self.pushHandler(push)
        } else {
            reply = Reply(status: 404)
        }
        let response = HTTPURLResponse(
            url: request.url!, statusCode: reply.status, httpVersion: "HTTP/1.1", headerFields: reply.headers
        )!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: reply.body)
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}

    private static let emptyPage = Reply(body: Data(#"{"notes":[],"attachments":[],"versions":[]}"#.utf8))

    private static func read(_ stream: InputStream?) -> Data {
        guard let stream else { return Data() }
        stream.open()
        defer { stream.close() }
        var data = Data()
        var buffer = [UInt8](repeating: 0, count: 4096)
        while stream.hasBytesAvailable {
            let count = stream.read(&buffer, maxLength: buffer.count)
            if count <= 0 { break }
            data.append(buffer, count: count)
        }
        return data
    }
}

private func page(
    notes: [SyncNote] = [],
    deletions: [SyncDeletion]? = nil,
    attachments: [SyncAttachment] = [],
    syncedAt: String? = "2026-11-05T09:23:45.123456789+00:00",
    hasMore: Bool = false,
    nextCursor: String? = nil
) -> ScriptedServerProtocol.Reply {
    let response = SyncPullResponse(
        notes: notes, deletions: deletions, attachments: attachments, versions: [],
        syncedAt: syncedAt, hasMore: hasMore, nextCursor: nextCursor
    )
    return ScriptedServerProtocol.Reply(body: (try? JSONEncoder().encode(response)) ?? Data())
}

/// The additions in docs/sync-protocol.md, against servers with and without them.
@Suite(.serialized)
struct SyncProtocolTests {

    private let key = SymmetricKey(data: Data(repeating: 7, count: 32))

    private struct Fixture {
        let db: DatabaseManager
        let repo: NoteRepository
        let syncRepo: SyncRepository
        let attachmentRepo: AttachmentRepository
        let versionRepo: VersionRepository
        let service: SyncService
    }

    private func makeFixture() throws -> Fixture {
        ScriptedServerProtocol.reset()
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-sync-protocol-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let db = try DatabaseManager(path: dir.appendingPathComponent("test.db").path)
        let versionRepo = VersionRepository(db: db)
        let repo = NoteRepository(db: db, versionRepo: versionRepo)
        let syncRepo = SyncRepository(db: db)
        let attachmentRepo = AttachmentRepository(db: db)

        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [ScriptedServerProtocol.self]
        let client = SyncClient(endpoint: "https://sync.example.test", apiKey: "device-key", configuration: configuration)
        return Fixture(
            db: db, repo: repo, syncRepo: syncRepo, attachmentRepo: attachmentRepo, versionRepo: versionRepo,
            service: SyncService(
                syncClient: client, noteRepo: repo, syncRepo: syncRepo, versionRepo: versionRepo,
                attachmentRepo: attachmentRepo, savedSearchRepo: SavedSearchRepository(db: db), key: key
            )
        )
    }

    private func remoteNote(
        id: String = UUID().uuidString,
        content: String = "from the server",
        ciphertext: String? = nil,
        contentHash: String? = nil,
        parentHash: String? = nil,
        hashChain: [String]? = nil,
        modifiedAt: String = "2026-11-05T09:00:00Z",
        deleted: Bool = false,
        deletedAt: String? = nil,
        showPreview: Bool? = nil
    ) throws -> SyncNote {
        SyncNote(
            id: id, createdAt: "2026-01-01T00:00:00Z", modifiedAt: modifiedAt,
            content: try ciphertext ?? CryptoService.serializeEncryptedJSON(CryptoService.encryptText(content, key: key)),
            tags: [], attachments: [], pinned: false, archived: false,
            deleted: deleted, deletedAt: deletedAt, version: 1, showPreview: showPreview,
            contentHash: contentHash, parentHash: parentHash, hashChain: hashChain
        )
    }

    private func synced(_ repo: NoteRepository, content: String = "local") throws -> NoteRecord {
        let note = try repo.create(content: content, key: key)
        try repo.markSynced(id: note.id, syncedAt: Date().iso8601)
        return try #require(try repo.getRaw(id: note.id))
    }

    // MARK: - The pull cursor

    @Test func aPageWithNoNotesDoesNotEndACursorPull() async throws {
        let fixture = try makeFixture()
        let note = try remoteNote()
        ScriptedServerProtocol.pullReplies = [
            page(hasMore: true, nextCursor: "c1"),          // all versions, say
            page(notes: [note], hasMore: false, nextCursor: "c2"),
        ]

        try await fixture.service.pull()

        #expect(ScriptedServerProtocol.pullRequests.map(\.cursor) == ["", "c1"])
        #expect(try fixture.repo.getRaw(id: note.id) != nil)
        #expect(try fixture.syncRepo.getMetadata()?.pullCursor == "c2")
    }

    @Test func theNextPullStartsFromTheStoredCursor() async throws {
        let fixture = try makeFixture()
        try fixture.syncRepo.updatePullCursor("stored")
        try fixture.syncRepo.updateLastSync(at: "2026-11-01T00:00:00Z")
        ScriptedServerProtocol.pullReplies = [page(nextCursor: "stored")]

        try await fixture.service.pull()

        let request = try #require(ScriptedServerProtocol.pullRequests.first)
        #expect(ScriptedServerProtocol.pullRequests.count == 1)
        #expect(request.cursor == "stored")
        #expect(request.lastSyncAt == "2026-11-01T00:00:00Z")
    }

    @Test func aPullStoppedPartwayKeepsTheLastAppliedCursor() async throws {
        let fixture = try makeFixture()
        try fixture.syncRepo.updatePullCursor("start")
        try fixture.syncRepo.updateLastSync(at: "2026-11-01T00:00:00Z")
        let applied = try remoteNote()
        ScriptedServerProtocol.pullReplies = [
            page(notes: [applied], hasMore: true, nextCursor: "c1"),
            .init(status: 500, body: Data("boom".utf8)),
        ]

        await #expect(throws: (any Error).self) { try await fixture.service.pull() }

        let metadata = try #require(try fixture.syncRepo.getMetadata())
        #expect(metadata.pullCursor == "c1")
        #expect(metadata.lastSyncAt == "2026-11-01T00:00:00Z")
        #expect(try fixture.repo.getRaw(id: applied.id) != nil)
    }

    /// Timestamp pulls can have skipped changes that nothing can detect, so the
    /// first time cursors work, everything is fetched once.
    @Test func theFirstPullWithCursorsStartsAgainFromTheTop() async throws {
        let fixture = try makeFixture()
        try fixture.syncRepo.updateLastSync(at: "2026-11-01T00:00:00Z")
        ScriptedServerProtocol.pullReplies = [
            page(nextCursor: "since-then"),
            page(nextCursor: "from-the-top"),
        ]

        try await fixture.service.pull()

        let requests = ScriptedServerProtocol.pullRequests
        #expect(requests.map(\.cursor) == ["", ""])
        #expect(requests.map(\.lastSyncAt) == ["2026-11-01T00:00:00Z", nil])
        #expect(try fixture.syncRepo.getMetadata()?.pullCursor == "from-the-top")
    }

    @Test func aFirstEverPullDoesNotPullTwice() async throws {
        let fixture = try makeFixture()
        ScriptedServerProtocol.pullReplies = [page(nextCursor: "c1")]

        try await fixture.service.pull()

        #expect(ScriptedServerProtocol.pullRequests.count == 1)
    }

    @Test func aServerThatNeverMovesItsCursorCantHoldThePullForEver() async throws {
        let fixture = try makeFixture()
        ScriptedServerProtocol.pullReplies = [
            page(hasMore: true, nextCursor: "stuck"),
            page(hasMore: true, nextCursor: "stuck"),
            page(hasMore: true, nextCursor: "stuck"),
        ]

        try await fixture.service.pull()

        #expect(ScriptedServerProtocol.pullRequests.count == 2)
    }

    @Test func aCursorThatIsntAShortAsciiStringIsNotKept() async throws {
        let fixture = try makeFixture()
        ScriptedServerProtocol.pullReplies = [page(nextCursor: String(repeating: "x", count: 1000))]

        try await fixture.service.pull()

        #expect(try fixture.syncRepo.getMetadata()?.pullCursor == nil)
    }

    // MARK: - An older server

    @Test func anOlderServerIsPagedByOffset() async throws {
        let fixture = try makeFixture()
        let notes = try (0..<2).map { _ in try remoteNote() }
        ScriptedServerProtocol.pullReplies = [
            page(notes: notes, hasMore: true),
            page(notes: [try remoteNote()], hasMore: false),
        ]

        try await fixture.service.pull()

        #expect(ScriptedServerProtocol.pullRequests.map(\.offset) == [0, 2])
        #expect(try fixture.syncRepo.getMetadata()?.pullCursor == nil)
    }

    /// An offset can't move past an empty page, so this keeps an older server
    /// from spinning. It's the only case where notes decide when to stop.
    @Test func anOlderServerSayingMoreWithNoNotesEndsThePull() async throws {
        let fixture = try makeFixture()
        ScriptedServerProtocol.pullReplies = [page(hasMore: true), page(hasMore: true)]

        try await fixture.service.pull()

        #expect(ScriptedServerProtocol.pullRequests.count == 1)
    }

    @Test func aServerThatLosesCursorSupportFallsBackToTimestamps() async throws {
        let fixture = try makeFixture()
        try fixture.syncRepo.updatePullCursor("stored")
        try fixture.syncRepo.updateLastSync(at: "2026-11-01T00:00:00Z")
        ScriptedServerProtocol.pullReplies = [page()]

        try await fixture.service.pull()

        let metadata = try #require(try fixture.syncRepo.getMetadata())
        #expect(metadata.pullCursor == nil)
        #expect(metadata.lastSyncAt != nil)
    }

    // MARK: - lastSyncAt comes from the server (jottery-xvp1)

    @Test func lastSyncAtIsTheServersTimeFromTheFirstPageWithAnOverlap() async throws {
        let fixture = try makeFixture()
        ScriptedServerProtocol.pullReplies = [
            page(syncedAt: "2026-11-05T10:00:00Z", hasMore: true, nextCursor: "c1"),
            page(syncedAt: "2026-11-05T11:00:00Z", nextCursor: "c2"),
        ]

        try await fixture.service.pull()

        let stored = try #require(try fixture.syncRepo.getMetadata()?.lastSyncAt)
        let expected = try #require(Date(iso8601: "2026-11-05T10:00:00Z")).addingTimeInterval(-SyncService.pullOverlap)
        #expect(Date(iso8601: stored) == expected)
    }

    @Test func aRustTimestampParses() {
        // What chrono's to_rfc3339() sends: nanoseconds and a numeric offset.
        #expect(Date(iso8601: "2026-11-05T09:23:45.123456789+00:00") != nil)
    }

    // MARK: - Applying a pulled note

    private func local(hash: String, parent: String? = nil, chain: [String] = [], needsSync: Bool) throws -> NoteRecord {
        var record = NoteRecord.new(encryptedContent: "{}", encryptedTags: "{}")
        record.contentHash = hash
        record.parentHash = parent
        record.hashChain = String(data: try JSONEncoder().encode(chain), encoding: .utf8)
        record.needsSync = needsSync
        return record
    }

    private func pulled(hash: String, parent: String? = nil, chain: [String] = [], of record: NoteRecord) throws -> SyncNote {
        try remoteNote(id: record.id, ciphertext: record.content, contentHash: hash, parentHash: parent, hashChain: chain)
    }

    @Test func aPendingEditIsKeptWhenTheSameNoteArrives() throws {
        let record = try local(hash: "a", chain: ["a"], needsSync: true)
        #expect(SyncService.action(forPulled: try pulled(hash: "a", of: record), local: record) == .keepLocal)
    }

    @Test func aPendingEditIsKeptWhenItsParentArrives() throws {
        let record = try local(hash: "b", parent: "a", chain: ["b", "a"], needsSync: true)
        #expect(SyncService.action(forPulled: try pulled(hash: "a", of: record), local: record) == .keepLocal)
    }

    @Test func aPendingEditIsKeptWhenAnOlderAncestorArrives() throws {
        let record = try local(hash: "c", parent: "b", chain: ["c", "b", "a"], needsSync: true)
        #expect(SyncService.action(forPulled: try pulled(hash: "a", of: record), local: record) == .keepLocal)
    }

    @Test func aPulledNoteBuiltOnTheEditReplacesIt() throws {
        let record = try local(hash: "b", parent: "a", chain: ["b", "a"], needsSync: true)
        let newer = try pulled(hash: "c", parent: "b", chain: ["c", "b", "a"], of: record)
        #expect(SyncService.action(forPulled: newer, local: record) == .take)
    }

    /// The doc calls this out: sharing an ancestor doesn't make it newer.
    @Test func sharingOnlyAnAncestorIsAConflict() throws {
        let record = try local(hash: "b", parent: "a", chain: ["b", "a"], needsSync: true)
        let other = try pulled(hash: "x", parent: "a", chain: ["x", "a"], of: record)
        #expect(SyncService.action(forPulled: other, local: record) == .conflict)
    }

    @Test func withNothingPendingTheServersNoteIsTaken() throws {
        let record = try local(hash: "b", parent: "a", chain: ["b", "a"], needsSync: false)
        let other = try pulled(hash: "x", parent: "a", chain: ["x", "a"], of: record)
        #expect(SyncService.action(forPulled: other, local: record) == .take)
    }

    /// Older notes have no hashes. A pending edit used to be overwritten here
    /// by any note that wasn't newer than it.
    @Test func aPendingEditOfANoteWithoutHashesIsNotOverwrittenByAnOlderCopy() throws {
        var record = NoteRecord.new(encryptedContent: "{}", encryptedTags: "{}")
        record.needsSync = true
        record.modifiedAt = "2026-11-05T10:00:00Z"
        let older = try remoteNote(id: record.id, modifiedAt: "2026-11-05T09:00:00Z")
        let newer = try remoteNote(id: record.id, modifiedAt: "2026-11-05T11:00:00+00:00")

        #expect(SyncService.action(forPulled: older, local: record) == .keepLocal)
        #expect(SyncService.action(forPulled: newer, local: record) == .conflict)
    }

    @Test func applyingAPageTwiceChangesNothingTheSecondTime() async throws {
        let fixture = try makeFixture()
        let note = try remoteNote(contentHash: "h", hashChain: ["h"])
        ScriptedServerProtocol.pullReplies = [page(notes: [note]), page(notes: [note])]

        let first = try await fixture.service.pull()
        let second = try await fixture.service.pull()

        #expect(first.updatedIds == [note.id])
        #expect(second.updatedIds.isEmpty)
    }

    // MARK: - Validating what arrives

    @Test func anInvalidNoteIsDroppedAndTheRestOfThePageApplied() async throws {
        let fixture = try makeFixture()
        let good = try remoteNote()
        let badId = try remoteNote(id: "../../etc/passwd")
        let badDate = try remoteNote(modifiedAt: "last tuesday")
        ScriptedServerProtocol.pullReplies = [page(notes: [badId, good, badDate], nextCursor: "c1")]

        let changes = try await fixture.service.pull()

        #expect(changes.updatedIds == [good.id])
        #expect(try fixture.syncRepo.getMetadata()?.pullCursor == "c1")
    }

    @Test func aMalformedNoteDoesNotLoseThePage() async throws {
        let fixture = try makeFixture()
        let good = try remoteNote()
        let body = String(decoding: try JSONEncoder().encode(good), as: UTF8.self)
        ScriptedServerProtocol.pullReplies = [.init(body: Data("""
            {"notes":[{"id":"no other fields"},\(body)],"attachments":[],"versions":[],"hasMore":false}
            """.utf8))]

        let changes = try await fixture.service.pull()

        #expect(changes.updatedIds == [good.id])
    }

    @Test func aDeletionWithABadIdIsIgnored() async throws {
        let fixture = try makeFixture()
        let record = try synced(fixture.repo)
        ScriptedServerProtocol.pullReplies = [page(deletions: [
            SyncDeletion(id: "not-a-uuid", deletedAt: "2026-11-05T09:00:00Z"),
            SyncDeletion(id: record.id, deletedAt: "not a date"),
        ])]

        try await fixture.service.pull()

        #expect(try fixture.repo.getRaw(id: record.id)?.deleted == false)
    }

    @Test func theServerCantMarkNotesItWasntSentAsSynced() async throws {
        let fixture = try makeFixture()
        let sent = try fixture.repo.create(content: "goes up", key: key)
        ScriptedServerProtocol.pushHandler = { _ in
            let response = SyncPushResponse(
                accepted: [SyncAccepted(id: UUID().uuidString), SyncAccepted(id: sent.id)], rejected: []
            )
            return .init(body: (try? JSONEncoder().encode(response)) ?? Data())
        }

        let changes = try await fixture.service.push()

        #expect(changes.updatedIds == [sent.id])
    }

    // MARK: - Authoritative attachments

    @Test func pushesSayTheirAttachmentListsAreComplete() async throws {
        let fixture = try makeFixture()
        _ = try fixture.repo.create(content: "a note", key: key)

        try await fixture.service.push()

        #expect(ScriptedServerProtocol.pushRequests.first?.attachmentsAuthoritative == true)
    }

    /// An unreadable list goes up empty, and must not delete anything.
    @Test func aListThatCantBeReadIsNotCalledComplete() async throws {
        let fixture = try makeFixture()
        let note = try fixture.repo.create(content: "a note", key: key)
        try await fixture.db.dbPool.write { db in
            try db.execute(sql: "UPDATE notes SET attachments = 'nonsense' WHERE id = ?", arguments: [note.id])
        }

        try await fixture.service.push()

        #expect(ScriptedServerProtocol.pushRequests.first?.attachmentsAuthoritative == nil)
    }

    // MARK: - Deletions

    @Test func aPermanentDeleteIsPushedWithNothingElsePending() async throws {
        let fixture = try makeFixture()
        let record = try synced(fixture.repo)

        try fixture.repo.permanentlyDelete(id: record.id)
        try await fixture.service.push()

        let sent = ScriptedServerProtocol.pushRequests.first?.deletions?.map(\.id)
        #expect(sent == [record.id])
    }

    /// `accepted` only ever lists notes, so waiting for the id there meant
    /// every deletion was sent again on every push.
    @Test func aDeletionIsSentOnce() async throws {
        let fixture = try makeFixture()
        let record = try synced(fixture.repo)
        try fixture.repo.permanentlyDelete(id: record.id)

        try await fixture.service.push()
        try await fixture.service.push()

        #expect(ScriptedServerProtocol.pushRequests.count == 1)
        #expect(try fixture.syncRepo.getPendingDeletions().isEmpty)
    }

    @Test func aDeletionTheServerRefusedIsNotSentAgain() async throws {
        let fixture = try makeFixture()
        let record = try synced(fixture.repo)
        try fixture.repo.permanentlyDelete(id: record.id)
        ScriptedServerProtocol.pushHandler = { _ in
            let response = SyncPushResponse(accepted: [], rejected: [], rejectedDeletions: [record.id])
            return .init(body: (try? JSONEncoder().encode(response)) ?? Data())
        }

        try await fixture.service.push()

        #expect(try fixture.syncRepo.getPendingDeletions().isEmpty)
    }

    @Test func aDeletionThatDidntReachTheServerIsKept() async throws {
        let fixture = try makeFixture()
        let record = try synced(fixture.repo)
        try fixture.repo.permanentlyDelete(id: record.id)
        ScriptedServerProtocol.pushHandler = { _ in .init(status: 500) }

        await #expect(throws: (any Error).self) { try await fixture.service.push() }

        #expect(try fixture.syncRepo.getPendingDeletions().map(\.id) == [record.id])
    }

    /// The server hasn't heard about the deletion yet, so a pull can still hold
    /// the note. Refused deletions come back after this, once they're cleared.
    @Test func aNoteDeletedHereIsNotBroughtBackBeforeTheServerHasDecided() async throws {
        let fixture = try makeFixture()
        let record = try synced(fixture.repo)
        try fixture.repo.permanentlyDelete(id: record.id)
        ScriptedServerProtocol.pullReplies = [page(notes: [try remoteNote(id: record.id)])]

        try await fixture.service.pull()

        #expect(try fixture.repo.getRaw(id: record.id) == nil)
    }

    @Test func permanentDeleteTakesItsVersionsAndUnsharedBlobsWithIt() async throws {
        let fixture = try makeFixture()
        let shared = UUID().uuidString, own = UUID().uuidString
        let refs = { (ids: [String]) in
            String(decoding: try JSONEncoder().encode(ids.map {
                AttachmentRef(id: $0, filename: "name", mimeType: "text/plain", size: 1, data: $0)
            }), as: UTF8.self)
        }
        for id in [shared, own] {
            try fixture.attachmentRepo.storeBlob(id: id, mimeType: "text/plain", size: 1, data: Data("x".utf8))
        }
        var going = try synced(fixture.repo)
        going.attachments = try refs([shared, own])
        try fixture.repo.updateRaw(going)
        try fixture.versionRepo.createVersion(from: going, reason: "sync")
        var staying = try synced(fixture.repo)
        staying.attachments = try refs([shared])
        try fixture.repo.updateRaw(staying)

        try fixture.repo.permanentlyDelete(id: going.id)

        #expect(try fixture.attachmentRepo.hasBlob(id: shared))
        #expect(try !fixture.attachmentRepo.hasBlob(id: own))
        #expect(try fixture.versionRepo.getVersions(noteId: going.id).isEmpty)
    }

    // MARK: - An untrusted server can't wipe a device (R2-069)

    @Test func aServerDeletionLeavesTheNoteInTheBin() async throws {
        let fixture = try makeFixture()
        let record = try synced(fixture.repo)
        ScriptedServerProtocol.pullReplies = [page(deletions: [
            SyncDeletion(id: record.id, deletedAt: "2026-11-05T09:00:00Z")
        ])]

        let changes = try await fixture.service.pull()

        #expect(changes.removedByServerIds == [record.id])
        #expect(changes.deletedIds == [record.id])
        #expect(try fixture.repo.getRaw(id: record.id)?.deleted == true)
    }

    @Test func aNoteAlreadyInTheBinIsNotCountedAsRemoved() async throws {
        let fixture = try makeFixture()
        let record = try synced(fixture.repo)
        try fixture.repo.softDelete(id: record.id)
        try fixture.repo.markSynced(id: record.id, syncedAt: Date().iso8601)
        ScriptedServerProtocol.pullReplies = [page(deletions: [
            SyncDeletion(id: record.id, deletedAt: "2026-11-05T09:00:00Z")
        ])]

        let changes = try await fixture.service.pull()

        #expect(changes.removedByServerIds.isEmpty)
    }

    /// A date from the server used to send a note straight past the bin.
    @Test func aBackdatedDeletionDoesntSkipTheBin() async throws {
        let fixture = try makeFixture()
        let record = try synced(fixture.repo)
        let note = try remoteNote(id: record.id, ciphertext: record.content, deleted: true, deletedAt: "2020-01-01T00:00:00Z")
        ScriptedServerProtocol.pullReplies = [page(notes: [note])]

        try await fixture.service.pull()
        let purged = try fixture.repo.purgeOldDeletedNotes()

        #expect(purged == 0)
        #expect(try fixture.repo.getRaw(id: record.id)?.deleted == true)
    }

    @Test func thePurgeUsesTheTimeTheNoteReachedTheBinHere() throws {
        let fixture = try makeFixture()
        let record = try synced(fixture.repo)
        try fixture.repo.softDelete(id: record.id)
        try fixture.repo.markSynced(id: record.id, syncedAt: Date().iso8601)

        #expect(try fixture.repo.purgeOldDeletedNotes(olderThan: 3600) == 0)
        #expect(try fixture.repo.purgeOldDeletedNotes(olderThan: -3600) == 1)
        #expect(try fixture.repo.getRaw(id: record.id) == nil)
    }

    @Test func thePurgeSkipsADeletionThatHasntBeenPushed() throws {
        let fixture = try makeFixture()
        let record = try synced(fixture.repo)
        try fixture.repo.softDelete(id: record.id)

        #expect(try fixture.repo.purgeOldDeletedNotes(olderThan: -3600) == 0)
    }

    /// An edit made elsewhere since the note was binned still has to win, so
    /// the deletion goes up with the time of the soft delete.
    @Test func aPurgedNoteIsDeletedEverywhereAsOfWhenItWasBinned() throws {
        let fixture = try makeFixture()
        let record = try synced(fixture.repo)
        try fixture.repo.softDelete(id: record.id)
        try fixture.repo.markSynced(id: record.id, syncedAt: Date().iso8601)
        let binnedAt = try #require(try fixture.repo.getRaw(id: record.id)?.deletedAt)

        try fixture.repo.purgeOldDeletedNotes(olderThan: -3600)

        let deletion = try #require(try fixture.syncRepo.getPendingDeletions().first)
        #expect(deletion.id == record.id)
        #expect(deletion.deletedAt == binnedAt)
    }

    // MARK: - One bad push doesn't stop the rest (jottery-cc81)

    @Test func aFailedPushStillPulls() async throws {
        let fixture = try makeFixture()
        _ = try fixture.repo.create(content: "stuck", key: key)
        let incoming = try remoteNote()
        ScriptedServerProtocol.pushHandler = { _ in .init(status: 500) }
        ScriptedServerProtocol.pullReplies = [page(notes: [incoming])]

        await #expect(throws: (any Error).self) { try await fixture.service.sync() }

        #expect(try fixture.repo.getRaw(id: incoming.id) != nil)
    }

    @Test func aNoteTheServerWontTakeDoesntHoldUpTheOthers() async throws {
        let fixture = try makeFixture()
        let tooBig = try fixture.repo.create(content: "far too big", key: key)
        let fine = try fixture.repo.create(content: "fine", key: key)
        ScriptedServerProtocol.pushHandler = { request in
            request.notes.contains { $0.id == tooBig.id }
                ? .init(status: 413)
                : ScriptedServerProtocol.acceptEverything(request)
        }

        await #expect(throws: SyncServiceError.notesTooLarge(count: 1)) {
            try await fixture.service.push()
        }

        #expect(try fixture.repo.getRaw(id: fine.id)?.needsSync == false)
        #expect(try fixture.repo.getRaw(id: tooBig.id)?.needsSync == true)
    }

    @Test func batchesStayUnderTheByteBudget() {
        let records = (0..<4).map { _ in NoteRecord.new(encryptedContent: "{}", encryptedTags: "{}") }

        let chunks = SyncService.chunked(records, size: 50, maxBytes: 100, bytes: { _ in 60 })

        #expect(chunks.map(\.count) == [1, 1, 1, 1])
        #expect(SyncService.chunked(records, size: 50, maxBytes: 130, bytes: { _ in 60 }).map(\.count) == [2, 2])
    }

    @Test func aBlobGoesUpOnceAndNotWithEveryEdit() async throws {
        let fixture = try makeFixture()
        var note = try fixture.repo.create(content: "with a file", key: key)
        let blobId = UUID().uuidString
        try fixture.attachmentRepo.storeBlob(id: blobId, mimeType: "text/plain", size: 1, data: Data("x".utf8))
        try fixture.repo.addAttachment(noteId: note.id, ref: AttachmentRef(
            id: blobId, filename: "name", mimeType: "text/plain", size: 1, data: blobId
        ))

        try await fixture.service.push()
        note.content = "edited"
        try fixture.repo.update(note, key: key)
        try await fixture.service.push()

        let sent = ScriptedServerProtocol.pushRequests.map(\.attachments.count)
        #expect(sent == [1, 0])
    }

    @Test func aPulledBlobIsNotSentBack() async throws {
        let fixture = try makeFixture()
        let blobId = UUID().uuidString
        var note = try remoteNote()
        note = SyncNote(
            id: note.id, createdAt: note.createdAt, modifiedAt: note.modifiedAt, content: note.content,
            tags: [], attachments: [AttachmentRef(id: blobId, filename: "name", mimeType: "text/plain", size: 1, data: blobId)],
            pinned: false, archived: false, deleted: false, version: 1
        )
        ScriptedServerProtocol.pullReplies = [page(
            notes: [note], attachments: [SyncAttachment(id: blobId, data: Data("x".utf8).base64EncodedString())]
        )]
        try await fixture.service.pull()

        #expect(try fixture.attachmentRepo.getBlob(id: blobId) != nil)
        #expect(try fixture.attachmentRepo.getBlobToUpload(id: blobId) == nil)
    }

    // MARK: - Status codes

    @Test func aRateLimitedSyncWaitsBeforeTryingAgain() async throws {
        let fixture = try makeFixture()
        _ = try fixture.repo.create(content: "a note", key: key)
        ScriptedServerProtocol.pushHandler = { _ in .init(status: 429, headers: ["Retry-After": "30"]) }
        ScriptedServerProtocol.pullReplies = [.init(status: 429, headers: ["Retry-After": "30"])]

        await #expect(throws: (any Error).self) { try await fixture.service.sync() }
        let requestsAfterFirstTry = ScriptedServerProtocol.pushRequests.count

        try await fixture.service.sync()

        #expect(ScriptedServerProtocol.pushRequests.count == requestsAfterFirstTry)
    }

    @Test func aDisabledAccountIsReportedAsThat() async throws {
        let fixture = try makeFixture()
        ScriptedServerProtocol.pullReplies = [.init(status: 403, body: Data(#"{"error":"Account deactivated"}"#.utf8))]

        await #expect {
            try await fixture.service.pull()
        } throws: { error in
            if case SyncClientError.accountDisabled = error { return true }
            return false
        }
    }

    // MARK: - Keep Server takes all of the server's copy (jottery-2xeq)

    private func conflict(on record: NoteRecord, archived: Bool?, locked: Bool?, color: String?) throws -> ConflictInfo {
        ConflictInfo(
            id: record.id,
            localContent: "local", localTags: [], localModifiedAt: record.modifiedAt, localAttachments: [],
            serverContent: "server", serverTags: [], serverModifiedAt: "2026-11-05T09:00:00Z",
            serverEncryptedContent: record.content, serverEncryptedTags: [],
            serverVersion: 7, serverAttachments: [], serverPinned: false,
            serverSyntaxLanguage: nil, serverWordWrap: nil, serverShowPreview: nil,
            serverContentHash: "server-hash", serverParentHash: nil, serverHashChain: ["server-hash"],
            serverArchived: archived, serverArchivedAt: archived == true ? "2026-11-05T09:00:00Z" : nil,
            serverLocked: locked, serverLockedAt: locked == true ? "2026-11-05T09:00:00Z" : nil,
            serverColor: color
        )
    }

    @Test(arguments: [ConflictResolutionStrategy.keepServer, .keepBoth])
    func resolvingForTheServerTakesItsFlags(strategy: ConflictResolutionStrategy) async throws {
        let fixture = try makeFixture()
        let record = try synced(fixture.repo)
        await fixture.service.addPendingConflictForTesting(
            try conflict(on: record, archived: true, locked: true, color: "red")
        )

        try await fixture.service.resolveConflict(noteId: record.id, strategy: strategy)

        let resolved = try #require(try fixture.repo.getRaw(id: record.id))
        #expect(resolved.archived)
        #expect(resolved.locked)
        #expect(resolved.color == "red")
    }

    @Test func anOlderServerThatSaysNothingLeavesTheFlagsAlone() async throws {
        let fixture = try makeFixture()
        var record = try synced(fixture.repo)
        record.color = "blue"
        record.locked = true
        try fixture.repo.updateRaw(record)
        await fixture.service.addPendingConflictForTesting(
            try conflict(on: record, archived: nil, locked: nil, color: nil)
        )

        try await fixture.service.resolveConflict(noteId: record.id, strategy: .keepServer)

        let resolved = try #require(try fixture.repo.getRaw(id: record.id))
        #expect(resolved.color == "blue")
        #expect(resolved.locked)
    }

    // MARK: - A note edited many times between syncs (R2-060)

    @Test func theServersHashStaysInTheChainHoweverManyEditsThereAre() throws {
        let fixture = try makeFixture()
        let record = try synced(fixture.repo)
        let serverHash = try #require(record.contentHash)
        var note = try #require(try fixture.repo.get(id: record.id, key: key))

        for edit in 0..<(CryptoService.maxHashChainLength + 10) {
            note.content = "edit \(edit)"
            try fixture.repo.update(note, key: key)
        }

        let chainJSON = try #require(try fixture.repo.getRaw(id: record.id)?.hashChain)
        let chain = try JSONDecoder().decode([String].self, from: Data(chainJSON.utf8))
        #expect(chain.contains(serverHash))
        #expect(chain.count <= CryptoService.maxHashChainLength)
    }

    // MARK: - Inbox notes keep their preview off (R2-069)

    @Test func theServerCantTurnOnThePreviewOfANoteFromTheInbox() async throws {
        let fixture = try makeFixture()
        var record = try synced(fixture.repo)
        record.fromInbox = true
        try fixture.repo.updateRaw(record)
        let note = try remoteNote(id: record.id, ciphertext: record.content, showPreview: true)
        ScriptedServerProtocol.pullReplies = [page(notes: [note])]

        try await fixture.service.pull()

        #expect(try fixture.repo.getRaw(id: record.id)?.showPreview == false)
    }
}
