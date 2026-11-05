import CryptoKit
import Foundation
import Testing

@testable import Jottery

/// Holds a server's requests until the test opens it, so a sync stays in
/// progress for exactly as long as a test needs. Each test has its own: a sync
/// left running can't reach into the next one.
final class RequestGate: @unchecked Sendable {
    private let lock = NSLock()
    private var isOpen = false
    private var held: [@Sendable () -> Void] = []
    private var received = 0

    /// Requests that have arrived, answered or not.
    var requestCount: Int { lock.withLock { received } }
    /// Requests in flight now, waiting for `open()`.
    var heldCount: Int { lock.withLock { held.count } }

    func receive(answer: @escaping @Sendable () -> Void) {
        let answerNow = lock.withLock {
            received += 1
            if !isOpen { held.append(answer) }
            return isOpen
        }
        if answerNow { answer() }
    }

    /// Answer everything held, and anything that arrives from now on.
    func open() {
        let answers = lock.withLock {
            isOpen = true
            defer { held = [] }
            return held
        }
        answers.forEach { $0() }
    }
}

/// A server that answers every request with an error, but only when its gate
/// says so. Which gate is decided by the request's host. 500, as a busy or
/// rate-limited answer would hold back the next sync for a reason of its own.
final class GatedServerProtocol: URLProtocol, @unchecked Sendable {

    private static let lock = NSLock()
    nonisolated(unsafe) private static var gates: [String: RequestGate] = [:]

    /// The endpoint of a server held by `gate`.
    static func endpoint(for gate: RequestGate) -> String {
        let host = "\(UUID().uuidString.lowercased()).example.test"
        lock.withLock { gates[host] = gate }
        return "https://\(host)"
    }

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        let url = request.url!
        let gate = Self.lock.withLock { Self.gates[url.host() ?? ""] }
        gate?.receive { [self] in
            let response = HTTPURLResponse(url: url, statusCode: 500, httpVersion: "HTTP/1.1", headerFields: nil)!
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: Data("failed".utf8))
            client?.urlProtocolDidFinishLoading(self)
        }
    }

    override func stopLoading() {}
}

/// A sync service encrypts and decrypts with the key it was created with. When
/// the vault's key changes, a sync still running would push old-key
/// ciphertext (clearing the "needs sync" flag the re-key just set) or write
/// pulled data under the old key. So the service is shut down, and a sync in
/// progress drained, before any re-key.
@Suite(.serialized)
@MainActor
struct SyncServiceShutdownTests {

    private let password = "notes-password-correct-horse"

    private func makeService(_ state: AppState, gate: RequestGate) throws -> SyncService {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [GatedServerProtocol.self]
        // Nothing may end a held request but the gate.
        configuration.timeoutIntervalForRequest = 86_400
        configuration.timeoutIntervalForResource = 86_400
        let client = SyncClient(
            endpoint: GatedServerProtocol.endpoint(for: gate), apiKey: "device-key", configuration: configuration
        )
        return SyncService(
            syncClient: client,
            noteRepo: try #require(state.noteRepo),
            syncRepo: try #require(state.syncRepo),
            versionRepo: try #require(state.versionRepo),
            attachmentRepo: try #require(state.attachmentRepo),
            savedSearchRepo: try #require(state.savedSearchRepo),
            key: try #require(state.keyManager.masterKey)
        )
    }

    /// Start a sync and wait until its request has reached the server, where
    /// it's held. From here it can't finish until the test opens the gate.
    private func startHeldSync(_ service: SyncService, gate: RequestGate) async throws -> Task<Void, Never> {
        let task = Task { _ = try? await service.sync() }
        try #require(await waitUntil { gate.heldCount == 1 }, "the sync's request never arrived")
        return task
    }

    @Test func shutDownWaitsForTheSyncInProgressAndBlocksNewOnes() async throws {
        let gate = RequestGate()
        let state = try TestVaults.makeState()
        try await state.createVault(password: password)
        let service = try makeService(state, gate: gate)
        let syncTask = try await startHeldSync(service, gate: gate)

        // Long enough that only the gate can end this.
        let shutDown = Task { await service.shutDown(timeout: .seconds(86_400)) }
        try #require(await waitUntil { await service.isShutDown })
        #expect(await service.isSyncing)  // so it's waiting, not finished
        gate.open()

        #expect(await shutDown.value)
        #expect(!(await service.isSyncing))
        _ = await syncTask.value
        let requests = gate.requestCount
        _ = try? await service.sync()
        #expect(gate.requestCount == requests)  // no new sync after shutdown
    }

    @Test func shutDownGivesUpAtItsDeadline() async throws {
        let gate = RequestGate()
        let state = try TestVaults.makeState()
        try await state.createVault(password: password)
        let service = try makeService(state, gate: gate)
        let syncTask = try await startHeldSync(service, gate: gate)

        let drained = await service.shutDown(timeout: .milliseconds(200))

        // The request can't have been answered, however long that took.
        #expect(!drained)
        #expect(gate.heldCount == 1)
        #expect(await service.isSyncing)

        gate.open()
        _ = await syncTask.value
    }

    @Test func aKeySwitchWaitsForTheRunningSyncFirst() async throws {
        let gate = RequestGate()
        let state = try TestVaults.makeState()
        try await state.createVault(password: password)
        _ = try state.createNote(content: "kept", tags: [])
        let service = try makeService(state, gate: gate)
        state.syncService = service
        state.syncShutdownTimeout = .seconds(86_400)
        let syncTask = try await startHeldSync(service, gate: gate)
        let keyBefore = TestVaults.keyData(state)

        let keySwitch = Task {
            try await state.switchVaultToMasterKey(CryptoService.generateMasterKey(), notesPassword: password)
        }
        try #require(await waitUntil { await service.isShutDown })

        // It's waiting for the sync, and nothing has been re-encrypted yet.
        #expect(await service.isSyncing)
        #expect(TestVaults.keyData(state) == keyBefore)
        gate.open()
        try await keySwitch.value

        // The old service finished before the re-key and was retired.
        #expect(!(await service.isSyncing))
        #expect(TestVaults.keyData(state) != keyBefore)
        _ = await syncTask.value
        let requests = gate.requestCount
        _ = try? await service.sync()
        #expect(gate.requestCount == requests)
        #expect(state.notes.map(\.content) == ["kept"])
    }
}
