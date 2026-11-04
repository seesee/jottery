import CryptoKit
import Foundation
import Testing

@testable import Jottery

/// A server that answers every request with an error after a delay, so a sync
/// stays in progress long enough to overlap with something else.
final class SlowServerProtocol: URLProtocol, @unchecked Sendable {

    nonisolated(unsafe) static var delay: TimeInterval = 0.5
    nonisolated(unsafe) static var requestCount = 0

    static func reset(delay: TimeInterval) {
        self.delay = delay
        requestCount = 0
    }

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        Self.requestCount += 1
        let url = request.url!
        DispatchQueue.global().asyncAfter(deadline: .now() + Self.delay) { [self] in
            let response = HTTPURLResponse(url: url, statusCode: 503, httpVersion: "HTTP/1.1", headerFields: nil)!
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: Data("busy".utf8))
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

    private func makeService(_ state: AppState) throws -> SyncService {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [SlowServerProtocol.self]
        let client = SyncClient(endpoint: "https://sync.example.test", apiKey: "device-key", configuration: configuration)
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

    /// Start a sync and wait until it is in progress.
    private func startSync(_ service: SyncService) async throws -> Task<Void, Never> {
        let task = Task { _ = try? await service.sync() }
        let deadline = ContinuousClock.now + .seconds(30)
        while !(await service.isSyncing) {
            try #require(ContinuousClock.now < deadline, "sync never started")
            try await Task.sleep(for: .milliseconds(10))
        }
        return task
    }

    @Test func shutDownWaitsForTheSyncInProgressAndBlocksNewOnes() async throws {
        SlowServerProtocol.reset(delay: 0.5)
        let state = try TestVaults.makeState()
        try await state.createVault(password: password)
        let service = try makeService(state)
        let syncTask = try await startSync(service)

        let drained = await service.shutDown(timeout: .seconds(10))

        #expect(drained)
        #expect(!(await service.isSyncing))
        _ = await syncTask.value
        let requests = SlowServerProtocol.requestCount
        _ = try? await service.sync()
        #expect(SlowServerProtocol.requestCount == requests)  // no new sync after shutdown
    }

    @Test func shutDownGivesUpAtItsDeadline() async throws {
        SlowServerProtocol.reset(delay: 2.0)
        let state = try TestVaults.makeState()
        try await state.createVault(password: password)
        let service = try makeService(state)
        _ = try await startSync(service)

        let drained = await service.shutDown(timeout: .milliseconds(200))

        #expect(!drained)
    }

    @Test func aKeySwitchWaitsForTheRunningSyncFirst() async throws {
        SlowServerProtocol.reset(delay: 0.5)
        let state = try TestVaults.makeState()
        try await state.createVault(password: password)
        _ = try state.createNote(content: "kept", tags: [])
        let service = try makeService(state)
        state.syncService = service
        let syncTask = try await startSync(service)

        try await state.switchVaultToMasterKey(CryptoService.generateMasterKey(), notesPassword: password)

        // The old service finished before the re-key and was retired.
        #expect(!(await service.isSyncing))
        _ = await syncTask.value
        let requests = SlowServerProtocol.requestCount
        _ = try? await service.sync()
        #expect(SlowServerProtocol.requestCount == requests)
        #expect(state.notes.map(\.content) == ["kept"])
    }
}
