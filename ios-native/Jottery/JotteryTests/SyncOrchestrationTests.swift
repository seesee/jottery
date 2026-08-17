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
/// 2. `AppState` gains a monotonically increasing ticket counter shared by
///    *both* `loadNotes()` (full reload) and `applySyncChanges(_:)`
///    (incremental) — each draws its own ticket at the start, before any
///    await, and publishes only if its ticket is newer than the last
///    published one (`publishIfNewest(ticket:_:)`). Publish order, not
///    operation kind, decides which refresh wins when two race — fixing a
///    gap in the old epoch scheme, where only full reloads bumped the
///    counter, so a full reload that *started* before a concurrent
///    incremental could still publish *after* it and clobber the
///    incremental's fresher rows. A dropped publication (of either kind)
///    schedules exactly one follow-up full reload via
///    `retryFullReloadIfNeeded()`, guarded by `isRetryReloadPending` so
///    concurrent drops don't cascade. Exercised through the
///    `publishIfNewest(ticket:_:)` seam directly, plus deterministic
///    ticket-prediction sequences through the real
///    `loadNotes()`/`applySyncChanges()` entry points — rather than by
///    racing real `Task`s, which is inherently timing-dependent and flaky.
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

    // MARK: - jottery-t94v: publish-ordering guard

    @Test func publishIfNewestAppliesWhenTicketIsNewer() throws {
        let state = try makeUnlockedState()

        var ran = false
        // A fresh state has never published — any positive ticket is newer.
        let applied = state.publishIfNewest(ticket: 1) { ran = true }

        #expect(applied == true)
        #expect(ran == true)
    }

    @Test func publishIfNewestSkipsWhenTicketIsNotNewer() throws {
        let state = try makeUnlockedState()
        var firstRan = false
        #expect(state.publishIfNewest(ticket: 5) { firstRan = true } == true)
        #expect(firstRan == true)

        // Equal ticket: a caller whose ticket ties the last published one
        // must not publish — `>` not `>=`, so ties always lose.
        var tieRan = false
        #expect(state.publishIfNewest(ticket: 5) { tieRan = true } == false)
        #expect(tieRan == false)

        // Older ticket: a caller that started before the last publish.
        var staleRan = false
        #expect(state.publishIfNewest(ticket: 4) { staleRan = true } == false)
        #expect(staleRan == false)
    }

    /// The bug this seam fixes: the old epoch scheme only bumped on full
    /// reloads, so a full reload that *started* before a concurrent
    /// incremental could still publish *after* it and clobber the
    /// incremental's fresher rows with an older snapshot. Simulates that
    /// ordering directly at the seam — deterministic, no real races — by
    /// publishing the "incremental" (ticket 2) first and then attempting to
    /// publish the "full reload" (ticket 1, i.e. it started earlier).
    @Test func olderTicketFullReloadIsDroppedAfterNewerTicketIncrementalPublished() throws {
        let state = try makeUnlockedState()

        var incrementalRan = false
        #expect(state.publishIfNewest(ticket: 2) { incrementalRan = true } == true)
        #expect(incrementalRan == true)

        var fullReloadRan = false
        let fullReloadPublished = state.publishIfNewest(ticket: 1) { fullReloadRan = true }

        #expect(fullReloadPublished == false)
        #expect(fullReloadRan == false)
    }

    /// Sequential, non-racing calls are the common case and must be
    /// unaffected by the ordering change: each call draws a strictly newer
    /// ticket than the last, so each one publishes in turn. A note created
    /// between the two calls must show up after the second, which would
    /// only fail to happen if the second call's ticket were somehow not
    /// newer than the first's.
    @Test func sequentialLoadNotesCallsEachPublish() async throws {
        let state = try makeUnlockedState()
        let noteRepo = try #require(state.noteRepo)
        let key = try #require(state.keyManager.masterKey)

        try await state.loadNotes()
        #expect(state.notes.isEmpty)

        _ = try noteRepo.create(content: "second load", tags: [], key: key)
        try await state.loadNotes()

        #expect(state.notes.contains { $0.content == "second load" })
    }

    @Test func applySyncChangesPublishesNormallyWhenUnraced() async throws {
        let state = try makeUnlockedState()
        let noteRepo = try #require(state.noteRepo)
        let key = try #require(state.keyManager.masterKey)

        let note = try noteRepo.create(content: "original", tags: [], key: key)
        try await state.loadNotes()

        var updated = note
        updated.content = "updated by server"
        try noteRepo.update(updated, key: key)

        await state.applySyncChanges(SyncChanges(updatedIds: [note.id]))

        #expect(state.notes.first(where: { $0.id == note.id })?.content == "updated by server")
    }

    @Test func applySyncChangesFullReloadFallbackPublishes() async throws {
        let state = try makeUnlockedState()
        let noteRepo = try #require(state.noteRepo)
        let key = try #require(state.keyManager.masterKey)
        let note = try noteRepo.create(content: "note", tags: [], key: key)
        try await state.loadNotes()

        // .fullReload routes through loadNotes() internally.
        await state.applySyncChanges(.fullReload)

        #expect(state.notes.contains { $0.id == note.id })
    }

    /// End-to-end version of `olderTicketFullReloadIsDroppedAfterNewerTicketIncrementalPublished`,
    /// driven through the real `loadNotes()` entry point instead of the
    /// seam, to also exercise the automatic retry. Ticket values drawn by
    /// `nextLoadTicket()` are deterministic (one per real
    /// `loadNotes()`/`applySyncChanges()` call, in call order), so after one
    /// priming `loadNotes()` call the next ticket it will draw is
    /// predictable — letting a manual `publishIfNewest` call simulate "a
    /// newer refresh already published" without racing real `Task`s.
    @Test func olderTicketLoadNotesIsDroppedThenRetryReloadConverges() async throws {
        let state = try makeUnlockedState()
        let noteRepo = try #require(state.noteRepo)
        let key = try #require(state.keyManager.masterKey)

        try await state.loadNotes()
        // One real load has happened, so loadNotes() below will draw ticket 2.

        let note = try noteRepo.create(content: "from db", tags: [], key: key)

        // Simulate a newer refresh (ticket 2) publishing first — exactly the
        // ticket the loadNotes() call below is about to draw for itself, so
        // its own publish attempt is guaranteed to be dropped.
        var raceWinnerRan = false
        #expect(state.publishIfNewest(ticket: 2) { raceWinnerRan = true } == true)
        #expect(raceWinnerRan == true)

        // Dropped (ticket 2 == last published 2); the automatic retry draws
        // ticket 3, which is newer, and converges to the real DB state.
        try await state.loadNotes()

        #expect(state.notes.contains { $0.id == note.id })
    }

    /// Same shape as the loadNotes() case above, but for a dropped
    /// *incremental*: an incremental only covers the ids it was told about,
    /// so if the refresh that beat it wasn't itself a full reload, ids
    /// outside its scope could stay stale forever without the retry
    /// fallback. `noteB` here is never mentioned in the `SyncChanges` passed
    /// to `applySyncChanges` — only the follow-up full reload picks it up.
    @Test func olderTicketApplySyncChangesIsDroppedThenRetryReloadConverges() async throws {
        let state = try makeUnlockedState()
        let noteRepo = try #require(state.noteRepo)
        let key = try #require(state.keyManager.masterKey)

        let noteA = try noteRepo.create(content: "original", tags: [], key: key)
        try await state.loadNotes()
        // One real load has happened, so applySyncChanges() below will draw
        // ticket 2.

        var updatedA = noteA
        updatedA.content = "updated by server"
        try noteRepo.update(updatedA, key: key)

        // Out-of-band row that lands in the DB but is never passed to
        // applySyncChanges() below — only a full reload would pick it up.
        let noteB = try noteRepo.create(content: "out of band", tags: [], key: key)

        var raceWinnerRan = false
        #expect(state.publishIfNewest(ticket: 2) { raceWinnerRan = true } == true)
        #expect(raceWinnerRan == true)

        await state.applySyncChanges(SyncChanges(updatedIds: [noteA.id]))

        // Dropped (ticket 2 == last published 2); the automatic retry draws
        // ticket 3, converging on both the server-side edit to noteA and
        // the out-of-band noteB that this incremental never asked about.
        #expect(state.notes.first(where: { $0.id == noteA.id })?.content == "updated by server")
        #expect(state.notes.contains { $0.id == noteB.id })
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
