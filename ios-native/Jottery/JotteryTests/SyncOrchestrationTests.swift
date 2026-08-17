import CryptoKit
import Foundation
import Testing

@testable import Jottery

/// Stub transport scoped to this file only — deliberately not the shared
/// `StubURLProtocol` from SyncClientTransportTests.swift, which drives its
/// stub state through `nonisolated(unsafe) static var`s. Two top-level test
/// suites can run concurrently in swift-testing, and sharing that mutable
/// static state across suites would make both flaky.
private final class ForceFullSyncStubURLProtocol: URLProtocol, @unchecked Sendable {
    nonisolated(unsafe) static var body: Data = Data()

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

/// Covers Task 2 of the iOS correctness-hardening plan (jottery-dggw + t94v +
/// vn6f):
///
/// 1. `AppState.triggerSync()` must route through `SyncService.sync()` —
///    which holds the actor's own `isSyncing` re-entrancy guard — instead of
///    calling `push()`/`pull()` directly. The old code bypassed that guard,
///    so an SSE-triggered `sync()` and a UI-triggered `triggerSync()` could
///    interleave across actor suspension points.
/// 2. `AppState` gains a monotonically increasing `loadEpoch`. `loadNotes()`
///    bumps it before its detached decrypt; `applySyncChanges(_:)` captures
///    it but never bumps it, and both drop their publication if a newer full
///    reload has started in the meantime. Exercised through the
///    `publishIfCurrent(epoch:_:)` seam directly — deterministic — rather
///    than by racing real `Task`s against `loadNotes()`.
/// 3. `AppState.unlock(password:)` gains an internal re-entrancy guard: two
///    concurrent calls must have exactly one real effect.
/// 4. `SyncService.forceFullSync()` no longer double-fires a reload via its
///    postSyncHandler; `AppState.forceFullSync()`'s own explicit reload is
///    the only one left.
@MainActor
struct SyncOrchestrationTests {

    private func makeUnlockedState() throws -> AppState {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-sync-orchestration-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let state = AppState()
        try state.initialise(database: DatabaseManager(path: dir.appendingPathComponent("test.db").path))
        state.keyManager.unlockWithKeyData(Data(repeating: 7, count: 32))
        state.isLocked = false
        return state
    }

    // MARK: - jottery-dggw: single entry point

    /// `SyncService.sync()` is the only place that sets `lastError` on
    /// failure — `push()`/`pull()` don't touch it themselves. If
    /// `triggerSync()` still called `push()`/`pull()` directly (the bug), a
    /// failing sync would leave `syncService.lastError` untouched even
    /// though `AppState`'s own `syncError` got set. Routing through `sync()`
    /// makes both non-nil.
    @Test func triggerSyncRoutesThroughActorSyncEntryPoint() async throws {
        let state = try makeUnlockedState()
        let noteRepo = try #require(state.noteRepo)
        let key = try #require(state.keyManager.masterKey)
        // At least one dirty record so push() actually attempts a network
        // call instead of short-circuiting on an empty change set.
        _ = try noteRepo.create(content: "needs push", tags: [], key: key)

        // Nothing listens on this port — the push attempt fails fast and
        // deterministically (connection refused) without needing a mock
        // server.
        let syncClient = SyncClient(endpoint: "http://localhost:9")
        let service = SyncService(
            syncClient: syncClient,
            noteRepo: noteRepo,
            syncRepo: try #require(state.syncRepo),
            versionRepo: try #require(state.versionRepo),
            attachmentRepo: try #require(state.attachmentRepo),
            savedSearchRepo: try #require(state.savedSearchRepo),
            key: key
        )
        state.syncService = service

        #expect(await service.lastError == nil)

        await state.triggerSync()

        let lastError = await service.lastError
        #expect(lastError != nil)
        #expect(state.syncError != nil)
        #expect(state.isSyncing == false)
    }

    // MARK: - jottery-t94v: load-epoch guard

    @Test func publishIfCurrentAppliesWhenEpochMatches() throws {
        let state = try makeUnlockedState()
        let epoch = state.loadEpoch

        var ran = false
        let applied = state.publishIfCurrent(epoch: epoch) { ran = true }

        #expect(applied == true)
        #expect(ran == true)
    }

    @Test func publishIfCurrentSkipsWhenEpochIsStale() throws {
        let state = try makeUnlockedState()
        let currentEpoch = state.loadEpoch

        var ran = false
        // A caller that captured an epoch before a newer full reload bumped
        // `loadEpoch` must have its publication skipped.
        let applied = state.publishIfCurrent(epoch: currentEpoch - 1) { ran = true }

        #expect(applied == false)
        #expect(ran == false)
    }

