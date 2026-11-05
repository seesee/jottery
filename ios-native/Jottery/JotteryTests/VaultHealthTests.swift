import CryptoKit
import Foundation
import GRDB
import Testing

@testable import Jottery

/// A server that answers by route, so that each attachment can behave differently.
private final class HealthServer: URLProtocol, @unchecked Sendable {
    struct Reply {
        var status: Int
        var body: Data = Data()
    }

    nonisolated(unsafe) private static var replies: [String: Reply] = [:]
    nonisolated(unsafe) private static var requests: [(method: String, path: String, body: Data)] = []
    private static let lock = NSLock()

    static func reset() {
        lock.withLock {
            replies = [:]
            requests = []
        }
    }

    static func reply(_ method: String, _ path: String, status: Int, body: Data = Data()) {
        lock.withLock { replies["\(method) \(path)"] = Reply(status: status, body: body) }
    }

    static var requested: [(method: String, path: String, body: Data)] {
        lock.withLock { requests }
    }

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        let method = request.httpMethod ?? "?"
        let path = request.url?.path ?? "?"
        let body = request.httpBody ?? request.httpBodyStream.map(Self.read) ?? Data()
        let reply = Self.lock.withLock {
            Self.requests.append((method, path, body))
            return Self.replies["\(method) \(path)"] ?? Reply(status: 404)
        }
        let response = HTTPURLResponse(
            url: request.url!, statusCode: reply.status, httpVersion: "HTTP/1.1", headerFields: nil
        )!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: reply.body)
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}

    private static func read(_ stream: InputStream) -> Data {
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

private let blobPath = "/api/v1/sync/attachments/"

/// The vault health scan and what it can repair (R2-143, jottery-m3wg).
@MainActor
@Suite(.serialized)
struct VaultHealthTests {

    private func makeVault() async throws -> AppState {
        let state = try TestVaults.makeState()
        try await state.createVault(password: "vault-health-\(UUID().uuidString)")
        return state
    }

    private func makeClient() -> SyncClient {
        HealthServer.reset()
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [HealthServer.self]
        return SyncClient(
            endpoint: "https://notes.example.org",
            apiKey: "api-\(UUID().uuidString)",
            configuration: configuration
        )
    }

    private func scan(_ state: AppState) throws -> VaultHealthReport {
        try VaultHealthService.scan(
            noteRepo: #require(state.noteRepo),
            attachmentRepo: #require(state.attachmentRepo),
            key: #require(state.keyManager.masterKey)
        )
    }

    /// A note with an attachment, whose blob is then taken away if `keepBlob` is false.
    private func noteWithAttachment(
        in state: AppState,
        content: String = "A note with a file",
        keepBlob: Bool
    ) throws -> (note: DecryptedNote, ref: AttachmentRef) {
        let note = try #require(try state.createNote(content: content))
        try state.addAttachment(
            to: note.id, data: Data("file contents".utf8), filename: "hello.txt", mimeType: "text/plain"
        )
        let key = try #require(state.keyManager.masterKey)
        let stored = try #require(try state.noteRepo?.get(id: note.id, key: key))
        let ref = try #require(stored.attachments.first)
        if !keepBlob { try state.attachmentRepo?.deleteBlob(id: ref.data) }
        return (stored, ref)
    }

    private func markSynced(_ state: AppState, _ id: String) throws {
        try TestVaults.execute(state, "UPDATE notes SET needs_sync = 0 WHERE id = ?", [id])
    }

    private func serverBlob(_ plaintext: String, key: SymmetricKey) throws -> Data {
        let encrypted = try CryptoService.encrypt(Data(plaintext.utf8), key: key)
        return Data(try CryptoService.serializeEncryptedJSON(encrypted).utf8)
    }

    private func jsonBody(id: String, data: String) -> Data {
        Data(#"{"id":"\#(id)","data":"\#(data)"}"#.utf8)
    }

    // MARK: - Scanning

    @Test func aHealthyVaultReportsNothing() async throws {
        let state = try await makeVault()
        _ = try state.createNote(content: "Fine")
        _ = try noteWithAttachment(in: state, keepBlob: true)

        let report = try scan(state)

        #expect(report.notesChecked == 2)
        #expect(report.isHealthy)
    }

    @Test func aNoteUnderAnotherKeyIsReportedWithoutItsContent() async throws {
        let state = try await makeVault()
        let note = try #require(try state.createNote(content: "Fine"))
        let foreign = try TestVaults.foreignCiphertext()
        try TestVaults.execute(state, "UPDATE notes SET content = ? WHERE id = ?", [foreign, note.id])

        let report = try scan(state)

        #expect(report.unreadableNotes.map(\.id) == [note.id])
        #expect(!report.isHealthy)
    }

    /// D3: stored plaintext that was never encrypted is unreadable too. It's
    /// not a note that happens to decrypt to itself.
    @Test func plaintextWhereCiphertextBelongsIsUnreadable() async throws {
        let state = try await makeVault()
        let note = try #require(try state.createNote(content: "Fine"))
        try TestVaults.execute(state, "UPDATE notes SET content = 'not encrypted at all' WHERE id = ?", [note.id])

        #expect(try scan(state).unreadableNotes.map(\.id) == [note.id])
    }

    @Test func unreadableTagsMakeTheNoteUnreadable() async throws {
        let state = try await makeVault()
        let note = try #require(try state.createNote(content: "Fine", tags: ["a"]))
        let foreign = try TestVaults.foreignCiphertext()
        try TestVaults.execute(state, "UPDATE notes SET tags = ? WHERE id = ?", [foreign, note.id])

        #expect(try scan(state).unreadableNotes.map(\.id) == [note.id])
    }

    /// The list and the bin both skip what they can't read, so a note in the
    /// bin or the archive can't be reached any other way.
    @Test func notesInTheBinAndArchiveAreScannedToo() async throws {
        let state = try await makeVault()
        let foreign = try TestVaults.foreignCiphertext()
        let binned = try #require(try state.createNote(content: "Binned"))
        let archived = try #require(try state.createNote(content: "Archived"))
        try state.deleteNote(id: binned.id)
        try state.archiveNote(id: archived.id)
        try TestVaults.execute(state, "UPDATE notes SET content = ? WHERE id IN (?, ?)", [foreign, binned.id, archived.id])

        let report = try scan(state)

        #expect(Set(report.unreadableNotes.map(\.id)) == [binned.id, archived.id])
        #expect(report.unreadableNotes.first { $0.id == binned.id }?.inBin == true)
        #expect(report.unreadableNotes.first { $0.id == archived.id }?.inBin == false)
    }

    @Test func aMissingBlobIsReportedWithItsNoteAndFilename() async throws {
        let state = try await makeVault()
        let (note, ref) = try noteWithAttachment(in: state, content: "My holiday\nphotos", keepBlob: false)

        let missing = try #require(try scan(state).missingAttachments.first)

        #expect(missing.attachmentId == ref.id)
        #expect(missing.noteId == note.id)
        #expect(missing.noteTitle == "My holiday")
        #expect(missing.filename == "hello.txt")
    }

    /// Unreadable means no title, and never the ciphertext where a title would be.
    @Test func aMissingBlobOnAnUnreadableNoteHasNoTitle() async throws {
        let state = try await makeVault()
        let (note, _) = try noteWithAttachment(in: state, keepBlob: false)
        let foreign = try TestVaults.foreignCiphertext()
        try TestVaults.execute(state, "UPDATE notes SET content = ? WHERE id = ?", [foreign, note.id])

        let report = try scan(state)

        #expect(report.missingAttachments.count == 1)
        #expect(report.missingAttachments.first?.noteTitle == nil)
        #expect(report.unreadableNotes.map(\.id) == [note.id])
    }

    @Test func aNoteInTheBinDoesntCountForMissingBlobs() async throws {
        let state = try await makeVault()
        let (note, _) = try noteWithAttachment(in: state, keepBlob: false)
        try state.deleteNote(id: note.id)

        #expect(try scan(state).missingAttachments.isEmpty)
    }

    @Test func anAttachmentSharedByTwoNotesIsReportedOnce() async throws {
        let state = try await makeVault()
        let (_, ref) = try noteWithAttachment(in: state, keepBlob: false)
        let other = try #require(try state.createNote(content: "Shares the file"))
        try state.noteRepo?.addAttachment(noteId: other.id, ref: ref)

        #expect(try scan(state).missingAttachments.count == 1)
    }

    @Test func aScanStopsWhenCancelled() async throws {
        let state = try await makeVault()
        _ = try state.createNote(content: "Fine")
        let noteRepo = try #require(state.noteRepo)
        let attachmentRepo = try #require(state.attachmentRepo)
        let key = try #require(state.keyManager.masterKey)

        let task = Task.detached {
            withUnsafeCurrentTask { $0?.cancel() }
            return try VaultHealthService.scan(noteRepo: noteRepo, attachmentRepo: attachmentRepo, key: key)
        }

        await #expect(throws: CancellationError.self) { try await task.value }
    }

    // MARK: - Repairing from the server

    private func repair(_ state: AppState, client: SyncClient) async throws -> (VaultHealthReport.MissingAttachment, VaultHealthService.RepairResult) {
        let missing = try #require(try scan(state).missingAttachments.first)
        let result = await VaultHealthService.repair(
            missing,
            client: client,
            attachmentRepo: try #require(state.attachmentRepo),
            key: try #require(state.keyManager.masterKey)
        )
        return (missing, result)
    }

    @Test func aBlobFromTheServerIsStoredAndOpens() async throws {
        let state = try await makeVault()
        let key = try #require(state.keyManager.masterKey)
        let (_, ref) = try noteWithAttachment(in: state, keepBlob: false)
        let client = makeClient()
        let blob = try serverBlob("file contents", key: key)
        HealthServer.reply("GET", blobPath + ref.id, status: 200, body: jsonBody(id: ref.id, data: blob.base64EncodedString()))

        let (_, result) = try await repair(state, client: client)

        #expect(result == .repaired)
        #expect(try state.attachmentRepo?.getBlob(id: ref.data) == blob)
        #expect(try scan(state).isHealthy)
    }

    /// It came from the server, so there's no call to send it back.
    @Test func aRepairedBlobIsMarkedAsOnTheServer() async throws {
        let state = try await makeVault()
        let key = try #require(state.keyManager.masterKey)
        let (_, ref) = try noteWithAttachment(in: state, keepBlob: false)
        let client = makeClient()
        let blob = try serverBlob("file contents", key: key)
        HealthServer.reply("GET", blobPath + ref.id, status: 200, body: jsonBody(id: ref.id, data: blob.base64EncodedString()))

        _ = try await repair(state, client: client)

        #expect(try state.attachmentRepo?.getBlobToUpload(id: ref.data) == nil)
    }

    @Test func aBlobUnderAnotherKeyIsNeverStored() async throws {
        let state = try await makeVault()
        let (_, ref) = try noteWithAttachment(in: state, keepBlob: false)
        let client = makeClient()
        let blob = try serverBlob("planted", key: SymmetricKey(size: .bits256))
        HealthServer.reply("GET", blobPath + ref.id, status: 200, body: jsonBody(id: ref.id, data: blob.base64EncodedString()))

        let (_, result) = try await repair(state, client: client)

        #expect(result == .unreadable)
        #expect(try state.attachmentRepo?.hasBlob(id: ref.data) == false)
    }

    /// D3: the server can't get plaintext into the vault by not encrypting it.
    @Test func plaintextFromTheServerIsNeverStored() async throws {
        let state = try await makeVault()
        let (_, ref) = try noteWithAttachment(in: state, keepBlob: false)
        let client = makeClient()
        let plaintext = Data("<script>planted</script>".utf8).base64EncodedString()
        HealthServer.reply("GET", blobPath + ref.id, status: 200, body: jsonBody(id: ref.id, data: plaintext))

        let (_, result) = try await repair(state, client: client)

        #expect(result == .unreadable)
        #expect(try state.attachmentRepo?.hasBlob(id: ref.data) == false)
    }

    @Test func invalidBase64IsNeverStored() async throws {
        let state = try await makeVault()
        let (_, ref) = try noteWithAttachment(in: state, keepBlob: false)
        let client = makeClient()
        HealthServer.reply("GET", blobPath + ref.id, status: 200, body: jsonBody(id: ref.id, data: "%%%not base64%%%"))

        let (_, result) = try await repair(state, client: client)

        #expect(result == .unreadable)
        #expect(try state.attachmentRepo?.hasBlob(id: ref.data) == false)
    }

    /// Valid ciphertext answering to the wrong id is a different attachment.
    @Test func aBlobForAnotherAttachmentIsNeverStored() async throws {
        let state = try await makeVault()
        let key = try #require(state.keyManager.masterKey)
        let (_, ref) = try noteWithAttachment(in: state, keepBlob: false)
        let client = makeClient()
        let blob = try serverBlob("someone else's", key: key)
        HealthServer.reply(
            "GET", blobPath + ref.id, status: 200,
            body: jsonBody(id: UUID().uuidString.lowercased(), data: blob.base64EncodedString())
        )

        let (_, result) = try await repair(state, client: client)

        #expect(result == .unreadable)
        #expect(try state.attachmentRepo?.hasBlob(id: ref.data) == false)
    }

    @Test func aResponseThatIsntAnAttachmentIsAFailure() async throws {
        let state = try await makeVault()
        let (_, ref) = try noteWithAttachment(in: state, keepBlob: false)
        let client = makeClient()
        HealthServer.reply("GET", blobPath + ref.id, status: 200, body: Data("[1,2,3]".utf8))

        let (_, result) = try await repair(state, client: client)

        #expect(result == .failed)
        #expect(try state.attachmentRepo?.hasBlob(id: ref.data) == false)
    }

    @Test func aBlobTheServerDoesntHaveSaysSo() async throws {
        let state = try await makeVault()
        _ = try noteWithAttachment(in: state, keepBlob: false)
        let client = makeClient()

        let (_, result) = try await repair(state, client: client)

        #expect(result == .notOnServer)
    }

    @Test func aServerErrorIsAFailureNotNotOnServer() async throws {
        let state = try await makeVault()
        let (_, ref) = try noteWithAttachment(in: state, keepBlob: false)
        let client = makeClient()
        HealthServer.reply("GET", blobPath + ref.id, status: 500)

        let (_, result) = try await repair(state, client: client)

        #expect(result == .failed)
    }

    /// An id from a note goes in a URL, and notes come from the server.
    @Test func anIdThatIsntOneIsNeverRequested() async throws {
        let state = try await makeVault()
        let client = makeClient()
        let missing = VaultHealthReport.MissingAttachment(
            attachmentId: "../../user/account", blobId: "../../user/account",
            noteId: UUID().uuidString, noteTitle: nil, filename: "x", mimeType: "text/plain", size: 1
        )

        let result = await VaultHealthService.repair(
            missing,
            client: client,
            attachmentRepo: try #require(state.attachmentRepo),
            key: try #require(state.keyManager.masterKey)
        )

        #expect(result == .unreadable)
        #expect(HealthServer.requested.isEmpty)
    }

    // MARK: - What the server has lost (jottery-m3wg)

    private func serverCheck(_ state: AppState, client: SyncClient) async throws -> [VaultHealthReport.HeldAttachment] {
        try await VaultHealthService.missingFromServer(try scan(state).heldAttachments, client: client)
    }

    /// The case the push warnings can't reach: synced, unedited, and its blob
    /// lost by the server.
    @Test func aBlobTheServerLostIsFoundForAStableNote() async throws {
        let state = try await makeVault()
        let (note, ref) = try noteWithAttachment(in: state, keepBlob: true)
        try markSynced(state, note.id)
        let client = makeClient()
        HealthServer.reply("HEAD", blobPath + ref.id, status: 404)

        let lost = try await serverCheck(state, client: client)

        #expect(lost.map(\.attachmentId) == [ref.id])
    }

    @Test func aBlobTheServerHasIsNotReported() async throws {
        let state = try await makeVault()
        let (note, ref) = try noteWithAttachment(in: state, keepBlob: true)
        try markSynced(state, note.id)
        let client = makeClient()
        HealthServer.reply("HEAD", blobPath + ref.id, status: 200)

        #expect(try await serverCheck(state, client: client).isEmpty)
    }

    /// Asking must not download anything: a vault can be far larger than memory.
    @Test func checkingDownloadsNoBlobs() async throws {
        let state = try await makeVault()
        let (note, ref) = try noteWithAttachment(in: state, keepBlob: true)
        try markSynced(state, note.id)
        let client = makeClient()
        HealthServer.reply("HEAD", blobPath + ref.id, status: 200)

        _ = try await serverCheck(state, client: client)

        #expect(HealthServer.requested.map(\.method) == ["HEAD"])
    }

    /// A note that hasn't been pushed yet takes its blob with it when it goes.
    @Test func aNoteStillWaitingToSyncIsLeftToTheNextPush() async throws {
        let state = try await makeVault()
        _ = try noteWithAttachment(in: state, keepBlob: true)
        let client = makeClient()

        #expect(try await serverCheck(state, client: client).isEmpty)
        #expect(HealthServer.requested.isEmpty)
    }

    /// Not knowing isn't the same as missing. A server having a bad moment
    /// mustn't look like one that has lost everything.
    @Test func aServerErrorStopsTheCheck() async throws {
        let state = try await makeVault()
        let (note, ref) = try noteWithAttachment(in: state, keepBlob: true)
        try markSynced(state, note.id)
        let client = makeClient()
        HealthServer.reply("HEAD", blobPath + ref.id, status: 500)

        await #expect(throws: (any Error).self) {
            try await serverCheck(state, client: client)
        }
    }

    /// Something between here and the server may not do HEAD.
    @Test func aServerWithoutHeadIsAskedWithGet() async throws {
        let state = try await makeVault()
        let key = try #require(state.keyManager.masterKey)
        let (note, ref) = try noteWithAttachment(in: state, keepBlob: true)
        try markSynced(state, note.id)
        let client = makeClient()
        HealthServer.reply("HEAD", blobPath + ref.id, status: 405)
        let blob = try serverBlob("file contents", key: key)
        HealthServer.reply("GET", blobPath + ref.id, status: 200, body: jsonBody(id: ref.id, data: blob.base64EncodedString()))

        #expect(try await serverCheck(state, client: client).isEmpty)
    }

    private func reupload(_ state: AppState, _ held: VaultHealthReport.HeldAttachment, client: SyncClient) async throws -> VaultHealthService.UploadResult {
        await VaultHealthService.reupload(
            held,
            client: client,
            attachmentRepo: try #require(state.attachmentRepo),
            key: try #require(state.keyManager.masterKey)
        )
    }

    private func acceptedPushBody() -> Data {
        Data(#"{"accepted":[],"rejected":[],"errors":[],"rejectedDeletions":[]}"#.utf8)
    }

    @Test func aLostBlobGoesBackInAnAttachmentsOnlyPush() async throws {
        let state = try await makeVault()
        let (note, ref) = try noteWithAttachment(in: state, keepBlob: true)
        try markSynced(state, note.id)
        let held = try #require(try scan(state).heldAttachments.first)
        let client = makeClient()
        HealthServer.reply("POST", "/api/v1/sync/push", status: 200, body: acceptedPushBody())
        HealthServer.reply("HEAD", blobPath + ref.id, status: 200)

        let result = try await reupload(state, held, client: client)

        #expect(result == .uploaded)
        let push = try #require(HealthServer.requested.first { $0.method == "POST" })
        let request = try JSONDecoder().decode(SyncPushRequest.self, from: push.body)
        #expect(request.notes.isEmpty)
        #expect(request.versions.isEmpty)
        #expect(request.attachments.map(\.id) == [ref.id])
        let blob = try #require(try state.attachmentRepo?.getBlob(id: ref.data))
        #expect(request.attachments.first?.data == blob.base64EncodedString())
        #expect(try state.attachmentRepo?.getBlobToUpload(id: ref.data) == nil)
    }

    /// The server ignores a blob it can't place, without an error, so only
    /// asking again shows whether it's there.
    @Test func aBlobTheServerDidntKeepIsNotCalledRepaired() async throws {
        let state = try await makeVault()
        let (note, ref) = try noteWithAttachment(in: state, keepBlob: true)
        try markSynced(state, note.id)
        let held = try #require(try scan(state).heldAttachments.first)
        let client = makeClient()
        HealthServer.reply("POST", "/api/v1/sync/push", status: 200, body: acceptedPushBody())
        HealthServer.reply("HEAD", blobPath + ref.id, status: 404)

        #expect(try await reupload(state, held, client: client) == .notKept)
    }

    /// A blob that's damaged here is no use to anyone, and overwriting nothing
    /// with it would hide that it needs replacing.
    @Test func aBlobThatDoesntOpenHereIsNotUploaded() async throws {
        let state = try await makeVault()
        let (note, ref) = try noteWithAttachment(in: state, keepBlob: true)
        try markSynced(state, note.id)
        try state.attachmentRepo?.updateBlobData(id: ref.data, data: Data("damaged".utf8))
        let held = try #require(try scan(state).heldAttachments.first)
        let client = makeClient()

        #expect(try await reupload(state, held, client: client) == .unreadable)
        #expect(HealthServer.requested.isEmpty)
    }

    @Test func aRefusedPushIsAFailure() async throws {
        let state = try await makeVault()
        let (note, _) = try noteWithAttachment(in: state, keepBlob: true)
        try markSynced(state, note.id)
        let held = try #require(try scan(state).heldAttachments.first)
        let client = makeClient()
        HealthServer.reply("POST", "/api/v1/sync/push", status: 413)

        #expect(try await reupload(state, held, client: client) == .failed)
    }

    // MARK: - The screen's model

    @Test func theModelScansAndCounts() async throws {
        let state = try await makeVault()
        _ = try noteWithAttachment(in: state, keepBlob: false)
        let model = VaultHealthModel()

        await model.scan(appState: state)

        #expect(model.report?.missingAttachments.count == 1)
        #expect(!model.isScanning)
        #expect(model.errorMessage == nil)
    }

    /// Deleting goes through the usual path, so other devices hear about it.
    @Test func deletingAnUnreadableNoteRemovesItAndTellsOtherDevices() async throws {
        let state = try await makeVault()
        let note = try #require(try state.createNote(content: "Fine"))
        let foreign = try TestVaults.foreignCiphertext()
        try TestVaults.execute(state, "UPDATE notes SET content = ? WHERE id = ?", [foreign, note.id])
        let model = VaultHealthModel()
        await model.scan(appState: state)

        model.deleteUnreadableNote(id: note.id, appState: state)

        #expect(model.report?.unreadableNotes.isEmpty == true)
        #expect(try state.noteRepo?.getRaw(id: note.id) == nil)
        let deletions = try #require(try state.syncRepo?.getPendingDeletions())
        #expect(deletions.map(\.id) == [note.id])
    }

    @Test func aRepairedAttachmentLeavesTheListAndOneThatFailedStays() async throws {
        let state = try await makeVault()
        let key = try #require(state.keyManager.masterKey)
        let (_, good) = try noteWithAttachment(in: state, content: "Good", keepBlob: false)
        let (_, gone) = try noteWithAttachment(in: state, content: "Gone", keepBlob: false)
        state.syncClient = makeClient()
        let blob = try serverBlob("file contents", key: key)
        HealthServer.reply("GET", blobPath + good.id, status: 200, body: jsonBody(id: good.id, data: blob.base64EncodedString()))
        let model = VaultHealthModel()
        await model.scan(appState: state)

        await model.repairMissing(appState: state)

        #expect(model.report?.missingAttachments.map(\.attachmentId) == [gone.id])
        #expect(model.repairResults[gone.id] == .notOnServer)
    }

    @Test func nothingRepairsWithoutASyncClient() async throws {
        let state = try await makeVault()
        _ = try noteWithAttachment(in: state, keepBlob: false)
        let model = VaultHealthModel()
        await model.scan(appState: state)

        await model.repairMissing(appState: state)

        #expect(model.report?.missingAttachments.count == 1)
        #expect(model.repairResults.isEmpty)
    }
}
