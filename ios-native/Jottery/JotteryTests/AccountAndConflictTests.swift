import CryptoKit
import Foundation
import GRDB
import Testing

@testable import Jottery

/// A server that answers by route and records what it was asked.
private final class AccountServer: URLProtocol, @unchecked Sendable {
    struct Recorded {
        var method: String
        var path: String
        var authorization: String?
        var contentType: String?
        var body: Data
    }

    struct Reply {
        var status: Int
        var body: Data
    }

    nonisolated(unsafe) private static var replies: [String: Reply] = [:]
    nonisolated(unsafe) private static var requests: [Recorded] = []
    nonisolated(unsafe) static var pushHandler: (@Sendable (SyncPushRequest) -> Data)?
    private static let lock = NSLock()

    static func reset() {
        lock.withLock {
            replies = [:]
            requests = []
            pushHandler = nil
        }
    }

    static func reply(_ method: String, _ path: String, status: Int = 200, json: String = "{}") {
        lock.withLock { replies["\(method) \(path)"] = Reply(status: status, body: Data(json.utf8)) }
    }

    static var recorded: [Recorded] { lock.withLock { requests } }

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        let method = request.httpMethod ?? "?"
        let path = request.url?.path ?? "?"
        let body = request.httpBody ?? request.httpBodyStream.map(Self.read) ?? Data()
        let reply: Reply = Self.lock.withLock {
            Self.requests.append(Recorded(
                method: method, path: path,
                authorization: request.value(forHTTPHeaderField: "Authorization"),
                contentType: request.value(forHTTPHeaderField: "Content-Type"),
                body: body
            ))
            if path.hasSuffix("sync/push"), let handler = Self.pushHandler,
               let push = try? JSONDecoder().decode(SyncPushRequest.self, from: body) {
                return Reply(status: 200, body: handler(push))
            }
            return Self.replies["\(method) \(path)"] ?? Reply(status: 404, body: Data())
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

private let accountJSON = """
{"email":"me@example.org","noteCount":3,"attachmentCount":1,"storageUsedBytes":5242880,
 "storageQuotaMb":100,"createdAt":"2026-01-01T00:00:00Z","lastSyncAt":null,
 "inbox":{"itemCount":2,"totalSizeBytes":10,"maxItems":100,"maxSizeMb":5,"hasToken":false}}
"""

private func makeSessionId() -> String { "session-\(UUID().uuidString)" }

private func rejection(of note: SyncNote, ancestorContent: String?, ancestorTags: [String]?) -> SyncRejected {
    SyncRejected(
        id: note.id, reason: "conflict", serverModifiedAt: "2026-11-05T09:00:00Z",
        serverContent: note.content, serverTags: [], serverVersion: 9, serverAttachments: [],
        serverPinned: false, serverContentHash: "server-hash", serverHashChain: ["server-hash"],
        ancestorHash: ancestorContent == nil ? nil : "ancestor-hash",
        ancestorContent: ancestorContent, ancestorTags: ancestorTags
    )
}

/// The Account screen, the sync section's extra rows and the common ancestor
/// in a conflict (R2-149). They share a suite because they share a stub server.
@MainActor
@Suite(.serialized)
struct AccountAndConflictTests {

    private let apiKey = "device-key-\(UUID().uuidString)"

    private func makeClient() -> SyncClient {
        AccountServer.reset()
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [AccountServer.self]
        return SyncClient(endpoint: "https://notes.example.org", apiKey: apiKey, configuration: configuration)
    }

    private func signedIn(sessionId: String = makeSessionId()) async -> (AccountModel, SyncClient, String) {
        let client = makeClient()
        AccountServer.reply("POST", "/api/v1/user/login", json: #"{"sessionId":"\#(sessionId)","expiresAt":"2030-01-01T00:00:00Z"}"#)
        AccountServer.reply("GET", "/api/v1/user/account", json: accountJSON)
        let model = AccountModel()
        await model.signIn(email: "me@example.org", password: "account-password", client: client)
        return (model, client, sessionId)
    }

    private func requests(_ method: String, _ path: String) -> [AccountServer.Recorded] {
        AccountServer.recorded.filter { $0.method == method && $0.path == path }
    }

    // MARK: - Deleting the account

    /// The server asks for the password again for the deletion itself.
    @Test func deletingTheAccountSendsThePasswordThatWasTyped() async throws {
        let client = makeClient()
        let sessionId = makeSessionId()
        AccountServer.reply("POST", "/api/v1/user/login", json: #"{"sessionId":"\#(sessionId)","expiresAt":"2030-01-01T00:00:00Z"}"#)
        AccountServer.reply("DELETE", "/api/v1/user/account")

        try await client.deleteAccount(email: "me@example.org", password: "typed-password", mode: .delete)

        let request = try #require(requests("DELETE", "/api/v1/user/account").first)
        let body = try JSONDecoder().decode(DeleteAccountRequest.self, from: request.body)
        #expect(body.currentPassword == "typed-password")
        #expect(request.authorization == "Bearer \(sessionId)")
    }

    // MARK: - The inbox

    /// An item's id goes into request paths and onto the note made from it, so
    /// one that isn't an id stops here.
    @Test func anInboxItemWhoseIdIsntOneIsDropped() async throws {
        let client = makeClient()
        let good = UUID().uuidString.lowercased()
        let fields = #""content":"x","tags":[],"createdAt":"2026-01-01T00:00:00Z""#
        AccountServer.reply("GET", "/api/v1/inbox", json: #"[{"id":"\#(good)",\#(fields)},{"id":"../user/account",\#(fields)}]"#)
        let state = try TestVaults.makeState()
        state.syncClient = client

        try await state.loadInboxItems()

        #expect(state.inboxItems.map(\.id) == [good])
    }

    // MARK: - Signing in

    @Test func signingInShowsTheAccount() async {
        let (model, _, _) = await signedIn()

        #expect(model.isSignedIn)
        #expect(model.info?.email == "me@example.org")
        #expect(model.info?.noteCount == 3)
        #expect(model.info?.inbox?.hasToken == false)
    }

    /// Account routes need a session. A device key must not be able to reach them.
    @Test func accountCallsUseTheSessionAndNotTheDeviceKey() async {
        let (_, _, sessionId) = await signedIn()

        let request = requests("GET", "/api/v1/user/account").first
        #expect(request?.authorization == "Bearer \(sessionId)")
    }

    @Test func aWrongPasswordLeavesItSignedOut() async {
        let client = makeClient()
        AccountServer.reply("POST", "/api/v1/user/login", status: 401, json: #"{"error":"Invalid credentials"}"#)
        let model = AccountModel()

        await model.signIn(email: "me@example.org", password: "wrong", client: client)

        #expect(!model.isSignedIn)
        #expect(model.errorMessage == L.deleteAccountWrongCredentials)
    }

    /// Error bodies are the server's own words, and it isn't trusted to choose
    /// what the user is told.
    @Test func aServerErrorDoesntShowTheServersWords() async {
        let client = makeClient()
        let planted = "Your phone is infected, call 0800 000"
        AccountServer.reply("POST", "/api/v1/user/login", status: 500, json: planted)
        let model = AccountModel()

        await model.signIn(email: "me@example.org", password: "x", client: client)

        #expect(model.errorMessage?.contains(planted) == false)
        #expect(model.errorMessage == L.accountRequestFailed)
    }

    @Test func aSessionThatHasEndedSignsOutAndSaysSo() async {
        let (model, client, _) = await signedIn()
        AccountServer.reply("GET", "/api/v1/user/account", status: 401)

        await model.refresh(client: client)

        #expect(!model.isSignedIn)
        #expect(model.info == nil)
        #expect(model.errorMessage == L.accountSessionEnded)
    }

    @Test func anOlderServerWithNoInboxDetailsStillShowsUsage() async {
        let client = makeClient()
        AccountServer.reply("POST", "/api/v1/user/login", json: #"{"sessionId":"s","expiresAt":"2030-01-01T00:00:00Z"}"#)
        AccountServer.reply("GET", "/api/v1/user/account", json: #"{"email":"me@example.org","noteCount":3}"#)
        let model = AccountModel()

        await model.signIn(email: "me@example.org", password: "x", client: client)

        #expect(model.info?.noteCount == 3)
        #expect(model.info?.inbox == nil)
    }

    // MARK: - Leaving nothing behind

    private func expectNotStored(_ secret: String, in state: AppState) throws {
        let database = try #require(state.db)
        let found = try database.dbPool.read { db -> Bool in
            for table in try String.fetchAll(db, sql: "SELECT name FROM sqlite_master WHERE type = 'table'") {
                for row in try Row.fetchAll(db, sql: "SELECT * FROM \"\(table)\"") {
                    for column in row.columnNames where (row[column] as DatabaseValue).description.contains(secret) {
                        return true
                    }
                }
            }
            return false
        }
        #expect(!found)
        let defaults = UserDefaults.standard.dictionaryRepresentation().description
        #expect(!defaults.contains(secret))
    }

    @Test func theSessionTokenIsNeverStored() async throws {
        let state = try TestVaults.makeState()
        try await state.createVault(password: "account-tests-\(UUID().uuidString)")
        let (model, _, sessionId) = await signedIn()

        #expect(model.isSignedIn)
        try expectNotStored(sessionId, in: state)
    }

    @Test func theInboxTokenIsNeverStored() async throws {
        let state = try TestVaults.makeState()
        try await state.createVault(password: "account-tests-\(UUID().uuidString)")
        let (model, client, _) = await signedIn()
        let token = String(repeating: "ab", count: 32)
        AccountServer.reply("POST", "/api/v1/user/inbox-token", status: 201, json: #"{"token":"\#(token)"}"#)

        await model.generateInboxToken(client: client)

        #expect(model.disclosedToken == token)
        try expectNotStored(token, in: state)
    }

    @Test func leavingEndsTheSessionAndForgetsTheToken() async {
        let (model, client, sessionId) = await signedIn()
        AccountServer.reply("POST", "/api/v1/user/inbox-token", status: 201, json: #"{"token":"abcdef"}"#)
        AccountServer.reply("POST", "/api/v1/user/logout", status: 204, json: "")
        await model.generateInboxToken(client: client)

        model.signOut(client: client)

        #expect(!model.isSignedIn)
        #expect(model.disclosedToken == nil)
        #expect(model.info == nil)
        await waitUntil { !requests("POST", "/api/v1/user/logout").isEmpty }
        #expect(requests("POST", "/api/v1/user/logout").first?.authorization == "Bearer \(sessionId)")
    }

    // MARK: - Deleting server notes

    private func allowDelete() {
        AccountServer.reply("DELETE", "/api/v1/user/notes", status: 204, json: "")
    }

    @Test func deletingSendsThePasswordWithTheSession() async throws {
        let (model, client, sessionId) = await signedIn()
        allowDelete()

        let deleted = await model.deleteServerNotes(password: "account-password", client: client)

        #expect(deleted)
        let request = try #require(requests("DELETE", "/api/v1/user/notes").first)
        #expect(request.authorization == "Bearer \(sessionId)")
        #expect(request.contentType == "application/json")
        let body = try JSONDecoder().decode(DeleteAccountRequest.self, from: request.body)
        #expect(body.currentPassword == "account-password")
    }

    @Test func deletingRefreshesTheFigures() async {
        let (model, client, _) = await signedIn()
        allowDelete()
        AccountServer.reply("GET", "/api/v1/user/account", json: #"{"email":"me@example.org","noteCount":0}"#)

        await model.deleteServerNotes(password: "account-password", client: client)

        #expect(model.info?.noteCount == 0)
    }

    /// Without the password there's no request at all.
    @Test func nothingIsDeletedWithoutAPassword() async {
        let (model, client, _) = await signedIn()
        allowDelete()

        let deleted = await model.deleteServerNotes(password: "", client: client)

        #expect(!deleted)
        #expect(requests("DELETE", "/api/v1/user/notes").isEmpty)
    }

    @Test func theWrongPasswordDeletesNothingAndStaysSignedIn() async {
        let (model, client, _) = await signedIn()
        AccountServer.reply("DELETE", "/api/v1/user/notes", status: 401, json: #"{"error":"nope"}"#)

        let deleted = await model.deleteServerNotes(password: "wrong", client: client)

        #expect(!deleted)
        #expect(model.isSignedIn)
        #expect(model.errorMessage == L.accountDeleteWrongPassword)
    }

    /// The server's notes go. This device's are the user's only copy now.
    @Test func notesOnThisDeviceAreLeftAlone() async throws {
        let state = try TestVaults.makeState()
        try await state.createVault(password: "account-tests-\(UUID().uuidString)")
        let note = try #require(try state.createNote(content: "Mine"))
        let before = try #require(try state.noteRepo?.getRaw(id: note.id))
        let (model, client, _) = await signedIn()
        allowDelete()

        await model.deleteServerNotes(password: "account-password", client: client)

        let after = try #require(try state.noteRepo?.getRaw(id: note.id))
        #expect(after.content == before.content)
        #expect(!after.deleted)
    }

    // MARK: - The inbox token

    @Test func generatingShowsTheTokenOnceAndNotesThatThereIsOne() async {
        let (model, client, sessionId) = await signedIn()
        AccountServer.reply("POST", "/api/v1/user/inbox-token", status: 201, json: #"{"token":"abcdef0123"}"#)

        await model.generateInboxToken(client: client)

        #expect(model.disclosedToken == "abcdef0123")
        #expect(model.info?.inbox?.hasToken == true)
        #expect(requests("POST", "/api/v1/user/inbox-token").first?.authorization == "Bearer \(sessionId)")
    }

    @Test func aTokenThatCouldntBeOneIsNotShown() async {
        let (model, client, _) = await signedIn()
        let planted = String(repeating: "x", count: 5_000)
        AccountServer.reply("POST", "/api/v1/user/inbox-token", status: 201, json: #"{"token":"\#(planted)"}"#)

        await model.generateInboxToken(client: client)

        #expect(model.disclosedToken == nil)
        #expect(model.errorMessage == L.accountRequestFailed)
    }

    @Test func revokingForgetsTheTokenAndNotesThereIsNone() async {
        let (model, client, _) = await signedIn()
        AccountServer.reply("POST", "/api/v1/user/inbox-token", status: 201, json: #"{"token":"abcdef0123"}"#)
        AccountServer.reply("DELETE", "/api/v1/user/inbox-token", status: 204, json: "")
        await model.generateInboxToken(client: client)

        await model.revokeInboxToken(client: client)

        #expect(model.disclosedToken == nil)
        #expect(model.info?.inbox?.hasToken == false)
    }

    // MARK: - Figures from the server

    @Test func usageIsAFractionOfTheQuota() throws {
        let info = try JSONDecoder().decode(AccountInfo.self, from: Data(accountJSON.utf8))
        #expect(info.usedFraction == 0.05)
    }

    @Test func aFractionNeverLeavesZeroToOne() throws {
        let over = try JSONDecoder().decode(AccountInfo.self, from: Data(#"{"storageUsedBytes":999999999999,"storageQuotaMb":1}"#.utf8))
        #expect(over.usedFraction == 1)
        let negative = try JSONDecoder().decode(AccountInfo.self, from: Data(#"{"storageUsedBytes":-5,"storageQuotaMb":1}"#.utf8))
        #expect(negative.usedFraction == 0)
    }

    @Test func noQuotaMeansNoFraction() throws {
        let zero = try JSONDecoder().decode(AccountInfo.self, from: Data(#"{"storageUsedBytes":5,"storageQuotaMb":0}"#.utf8))
        #expect(zero.usedFraction == nil)
        #expect(zero.quotaBytes == nil)
    }

    /// Multiplying this up would trap, on a number the server chose.
    @Test func aQuotaTooLargeToBeRealDoesntCrash() throws {
        let huge = try JSONDecoder().decode(AccountInfo.self, from: Data(#"{"storageUsedBytes":5,"storageQuotaMb":9223372036854775807}"#.utf8))
        #expect(huge.quotaBytes == nil)
    }

    // MARK: - The sync section

    @Test func aDeviceNameLosesWhatItShouldntHave() {
        #expect(SyncValidation.displayName("My iPhone") == "My iPhone")
        #expect(SyncValidation.displayName("  My\u{0}\u{202E}iPhone\n ") == "MyiPhone")
        #expect(SyncValidation.displayName(" \n ") == nil)
        #expect(SyncValidation.displayName(String(repeating: "a", count: 500))?.count == SyncValidation.maxDisplayNameLength)
    }

    @Test func theDeviceNameComesFromTheServer() async throws {
        let client = makeClient()
        AccountServer.reply("GET", "/api/v1/sync/status", json: """
        {"clientId":"c","deviceName":"Chris\\u0000's iPhone","serverLastModified":"2026-01-01T00:00:00Z","noteCount":1}
        """)

        #expect(await client.deviceName() == "Chris's iPhone")
    }

    @Test func noDeviceNameIfTheServerCantBeAsked() async {
        let client = makeClient()
        AccountServer.reply("GET", "/api/v1/sync/status", status: 500)

        #expect(await client.deviceName() == nil)
    }

    @Test func theCountOfWhatsWaitingIncludesEverythingThatWillGoUp() async throws {
        let state = try TestVaults.makeState()
        try await state.createVault(password: "account-tests-\(UUID().uuidString)")
        _ = try state.createNote(content: "New")
        let binned = try #require(try state.createNote(content: "Binned"))
        try state.deleteNote(id: binned.id)
        let gone = try #require(try state.createNote(content: "Gone"))
        try state.permanentlyDeleteNote(id: gone.id)

        #expect(state.pendingSyncCount() == 3)
    }

    @Test func nothingWaitingIsZero() async throws {
        let state = try TestVaults.makeState()
        try await state.createVault(password: "account-tests-\(UUID().uuidString)")

        #expect(state.pendingSyncCount() == 0)
    }

    // MARK: - The common ancestor

    private func encrypted(_ text: String, key: SymmetricKey) throws -> String {
        try CryptoService.serializeEncryptedJSON(CryptoService.encryptText(text, key: key))
    }

    @Test func anAncestorDecrypts() throws {
        let key = SymmetricKey(size: .bits256)
        let ancestor = ConflictAncestor.decrypted(
            content: try encrypted("Where they split", key: key),
            tags: [try CryptoService.encryptTagForSync("work", key: key)],
            key: key
        )
        #expect(ancestor == ConflictAncestor(content: "Where they split", tags: ["work"]))
    }

    /// Web and Android write a version's tags as one blob, and an ancestor comes
    /// from version history.
    @Test func anAncestorsTagsCanBeOneBlob() throws {
        let key = SymmetricKey(size: .bits256)
        let blob = try CryptoService.serializeEncryptedJSON(CryptoService.encryptStringArray(["a", "b"], key: key))

        let ancestor = ConflictAncestor.decrypted(content: try encrypted("x", key: key), tags: [blob], key: key)

        #expect(ancestor?.tags == ["a", "b"])
    }

    @Test func anAncestorWithNoTagsHasNone() throws {
        let key = SymmetricKey(size: .bits256)
        #expect(ConflictAncestor.decrypted(content: try encrypted("x", key: key), tags: nil, key: key)?.tags == [])
    }

    @Test func noAncestorContentIsNoAncestor() {
        #expect(ConflictAncestor.decrypted(content: nil, tags: nil, key: SymmetricKey(size: .bits256)) == nil)
    }

    /// D3: an ancestor that won't decrypt is no ancestor, not a third version
    /// made of ciphertext or of whatever the server typed.
    @Test func anAncestorUnderAnotherKeyIsNoAncestor() throws {
        let content = try encrypted("x", key: SymmetricKey(size: .bits256))
        #expect(ConflictAncestor.decrypted(content: content, tags: [], key: SymmetricKey(size: .bits256)) == nil)
    }

    @Test func plaintextIsNoAncestor() {
        #expect(ConflictAncestor.decrypted(content: "planted text", tags: [], key: SymmetricKey(size: .bits256)) == nil)
    }

    @Test func oneBadTagMakesItNoAncestor() throws {
        let key = SymmetricKey(size: .bits256)
        let good = try CryptoService.encryptTagForSync("good", key: key)

        #expect(ConflictAncestor.decrypted(content: try encrypted("x", key: key), tags: [good, "planted"], key: key) == nil)
    }

    private func conflict(ancestor: ConflictAncestor?) -> ConflictInfo {
        ConflictInfo(
            id: UUID().uuidString, localContent: "Mine", localTags: ["l"], localModifiedAt: "2026-01-01T00:00:00Z",
            localAttachments: [], serverContent: "Theirs", serverTags: ["s"], serverModifiedAt: "2026-01-02T00:00:00Z",
            serverEncryptedContent: "", serverEncryptedTags: [], serverVersion: 1, serverAttachments: [],
            serverPinned: false, serverSyntaxLanguage: nil, serverWordWrap: nil, serverShowPreview: nil,
            serverContentHash: nil, serverParentHash: nil, serverHashChain: nil, ancestor: ancestor
        )
    }

    @Test func thePickerOffersTheAncestorOnlyWhenThereIsOne() {
        #expect(conflict(ancestor: nil).versions == [.local, .server])
        #expect(conflict(ancestor: ConflictAncestor(content: "x", tags: [])).versions == [.local, .server, .ancestor])
    }

    @Test func eachVersionShowsItsOwnContent() {
        let info = conflict(ancestor: ConflictAncestor(content: "Before", tags: ["b"]))

        #expect(info.content(of: .local) == "Mine")
        #expect(info.content(of: .server) == "Theirs")
        #expect(info.content(of: .ancestor) == "Before")
        #expect(info.tags(of: .ancestor) == ["b"])
        #expect(info.modifiedAt(of: .server) == "2026-01-02T00:00:00Z")
        #expect(info.modifiedAt(of: .ancestor) == nil)
    }

    private func makeSyncFixture() throws -> (SyncService, NoteRepository, SymmetricKey) {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-ancestor-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let db = try DatabaseManager(path: dir.appendingPathComponent("test.db").path)
        let versionRepo = VersionRepository(db: db)
        let repo = NoteRepository(db: db, versionRepo: versionRepo)
        let key = SymmetricKey(size: .bits256)
        let service = SyncService(
            syncClient: makeClient(), noteRepo: repo, syncRepo: SyncRepository(db: db), versionRepo: versionRepo,
            attachmentRepo: AttachmentRepository(db: db), savedSearchRepo: SavedSearchRepository(db: db), key: key
        )
        AccountServer.reply("POST", "/api/v1/sync/pull", json: #"{"notes":[],"attachments":[],"versions":[]}"#)
        return (service, repo, key)
    }

    /// Runs on the stub's thread, so nothing here belongs to the main actor.
    private func refuse(ancestorContent: String?, ancestorTags: [String]?) {
        AccountServer.pushHandler = { request in
            let rejected = request.notes.map {
                rejection(of: $0, ancestorContent: ancestorContent, ancestorTags: ancestorTags)
            }
            return (try? JSONEncoder().encode(SyncPushResponse(accepted: [], rejected: rejected))) ?? Data()
        }
    }

    @Test func aRefusedPushCarriesTheAncestorIntoTheConflict() async throws {
        let (service, repo, key) = try makeSyncFixture()
        _ = try repo.create(content: "My edit", key: key)
        refuse(
            ancestorContent: try encrypted("Where they split", key: key),
            ancestorTags: [try CryptoService.encryptTagForSync("work", key: key)]
        )

        try await service.sync()

        let conflicts = await service.pendingConflicts
        #expect(conflicts.first?.ancestor == ConflictAncestor(content: "Where they split", tags: ["work"]))
    }

    @Test func anOlderServerGivesAConflictWithNoAncestor() async throws {
        let (service, repo, key) = try makeSyncFixture()
        _ = try repo.create(content: "My edit", key: key)
        refuse(ancestorContent: nil, ancestorTags: nil)

        try await service.sync()

        let conflicts = await service.pendingConflicts
        #expect(conflicts.count == 1)
        #expect(conflicts.first?.ancestor == nil)
    }

    @Test func anAncestorThatWontOpenLeavesTheConflictIntact() async throws {
        let (service, repo, key) = try makeSyncFixture()
        _ = try repo.create(content: "My edit", key: key)
        refuse(ancestorContent: "planted by the server", ancestorTags: nil)

        try await service.sync()

        let conflicts = await service.pendingConflicts
        #expect(conflicts.count == 1)
        #expect(conflicts.first?.ancestor == nil)
    }
}