    @Test func loadNotesBumpsLoadEpoch() async throws {
        let state = try makeUnlockedState()
        let epochBefore = state.loadEpoch

        try await state.loadNotes()
        #expect(state.loadEpoch == epochBefore + 1)

        try await state.loadNotes()
        #expect(state.loadEpoch == epochBefore + 2)
    }

    @Test func applySyncChangesDoesNotBumpLoadEpoch() async throws {
        let state = try makeUnlockedState()
        let noteRepo = try #require(state.noteRepo)
        let key = try #require(state.keyManager.masterKey)

        let note = try noteRepo.create(content: "original", tags: [], key: key)
        try await state.loadNotes()
        let epochAfterLoad = state.loadEpoch

        var updated = note
        updated.content = "updated by server"
        try noteRepo.update(updated, key: key)

        await state.applySyncChanges(SyncChanges(updatedIds: [note.id]))

        // The incremental apply must not itself advance the generation —
        // only full reloads (loadNotes()) do that.
        #expect(state.loadEpoch == epochAfterLoad)
        #expect(state.notes.first(where: { $0.id == note.id })?.content == "updated by server")
    }

    @Test func applySyncChangesFullReloadFallbackBumpsLoadEpoch() async throws {
        let state = try makeUnlockedState()
        let noteRepo = try #require(state.noteRepo)
        let key = try #require(state.keyManager.masterKey)
        _ = try noteRepo.create(content: "note", tags: [], key: key)
        try await state.loadNotes()
        let epochAfterLoad = state.loadEpoch

        // .fullReload routes through loadNotes() internally, which does bump.
        await state.applySyncChanges(.fullReload)

        #expect(state.loadEpoch == epochAfterLoad + 1)
    }

    // MARK: - jottery-t94v: unlock re-entrancy guard

    @Test func concurrentUnlockCallsHaveSingleEffect() async throws {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-unlock-reentrancy-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let state = AppState()
        try state.initialise(database: DatabaseManager(path: dir.appendingPathComponent("test.db").path))

        try await state.createVault(password: DemoSeedService.demoPassword)
        state.lock()
        #expect(state.isLocked == true)

        async let first = try state.unlock(password: DemoSeedService.demoPassword)
        async let second = try state.unlock(password: DemoSeedService.demoPassword)

        let (result1, result2) = try await (first, second)

        // Exactly one of the two concurrent calls performed the real unlock
        // (returned true); the other saw the re-entrancy guard already set
        // and returned false immediately without redoing the work.
        #expect(result1 != result2)
        #expect(state.isLocked == false)
    }

    // MARK: - jottery-vn6f: single reload in forceFullSync

    /// `SyncService.forceFullSync()` must not fire its wired postSyncHandler
    /// on success — that used to trigger a second, redundant `loadNotes()`
    /// on top of `AppState.forceFullSync()`'s own explicit reload. Runs the
    /// full cycle for real (stubbed network) rather than only the failure
    /// path, so a regression that re-adds the call would be caught even
    /// though it sits after every throwing step.
    @Test func forceFullSyncDoesNotInvokePostSyncHandler() async throws {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-force-full-sync-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let db = try DatabaseManager(path: dir.appendingPathComponent("test.db").path)
        let versionRepo = VersionRepository(db: db)
        let noteRepo = NoteRepository(db: db, versionRepo: versionRepo)
        let syncRepo = SyncRepository(db: db)
        let attachmentRepo = AttachmentRepository(db: db)
        let savedSearchRepo = SavedSearchRepository(db: db)
        let key = SymmetricKey(data: Data(repeating: 3, count: 32))

        // No notes need pushing, so push() makes zero network calls; the
        // pull loop makes exactly one, which this stub answers with an
        // empty, single-page response.
        ForceFullSyncStubURLProtocol.body = Data(#"{"notes":[],"attachments":[],"versions":[]}"#.utf8)
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [ForceFullSyncStubURLProtocol.self]
        let syncClient = SyncClient(
            endpoint: "https://sync-orchestration-tests.example.org",
            apiKey: "test-key",
            configuration: config
        )
        let service = SyncService(
            syncClient: syncClient,
            noteRepo: noteRepo,
            syncRepo: syncRepo,
            versionRepo: versionRepo,
            attachmentRepo: attachmentRepo,
            savedSearchRepo: savedSearchRepo,
            key: key
        )

        actor Counter {
            private(set) var calls = 0
            func bump() { calls += 1 }
        }
        let counter = Counter()
        await service.setPostSyncHandler { _ in await counter.bump() }

        try await service.forceFullSync()

        let calls = await counter.calls
        #expect(calls == 0)
    }
}
