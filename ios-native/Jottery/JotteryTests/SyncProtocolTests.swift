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
        // The longest wait allowed, so it can't run out while the test looks.
        ScriptedServerProtocol.pushHandler = { _ in .init(status: 429, headers: ["Retry-After": "3600"]) }
        ScriptedServerProtocol.pullReplies = [.init(status: 429, headers: ["Retry-After": "3600"])]

        await #expect(throws: (any Error).self) { try await fixture.service.sync() }
        let requestsAfterFirstTry = ScriptedServerProtocol.pushRequests.count

        try await fixture.service.sync()

        #expect(ScriptedServerProtocol.pushRequests.count == requestsAfterFirstTry)
    }

    @Test func aBusyServerIsLeftAloneForAsLongAsItAsks() async throws {
        let fixture = try makeFixture()
        _ = try fixture.repo.create(content: "a note", key: key)
        ScriptedServerProtocol.pushHandler = { _ in .init(status: 503, headers: ["Retry-After": "3600"]) }
        ScriptedServerProtocol.pullReplies = [.init(status: 503, headers: ["Retry-After": "3600"])]

        await #expect(throws: (any Error).self) { try await fixture.service.sync() }
        let requests = ScriptedServerProtocol.pushRequests.count

        try await fixture.service.sync()

        #expect(ScriptedServerProtocol.pushRequests.count == requests)
    }

    @Test func aTimeoutCanBeTriedAgainAtOnce() async throws {
        let fixture = try makeFixture()
        let note = try fixture.repo.create(content: "a note", key: key)
        ScriptedServerProtocol.pushHandler = { _ in .init(status: 408) }

        await #expect {
            try await fixture.service.push()
        } throws: { error in
            if case SyncClientError.requestTimedOut = error { return true }
            return false
        }
        #expect(try fixture.repo.getRaw(id: note.id)?.needsSync == true)

        // Sending the same push twice is safe, and nothing holds it back.
        ScriptedServerProtocol.pushHandler = { ScriptedServerProtocol.acceptEverything($0) }
        try await fixture.service.sync()

        #expect(try fixture.repo.getRaw(id: note.id)?.needsSync == false)
    }

    /// Deleting is how the user makes room, so it has to get through.
    @Test func atTheStorageQuotaOnlyDeletionsGoUpAndTheUserIsTold() async throws {
        let fixture = try makeFixture()
        let gone = try synced(fixture.repo)
        try fixture.repo.permanentlyDelete(id: gone.id)
        let waiting = try fixture.repo.create(content: "waiting for room", key: key)
        _ = try SavedSearchRepository(db: fixture.db).create(name: "mine", query: "#a", key: key)
        let incoming = try remoteNote()
        ScriptedServerProtocol.pullReplies = [page(notes: [incoming])]
        ScriptedServerProtocol.pushHandler = { request in
            guard request.notes.isEmpty, request.savedSearches == nil else {
                return .init(status: 507, body: Data(#"{"error":"x","code":"storage_quota_exceeded"}"#.utf8))
            }
            return ScriptedServerProtocol.acceptEverything(request)
        }

        await #expect {
            try await fixture.service.sync()
        } throws: { error in
            if case SyncClientError.storageQuotaExceeded = error { return true }
            return false
        }

        #expect(try fixture.syncRepo.getPendingDeletions().isEmpty)
        #expect(try fixture.repo.getRaw(id: waiting.id)?.needsSync == true)
        #expect(ScriptedServerProtocol.pushRequests.filter { $0.savedSearches != nil }.isEmpty)
        #expect(try fixture.repo.getRaw(id: incoming.id) != nil)  // and it still receives
    }

    @Test func timeoutsAndBusyServersAreRetriedThenLeft() {
        #expect(SyncRetry.delay(after: SyncClientError.requestTimedOut, attempt: 0) == 5)
        #expect(SyncRetry.delay(after: SyncClientError.requestTimedOut, attempt: 2) == 20)
        #expect(SyncRetry.delay(after: SyncClientError.serverBusy(retryAfter: 90), attempt: 0) == 90)
        #expect(SyncRetry.delay(after: SyncClientError.rateLimited(retryAfter: 1), attempt: 1) == 10)
        #expect(SyncRetry.delay(after: SyncClientError.requestTimedOut, attempt: SyncRetry.maxAttempts) == nil)
    }

    @Test func aFullAccountIsntRetriedByItself() {
        #expect(SyncRetry.delay(after: SyncClientError.storageQuotaExceeded, attempt: 0) == nil)
        #expect(SyncRetry.delay(after: SyncClientError.accountDisabled, attempt: 0) == nil)
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

    // MARK: - Version history tags, in both forms (R2-063)

    private func versionTags(of tags: [String], fixture: Fixture, noteId: String) async throws -> [String] {
        let version = SyncNoteVersion(
            versionKey: "\(noteId):1", noteId: noteId, version: 1,
            createdAt: "2026-11-05T09:00:00Z", syncedAt: "2026-11-05T09:00:00Z",
            content: try CryptoService.serializeEncryptedJSON(CryptoService.encryptText("old text", key: key)),
            tags: tags, attachments: [], reason: "sync"
        )
        let response = SyncPullResponse(notes: [], attachments: [], versions: [version])
        ScriptedServerProtocol.pullReplies = [.init(body: try JSONEncoder().encode(response))]
        try await fixture.service.pull()

        let stored = try #require(try fixture.versionRepo.getVersions(noteId: noteId).first)
        return try CryptoService.decryptStringArray(CryptoService.parseEncryptedJSON(stored.tags), key: key)
    }

    /// Web and Android write one blob holding the whole array.
    @Test func aVersionFromTheWebKeepsAllItsTags() async throws {
        let fixture = try makeFixture()
        let blob = try CryptoService.serializeEncryptedJSON(CryptoService.encryptStringArray(["a", "b"], key: key))

        #expect(try await versionTags(of: [blob], fixture: fixture, noteId: UUID().uuidString) == ["a", "b"])
    }

    /// iOS and the TUI write one ciphertext per tag, which is the canonical form.
    @Test func aVersionWithATagPerEntryKeepsAllItsTags() async throws {
        let fixture = try makeFixture()
        let tags = try ["a", "b"].map { try CryptoService.encryptTagForSync($0, key: key) }

        #expect(try await versionTags(of: tags, fixture: fixture, noteId: UUID().uuidString) == ["a", "b"])
    }

    /// Storing it would leave a raw value in the vault, and fail on every load.
    @Test func aVersionWhoseTagsWontDecryptIsDropped() async throws {
        let fixture = try makeFixture()
        let noteId = UUID().uuidString
        let other = SymmetricKey(data: Data(repeating: 9, count: 32))
        let foreign = try CryptoService.serializeEncryptedJSON(CryptoService.encryptStringArray(["x"], key: other))
        let version = SyncNoteVersion(
            versionKey: "\(noteId):1", noteId: noteId, version: 1,
            createdAt: "2026-11-05T09:00:00Z", syncedAt: "2026-11-05T09:00:00Z",
            content: foreign, tags: [foreign], attachments: [], reason: "sync"
        )
        let response = SyncPullResponse(notes: [], attachments: [], versions: [version])
        ScriptedServerProtocol.pullReplies = [.init(body: try JSONEncoder().encode(response))]

        try await fixture.service.pull()

        #expect(try fixture.versionRepo.getVersions(noteId: noteId).isEmpty)
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

    // MARK: - Notes the server skipped (checklist item 8)

    private func answer(
        skippedDeleted: [String]? = nil, errors: [String]? = nil
    ) -> ScriptedServerProtocol.Reply {
        var response = SyncPushResponse(accepted: [], rejected: [])
        response.skippedDeleted = skippedDeleted
        response.errors = errors
        return .init(body: (try? JSONEncoder().encode(response)) ?? Data())
    }

    @Test func aNoteDeletedElsewhereGoesToTheBinAndIsNotSentAgain() async throws {
        let fixture = try makeFixture()
        let note = try fixture.repo.create(content: "edited before it was deleted elsewhere", key: key)
        ScriptedServerProtocol.pushHandler = { self.answer(skippedDeleted: $0.notes.map(\.id)) }

        let changes = try await fixture.service.sync()

        let record = try #require(try fixture.repo.getRaw(id: note.id))
        #expect(record.deleted, "the bin, not gone: nothing the server says can delete a note outright")
        #expect(!record.needsSync)
        #expect(record.localDeletedAt != nil, "or the purge wouldn't know when it arrived")
        #expect(changes.deletedIds.contains(note.id))
        #expect(changes.removedByServerIds.contains(note.id), "it counts towards the warning about mass removal")

        let requests = ScriptedServerProtocol.pushRequests.count
        try await fixture.service.sync()
        #expect(ScriptedServerProtocol.pushRequests.count == requests)
    }

    @Test func aNoteEditedSinceItWasSentIsKeptAndSentAgain() async throws {
        let fixture = try makeFixture()
        let note = try fixture.repo.create(content: "first edit", key: key)
        let repo = fixture.repo
        ScriptedServerProtocol.pushHandler = { request in
            if var record = try? repo.getRaw(id: note.id) {
                record.modifiedAt = Date(timeIntervalSinceNow: 5).iso8601
                try? repo.updateRaw(record)
            }
            return self.answer(skippedDeleted: request.notes.map(\.id))
        }

        let changes = try await fixture.service.sync()

        let record = try #require(try fixture.repo.getRaw(id: note.id))
        #expect(!record.deleted, "this edit is later than the deletion, so it brings the note back")
        #expect(record.needsSync)
        #expect(!changes.removedByServerIds.contains(note.id))
    }

    /// An answer is the server's word on a note, whatever else the response names.
    @Test func aNoteTheServerAnsweredForIsntBinnedForBeingNamedAsWell() async throws {
        let fixture = try makeFixture()
        let note = try fixture.repo.create(content: "accepted and named", key: key)
        ScriptedServerProtocol.pushHandler = { request in
            var response = SyncPushResponse(accepted: request.notes.map { SyncAccepted(id: $0.id) }, rejected: [])
            response.skippedDeleted = request.notes.map(\.id)
            return .init(body: (try? JSONEncoder().encode(response)) ?? Data())
        }

        let changes = try await fixture.service.sync()

        #expect(try #require(try fixture.repo.getRaw(id: note.id)).deleted == false)
        #expect(!changes.removedByServerIds.contains(note.id))
    }

    /// Every batch after the first would get the same answer.
    @Test func theQuotaStopsTheBatchesStillToGo() async throws {
        let fixture = try makeFixture()
        for index in 0...SyncService.pushChunkSize {
            _ = try fixture.repo.create(content: "note \(index)", key: key)
        }
        ScriptedServerProtocol.pushHandler = { _ in
            .init(status: 507, body: Data(#"{"error":"x","code":"storage_quota_exceeded"}"#.utf8))
        }

        await #expect {
            try await fixture.service.sync()
        } throws: { error in
            if case SyncClientError.storageQuotaExceeded = error { return true }
            return false
        }

        #expect(ScriptedServerProtocol.pushRequests.count == 1)
    }

    /// With nothing scheduled, the device would stay quiet until the next edit.
    @Test @MainActor func aBusyServerIsAskedAgainAfterTheWaitItNamed() async throws {
        ScriptedServerProtocol.reset()
        let state = try TestVaults.makeState()
        state.keyManager.unlockWithKeyData(Data(repeating: 7, count: 32))
        state.isLocked = false
        _ = try state.createNote(content: "waiting to go up")
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [ScriptedServerProtocol.self]
        state.syncService = SyncService(
            syncClient: SyncClient(endpoint: "https://sync.example.test", apiKey: "device-key", configuration: configuration),
            noteRepo: try #require(state.noteRepo),
            syncRepo: try #require(state.syncRepo),
            versionRepo: try #require(state.versionRepo),
            attachmentRepo: try #require(state.attachmentRepo),
            savedSearchRepo: try #require(state.savedSearchRepo),
            key: try #require(state.keyManager.masterKey)
        )
        state.syncEnabled = true
        let pauses = PauseLog()
        state.autoSyncPause = { await pauses.record($0) }
        ScriptedServerProtocol.pushHandler = { _ in .init(status: 503, headers: ["Retry-After": "90"]) }

        await state.triggerSync()

        await waitUntil { await !pauses.pauses.isEmpty }
        #expect(await pauses.pauses.first == .seconds(90))
        state.teardownSync()
    }

    /// A hostile or broken server doesn't get to silence a device for days.
    @Test func aWaitOverAnHourIsCutToAnHour() async throws {
        let fixture = try makeFixture()
        _ = try fixture.repo.create(content: "a note", key: key)
        ScriptedServerProtocol.pushHandler = { _ in .init(status: 503, headers: ["Retry-After": "999999"]) }

        await #expect {
            try await fixture.service.sync()
        } throws: { error in
            if case SyncClientError.serverBusy(let wait) = error { return wait == 3600 }
            return false
        }
    }

    @Test func theServerCantSkipANoteItWasntSent() async throws {
        let fixture = try makeFixture()
        let bystander = try synced(fixture.repo)
        _ = try fixture.repo.create(content: "the only note pushed", key: key)
        ScriptedServerProtocol.pushHandler = { _ in self.answer(skippedDeleted: [bystander.id]) }

        try await fixture.service.sync()

        #expect(try #require(try fixture.repo.getRaw(id: bystander.id)).deleted == false)
    }

    @Test func aNoteInConflictIsLeftForTheUserToDecide() async throws {
        let fixture = try makeFixture()
        let record = try synced(fixture.repo)
        var edited = record
        edited.needsSync = true
        try fixture.repo.updateRaw(edited)
        await fixture.service.addPendingConflictForTesting(try conflict(on: record, archived: nil, locked: nil, color: nil))
        ScriptedServerProtocol.pushHandler = { self.answer(skippedDeleted: $0.notes.map(\.id)) }

        try await fixture.service.sync()

        #expect(try #require(try fixture.repo.getRaw(id: record.id)).deleted == false)
    }

    @Test func aNoteAlreadyInTheBinIsNoLongerSent() async throws {
        let fixture = try makeFixture()
        let note = try fixture.repo.create(content: "binned here, deleted for good elsewhere", key: key)
        try fixture.repo.softDelete(id: note.id)
        ScriptedServerProtocol.pushHandler = { self.answer(skippedDeleted: $0.notes.map(\.id)) }

        let changes = try await fixture.service.sync()

        let record = try #require(try fixture.repo.getRaw(id: note.id))
        #expect(record.deleted)
        #expect(!record.needsSync)
        #expect(!changes.removedByServerIds.contains(note.id), "it wasn't in use")
    }

    // MARK: - A server that keeps deleting (R2-069, R2-168)

    /// The web's hold had this hole: a note held in the bin counted as not live, so
    /// the same deletions sent again deleted it from there. Nothing a server says
    /// may take a note out of the bin, however often it says it.
    @Test func aRepeatedDeletionCantTakeANoteOutOfTheBin() async throws {
        let fixture = try makeFixture()
        let records = try (0..<12).map { _ in try synced(fixture.repo) }
        let deletions = records.map { SyncDeletion(id: $0.id, deletedAt: "2000-01-01T00:00:00Z") }
        ScriptedServerProtocol.pullReplies = (0..<4).map { _ in page(deletions: deletions) }

        let first = try await fixture.service.pull()
        #expect(first.removedByServerIds.count == 12)
        let binnedAt = try records.map { try fixture.repo.getRaw(id: $0.id)?.localDeletedAt }

        for _ in 0..<3 {
            let again = try await fixture.service.pull()
            #expect(again.removedByServerIds.isEmpty, "already in the bin, so not removed again")
        }

        for (record, time) in zip(records, binnedAt) {
            let stored = try #require(try fixture.repo.getRaw(id: record.id))
            #expect(stored.deleted)
            #expect(stored.localDeletedAt == time, "repeating it can't move the purge")
        }
        #expect(try fixture.repo.purgeOldDeletedNotes() == 0)
        #expect(try records.allSatisfy { try fixture.repo.getRaw(id: $0.id) != nil })
    }

    /// Every way a server can put a note in the bin, with the oldest date it can.
    @Test func aDeletionDatedLongAgoNeverPurgesSooner() async throws {
        let fixture = try makeFixture()
        let tombstoned = try synced(fixture.repo)
        let pulled = try synced(fixture.repo)
        let skipped = try fixture.repo.create(content: "edited, then deleted elsewhere", key: key)
        let longAgo = "2000-01-01T00:00:00Z"
        ScriptedServerProtocol.pushHandler = { self.answer(skippedDeleted: $0.notes.map(\.id)) }
        ScriptedServerProtocol.pullReplies = [page(
            notes: [try remoteNote(id: pulled.id, ciphertext: pulled.content, deleted: true, deletedAt: longAgo)],
            deletions: [SyncDeletion(id: tombstoned.id, deletedAt: longAgo)]
        )]

        try await fixture.service.sync()

        #expect(try fixture.repo.purgeOldDeletedNotes() == 0)
        for id in [tombstoned.id, pulled.id, skipped.id] {
            #expect(try fixture.repo.getRaw(id: id)?.deleted == true)
        }
    }

    @Test func keepServerWithADateLongAgoDoesntPurgeSooner() async throws {
        let fixture = try makeFixture()
        let note = try fixture.repo.create(content: "kept in the bin, for now", key: key)
        refuse(deleted: true, deletedAt: "2000-01-01T00:00:00Z")
        try await fixture.service.sync()

        try await fixture.service.resolveConflict(noteId: note.id, strategy: .keepServer)

        #expect(try fixture.repo.purgeOldDeletedNotes() == 0)
        #expect(try fixture.repo.getRaw(id: note.id) != nil)
    }

    // MARK: - A server copy that's in the bin

    private func rejection(of note: SyncNote, deleted: Bool? = nil, deletedAt: String? = nil) -> SyncRejected {
        SyncRejected(
            id: note.id, reason: "conflict", serverModifiedAt: "2026-11-05T09:00:00Z",
            serverContent: note.content, serverTags: [], serverVersion: 9, serverAttachments: [],
            serverPinned: false, serverDeleted: deleted, serverDeletedAt: deletedAt,
            serverContentHash: "server-hash", serverHashChain: ["server-hash"]
        )
    }

    private func refuse(deleted: Bool?, deletedAt: String? = nil) {
        ScriptedServerProtocol.pushHandler = { request in
            let rejected = request.notes.map { self.rejection(of: $0, deleted: deleted, deletedAt: deletedAt) }
            let response = SyncPushResponse(accepted: [], rejected: rejected)
            return .init(body: (try? JSONEncoder().encode(response)) ?? Data())
        }
    }

    /// Later pulls skip a note whose hash matches, so taking the server's copy
    /// without its place in the bin would leave the two different for good.
    @Test func keepServerTakesTheServersBin() async throws {
        let fixture = try makeFixture()
        let note = try fixture.repo.create(content: "edited while it sat in the bin elsewhere", key: key)
        refuse(deleted: true, deletedAt: "2026-11-05T09:00:00Z")

        try await fixture.service.sync()
        try await fixture.service.resolveConflict(noteId: note.id, strategy: .keepServer)

        let record = try #require(try fixture.repo.getRaw(id: note.id))
        #expect(record.deleted)
        #expect(record.deletedAt == "2026-11-05T09:00:00Z")
        #expect(record.localDeletedAt != nil, "or the purge wouldn't know when it arrived")
    }

    @Test func anOlderServerThatDoesntSayLeavesTheNoteWhereItIs() async throws {
        let fixture = try makeFixture()
        let note = try fixture.repo.create(content: "conflict with an older server", key: key)
        refuse(deleted: nil)

        try await fixture.service.sync()
        try await fixture.service.resolveConflict(noteId: note.id, strategy: .keepServer)

        #expect(try #require(try fixture.repo.getRaw(id: note.id)).deleted == false)
    }

    @Test func keepServerBringsANoteOutOfTheBinIfTheServersCopyIsLive() async throws {
        let fixture = try makeFixture()
        let note = try fixture.repo.create(content: "binned here, live there", key: key)
        try fixture.repo.softDelete(id: note.id)
        refuse(deleted: false)

        try await fixture.service.sync()
        try await fixture.service.resolveConflict(noteId: note.id, strategy: .keepServer)

        let record = try #require(try fixture.repo.getRaw(id: note.id))
        #expect(!record.deleted)
        #expect(record.deletedAt == nil)
        #expect(record.localDeletedAt == nil)
    }

    // MARK: - A clock that's badly wrong

    /// Well before any note made by these tests.
    private let serverLongAgo = "2020-01-01T00:00:00Z"

    @Test func aNoteDatedFarAheadOfTheServerWarnsAboutTheClock() async throws {
        let fixture = try makeFixture()
        let note = try fixture.repo.create(content: "dated by a clock years ahead", key: key)
        ScriptedServerProtocol.pushHandler = { _ in self.answer(errors: ["note: modifiedAt is more than 24 hours ahead"]) }
        ScriptedServerProtocol.pullReplies = [page(syncedAt: serverLongAgo)]

        let changes = try await fixture.service.sync()

        #expect(changes.clockLooksWrong)
        #expect(try #require(try fixture.repo.getRaw(id: note.id)).needsSync, "it goes up once the clock is right")
    }

    /// The server's clock is the judge: the device's own is the one in doubt.
    @Test func aNoteWithinADayOfTheServerIsNotBlamedOnTheClock() async throws {
        let fixture = try makeFixture()
        _ = try fixture.repo.create(content: "skipped for some other reason", key: key)
        ScriptedServerProtocol.pushHandler = { _ in self.answer(errors: ["something else"]) }
        ScriptedServerProtocol.pullReplies = [page(syncedAt: Date(timeIntervalSinceNow: -3_600).iso8601)]

        let changes = try await fixture.service.sync()

        #expect(!changes.clockLooksWrong)
    }

    @Test func aNoteTheServerAnsweredForIsNotBlamedOnTheClock() async throws {
        let fixture = try makeFixture()
        _ = try fixture.repo.create(content: "accepted, whatever its date", key: key)
        ScriptedServerProtocol.pullReplies = [page(syncedAt: serverLongAgo)]

        let changes = try await fixture.service.sync()

        #expect(!changes.clockLooksWrong)
    }

    @Test func aNoteSkippedAsDeletedIsNotBlamedOnTheClock() async throws {
        let fixture = try makeFixture()
        _ = try fixture.repo.create(content: "deleted elsewhere", key: key)
        ScriptedServerProtocol.pushHandler = { self.answer(skippedDeleted: $0.notes.map(\.id)) }
        ScriptedServerProtocol.pullReplies = [page(syncedAt: serverLongAgo)]

        let changes = try await fixture.service.sync()

        #expect(!changes.clockLooksWrong)
    }

    @Test func aPullThatGivesNoServerTimeWarnsOfNothing() async throws {
        let fixture = try makeFixture()
        _ = try fixture.repo.create(content: "no time to judge it by", key: key)
        ScriptedServerProtocol.pushHandler = { _ in self.answer(errors: ["skipped"]) }
        ScriptedServerProtocol.pullReplies = [page(syncedAt: nil)]

        let changes = try await fixture.service.sync()

        #expect(!changes.clockLooksWrong)
    }

    /// A search is all that request holds, so an error means one was skipped.
    /// Nothing says which, and sending them all again is harmless.
    @Test func aSavedSearchTheServerSkippedStaysQueuedAndWarnsAboutTheClock() async throws {
        let fixture = try makeFixture()
        let searches = SavedSearchRepository(db: fixture.db)
        let search = try searches.create(name: "Dated ahead", query: "#clock", key: key)
        ScriptedServerProtocol.pushHandler = { _ in self.answer(errors: ["search: modifiedAt is more than 24 hours ahead"]) }
        ScriptedServerProtocol.pullReplies = [page(syncedAt: serverLongAgo)]

        let changes = try await fixture.service.sync()

        #expect(changes.clockLooksWrong)
        #expect(try searches.listNeedingSync().map(\.id) == [search.id])
    }

    // MARK: - Pulling a note with a conflict waiting (web wave 2)

    /// A conflict is an unsent edit: this copy may be the only one of it.
    @Test func aPullLeavesTheLocalSideOfAConflictAlone() async throws {
        let fixture = try makeFixture()
        let record = try synced(fixture.repo)
        await fixture.service.addPendingConflictForTesting(try conflict(on: record, archived: nil, locked: nil, color: nil))
        ScriptedServerProtocol.pullReplies = [page(notes: [
            try remoteNote(id: record.id, content: "newer on the server", contentHash: "newer-hash"),
        ])]

        try await fixture.service.pull()

        #expect(try #require(try fixture.repo.getRaw(id: record.id)).content == record.content)
    }

    /// The push's refusal leaves the edit queued, so a restart, which forgets
    /// conflicts held in memory, finds it again instead of overwriting it.
    @Test func aRefusedPushStaysAnUnsentEdit() async throws {
        let fixture = try makeFixture()
        let note = try fixture.repo.create(content: "refused by the server", key: key)
        ScriptedServerProtocol.pushHandler = { request in
            let response = SyncPushResponse(accepted: [], rejected: request.notes.map { self.rejection(of: $0) })
            return .init(body: (try? JSONEncoder().encode(response)) ?? Data())
        }

        try await fixture.service.sync()

        #expect(try #require(try fixture.repo.getRaw(id: note.id)).needsSync)
    }

    /// Keep Server has to take the server as it is now. The cursor has moved past
    /// this note, so a copy passed over here would never arrive again.
    @Test func aConflictShowsTheNewestServerCopy() async throws {
        let fixture = try makeFixture()
        let record = try synced(fixture.repo)
        var edited = record
        edited.needsSync = true
        try fixture.repo.updateRaw(edited)
        await fixture.service.addPendingConflictForTesting(try conflict(on: record, archived: nil, locked: nil, color: nil))
        ScriptedServerProtocol.pullReplies = [page(notes: [
            try remoteNote(id: record.id, content: "newest on the server", contentHash: "newest-hash"),
        ])]

        try await fixture.service.pull()

        let waiting = await fixture.service.pendingConflicts
        #expect(waiting.count == 1)
        #expect(waiting.first?.serverContent == "newest on the server")
        #expect(waiting.first?.serverContentHash == "newest-hash")

        try await fixture.service.resolveConflict(noteId: record.id, strategy: .keepServer)

        let resolved = try #require(try fixture.repo.get(id: record.id, key: key))
        #expect(resolved.content == "newest on the server")
    }

    // MARK: - A side of a conflict that won't decrypt (R2-071)

    /// Text the server could send, encrypted with some other key.
    private func undecryptable() throws -> String {
        let other = SymmetricKey(data: Data(repeating: 9, count: 32))
        return try CryptoService.serializeEncryptedJSON(CryptoService.encryptText("not for this key", key: other))
    }

    private func refuseWith(serverContent: String) {
        ScriptedServerProtocol.pushHandler = { request in
            let rejected = request.notes.map { note in
                SyncRejected(
                    id: note.id, reason: "conflict", serverModifiedAt: "2026-11-05T09:00:00Z",
                    serverContent: serverContent, serverTags: [], serverVersion: 9,
                    serverAttachments: [], serverPinned: false,
                    serverContentHash: "server-hash", serverHashChain: ["server-hash"]
                )
            }
            return .init(body: (try? JSONEncoder().encode(SyncPushResponse(accepted: [], rejected: rejected))) ?? Data())
        }
    }

    @Test(arguments: [ConflictResolutionStrategy.keepServer, .keepBoth])
    func aServerCopyThatWontDecryptCantBeTaken(strategy: ConflictResolutionStrategy) async throws {
        let fixture = try makeFixture()
        let note = try fixture.repo.create(content: "the only readable copy", key: key)
        refuseWith(serverContent: try undecryptable())

        try await fixture.service.sync()

        let waiting = try #require(await fixture.service.pendingConflicts.first)
        #expect(waiting.serverUnreadable)
        await #expect(throws: ConflictResolutionError.unreadable) {
            try await fixture.service.resolveConflict(noteId: note.id, strategy: strategy)
        }

        let kept = try #require(try fixture.repo.get(id: note.id, key: key))
        #expect(kept.content == "the only readable copy")
        #expect(await fixture.service.pendingConflicts.count == 1, "the conflict stays until it can be settled")
    }

    @Test func plaintextFromTheServerIsNotTakenForANote() async throws {
        let fixture = try makeFixture()
        let note = try fixture.repo.create(content: "the only readable copy", key: key)
        refuseWith(serverContent: "plaintext chosen by the server")

        try await fixture.service.sync()

        let waiting = try #require(await fixture.service.pendingConflicts.first)
        #expect(waiting.serverUnreadable)
        #expect(waiting.serverContent.isEmpty, "never shown as if it were the server's version")
        await #expect(throws: ConflictResolutionError.unreadable) {
            try await fixture.service.resolveConflict(noteId: note.id, strategy: .keepBoth)
        }
    }

    @Test func aServerCopyThatWontDecryptCanStillBeOverruled() async throws {
        let fixture = try makeFixture()
        let note = try fixture.repo.create(content: "kept over a copy nobody can read", key: key)
        refuseWith(serverContent: try undecryptable())
        try await fixture.service.sync()

        try await fixture.service.resolveConflict(noteId: note.id, strategy: .keepLocal)

        let record = try #require(try fixture.repo.getRaw(id: note.id))
        #expect(record.needsSync)
        #expect(await fixture.service.pendingConflicts.isEmpty)
    }

    /// Keep Both copies the local side, so a local side that can't be read would
    /// become an empty note while the original is overwritten.
    @Test func aLocalCopyThatWontDecryptCantBeKeptAsWell() async throws {
        let fixture = try makeFixture()
        let note = try fixture.repo.create(content: "soon unreadable", key: key)
        var record = try #require(try fixture.repo.getRaw(id: note.id))
        record.content = try undecryptable()
        try fixture.repo.updateRaw(record)
        let serverCopy = try CryptoService.serializeEncryptedJSON(CryptoService.encryptText("the server's copy", key: key))
        refuseWith(serverContent: serverCopy)

        try await fixture.service.sync()

        let waiting = try #require(await fixture.service.pendingConflicts.first)
        #expect(waiting.localUnreadable)
        await #expect(throws: ConflictResolutionError.unreadable) {
            try await fixture.service.resolveConflict(noteId: note.id, strategy: .keepBoth)
        }
        #expect(try fixture.repo.getRaw(id: note.id)?.content == record.content)

        try await fixture.service.resolveConflict(noteId: note.id, strategy: .keepServer)
        #expect(try fixture.repo.get(id: note.id, key: key)?.content == "the server's copy")
    }

    /// Keep Both writes two notes. Stopping between them must not leave the
    /// original overwritten with the local side nowhere.
    @Test func keepBothWritesBothNotesOrNeither() async throws {
        let fixture = try makeFixture()
        let note = try fixture.repo.create(content: "the local side", key: key)
        let serverCopy = try CryptoService.serializeEncryptedJSON(CryptoService.encryptText("the server's copy", key: key))
        refuseWith(serverContent: serverCopy)
        try await fixture.service.sync()
        // Fails the second of the two writes: the duplicate is a new row.
        try await fixture.db.dbPool.write { db in
            try db.execute(sql: """
                CREATE TRIGGER no_new_notes BEFORE INSERT ON notes
                WHEN NOT EXISTS (SELECT 1 FROM notes WHERE id = NEW.id)
                BEGIN SELECT RAISE(ABORT, 'no new notes'); END
            """)
        }

        await #expect(throws: (any Error).self) {
            try await fixture.service.resolveConflict(noteId: note.id, strategy: .keepBoth)
        }

        #expect(try fixture.repo.get(id: note.id, key: key)?.content == "the local side")
        #expect(await fixture.service.pendingConflicts.count == 1)
    }

    // MARK: - Version keys collide across devices (R2-076)

    /// A key is only unique per client, so a pulled version with the same key can
    /// be someone else's revision. This device's own must survive that.
    @Test func aPulledVersionNeverReplacesOneThisDeviceHolds() async throws {
        let fixture = try makeFixture()
        let record = try synced(fixture.repo)
        try fixture.versionRepo.createVersion(from: record, reason: "manual")
        let mine = try #require(try fixture.versionRepo.getVersions(noteId: record.id).first)
        let response = SyncPullResponse(
            notes: [], deletions: nil, attachments: [],
            versions: [SyncNoteVersion(
                versionKey: mine.versionKey, noteId: record.id, version: mine.version,
                createdAt: mine.createdAt, syncedAt: "2026-11-05T09:00:00Z",
                content: "a different device's revision", tags: [], attachments: [],
                syntaxLanguage: "markdown", wordWrap: true, showPreview: false, reason: "sync"
            )],
            syncedAt: "2026-11-05T09:23:45Z", hasMore: false, nextCursor: nil
        )
        ScriptedServerProtocol.pullReplies = [.init(body: try JSONEncoder().encode(response))]

        try await fixture.service.pull()

        let kept = try #require(try fixture.versionRepo.getVersions(noteId: record.id).first)
        #expect(kept.content == mine.content)
    }

    // MARK: - A sync that didn't run (jottery-bp2j)

    @Test func aSyncThatDidntRunSaysSo() async throws {
        let fixture = try makeFixture()
        ScriptedServerProtocol.pushHandler = { _ in .init(status: 429, body: Data(#"{"retryAfter":600}"#.utf8)) }
        _ = try fixture.repo.create(content: "waiting", key: key)
        await #expect(throws: (any Error).self) { try await fixture.service.sync() }

        let changes = try await fixture.service.sync()

        #expect(changes.wasSkipped)
    }

    @Test func aSyncThatRanSaysSoEvenWithNothingToDo() async throws {
        let fixture = try makeFixture()

        let changes = try await fixture.service.sync()

        #expect(!changes.wasSkipped)
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
