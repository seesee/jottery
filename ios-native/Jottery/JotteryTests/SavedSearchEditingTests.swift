import CryptoKit
import Foundation
import Testing

@testable import Jottery

/// Answers every push and keeps what was sent. Scoped to this file, as swift-testing
/// runs suites side by side and the shared stubs hold their state in statics.
private final class SavedSearchPushStub: URLProtocol, @unchecked Sendable {
    nonisolated(unsafe) static var pushed: [SyncSavedSearch] = []

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        let body = request.httpBody ?? Self.read(request.httpBodyStream)
        if let body, let decoded = try? JSONDecoder().decode(SyncPushRequest.self, from: body) {
            Self.pushed += decoded.savedSearches ?? []
        }
        let response = SyncPushResponse(accepted: [], rejected: [], errors: nil, attachmentWarnings: nil)
        let http = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: nil)!
        client?.urlProtocol(self, didReceive: http, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: (try? JSONEncoder().encode(response)) ?? Data())
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}

    private static func read(_ stream: InputStream?) -> Data? {
        guard let stream else { return nil }
        stream.open()
        defer { stream.close() }
        var data = Data()
        var buffer = [UInt8](repeating: 0, count: 4096)
        while stream.hasBytesAvailable {
            let count = stream.read(&buffer, maxLength: buffer.count)
            guard count > 0 else { break }
            data.append(buffer, count: count)
        }
        return data
    }
}

/// Saved searches sync through their `order`, and last write wins per search. So a
/// reorder has to reach the server as a change to each search it moved, and leave
/// the others alone.
@MainActor
@Suite(.serialized)
struct SavedSearchEditingTests {

    private func makeUnlockedState() throws -> AppState {
        let state = try TestVaults.makeState()
        state.keyManager.unlockWithKeyData(Data(repeating: 7, count: 32))
        state.isLocked = false
        return state
    }

    /// Searches that have all reached the server.
    private func makeSyncedSearches(_ names: [String], in state: AppState) throws -> SavedSearchRepository {
        let repo = try #require(state.savedSearchRepo)
        for name in names {
            try state.saveSearch(name: name, query: "#\(name)")
        }
        for search in state.savedSearches {
            try repo.markSynced(id: search.id, syncedAt: Date().iso8601)
        }
        return repo
    }

    private func reloaded(_ state: AppState) throws -> [SavedSearchRepository.SavedSearch] {
        let key = try #require(state.keyManager.masterKey)
        return try #require(state.savedSearchRepo).listAll(key: key)
    }

    // MARK: - Editing

    @Test func editingChangesTheNameAndQuery() throws {
        let state = try makeUnlockedState()
        let repo = try makeSyncedSearches(["work"], in: state)
        let search = try #require(state.savedSearches.first)

        try state.updateSavedSearch(id: search.id, name: "Office", query: "#office -lunch")

        #expect(state.savedSearches.first?.name == "Office")
        #expect(state.savedSearches.first?.query == "#office -lunch")
        let stored = try #require(try reloaded(state).first)
        #expect(stored.name == "Office")
        #expect(stored.query == "#office -lunch")
        #expect(stored.version == search.version + 1)
        #expect(try repo.listNeedingSync().map(\.id) == [search.id])
    }

    /// The point of editing rather than deleting and adding again.
    @Test func editingLeavesASearchWhereItIs() throws {
        let state = try makeUnlockedState()
        _ = try makeSyncedSearches(["one", "two", "three"], in: state)

        try state.updateSavedSearch(id: state.savedSearches[1].id, name: "two!", query: "#two")

        #expect(try reloaded(state).map(\.name) == ["one", "two!", "three"])
    }

    @Test func whatIsEditedStaysEncrypted() throws {
        let state = try makeUnlockedState()
        let repo = try makeSyncedSearches(["work"], in: state)
        let search = try #require(state.savedSearches.first)

        try state.updateSavedSearch(id: search.id, name: "A private name", query: "#private")

        let raw = try #require(try repo.getRaw(id: search.id))
        #expect(!raw.name.contains("private"))
        #expect(!raw.query.contains("private"))
    }

    @Test func nothingIsEncryptedWhileTheVaultIsBeingReKeyed() throws {
        let state = try makeUnlockedState()
        _ = try makeSyncedSearches(["work"], in: state)
        let search = try #require(state.savedSearches.first)
        state.isReKeying = true

        #expect(throws: AppStateError.self) {
            try state.updateSavedSearch(id: search.id, name: "Office", query: "#office")
        }

        state.isReKeying = false
        #expect(try reloaded(state).first?.name == "work")
    }

    // MARK: - Reordering

    @Test func aMoveLastsAcrossAReload() throws {
        let state = try makeUnlockedState()
        _ = try makeSyncedSearches(["one", "two", "three"], in: state)

        try state.moveSavedSearches(fromOffsets: IndexSet(integer: 2), toOffset: 0)

        #expect(state.savedSearches.map(\.name) == ["three", "one", "two"])
        #expect(try reloaded(state).map(\.name) == ["three", "one", "two"])
    }

    @Test func onlyTheSearchesThatMovedAreQueued() throws {
        let state = try makeUnlockedState()
        let repo = try makeSyncedSearches(["one", "two", "three", "four"], in: state)
        let ids = state.savedSearches.map(\.id)
        let untouched = try #require(try repo.getRaw(id: ids[3]))

        try state.moveSavedSearches(fromOffsets: IndexSet(integer: 0), toOffset: 2)

        #expect(state.savedSearches.map(\.name) == ["two", "one", "three", "four"])
        #expect(Set(try repo.listNeedingSync().map(\.id)) == [ids[0], ids[1]])
        let after = try #require(try repo.getRaw(id: ids[3]))
        #expect(after.modifiedAt == untouched.modifiedAt)
        #expect(after.version == untouched.version)
    }

    /// Last write wins on `modified_at`, here and on the server, so a change that
    /// didn't move it would lose to the copy it replaces.
    @Test func aMovedSearchIsMarkedAsChanged() throws {
        let state = try makeUnlockedState()
        let repo = try makeSyncedSearches(["one", "two"], in: state)
        let id = state.savedSearches[0].id
        try TestVaults.execute(
            state, "UPDATE saved_searches SET modified_at = ?", ["2020-01-01T00:00:00.000Z"]
        )
        let before = try #require(try repo.getRaw(id: id))

        try state.moveSavedSearches(fromOffsets: IndexSet(integer: 0), toOffset: 2)

        let after = try #require(try repo.getRaw(id: id))
        #expect(Date.isLater(after.modifiedAt, than: before.modifiedAt))
        #expect(after.version == before.version + 1)
        #expect(after.needsSync)
    }

    @Test func aMoveThatChangesNothingQueuesNothing() throws {
        let state = try makeUnlockedState()
        let repo = try makeSyncedSearches(["one", "two"], in: state)

        try state.moveSavedSearches(fromOffsets: IndexSet(integer: 0), toOffset: 1)

        #expect(try repo.listNeedingSync().isEmpty)
    }

    /// Two devices that each add a search offline give them the same order.
    @Test func aTieIsBrokenByTheMove() throws {
        let state = try makeUnlockedState()
        _ = try makeSyncedSearches(["one", "two", "three"], in: state)
        try TestVaults.execute(state, "UPDATE saved_searches SET display_order = 5", [])
        state.savedSearches = try reloaded(state)
        let expected = [state.savedSearches[1].name, state.savedSearches[0].name, state.savedSearches[2].name]

        try state.moveSavedSearches(fromOffsets: IndexSet(integer: 0), toOffset: 2)

        #expect(try reloaded(state).map(\.name) == expected)
    }

    @Test func aDeletedSearchKeepsItsPlaceOutOfIt() throws {
        let state = try makeUnlockedState()
        _ = try makeSyncedSearches(["one", "two", "three"], in: state)
        try state.deleteSavedSearch(id: state.savedSearches[1].id)

        try state.moveSavedSearches(fromOffsets: IndexSet(integer: 1), toOffset: 0)

        #expect(try reloaded(state).map(\.name) == ["three", "one"])
    }

    // MARK: - On the Wire

    @Test func aMoveReachesTheServerAsNewOrders() async throws {
        let state = try makeUnlockedState()
        _ = try makeSyncedSearches(["one", "two", "three"], in: state)
        try state.moveSavedSearches(fromOffsets: IndexSet(integer: 2), toOffset: 0)
        let moved = state.savedSearches

        SavedSearchPushStub.pushed = []
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [SavedSearchPushStub.self]
        let service = SyncService(
            syncClient: SyncClient(endpoint: "https://saved-search-tests.example.org", apiKey: "test-key", configuration: config),
            noteRepo: try #require(state.noteRepo),
            syncRepo: try #require(state.syncRepo),
            versionRepo: try #require(state.versionRepo),
            attachmentRepo: try #require(state.attachmentRepo),
            savedSearchRepo: try #require(state.savedSearchRepo),
            key: try #require(state.keyManager.masterKey)
        )

        _ = try await service.push()

        let sent = SavedSearchPushStub.pushed.sorted { $0.order < $1.order }
        #expect(sent.map(\.id) == moved.map(\.id))
        #expect(try #require(state.savedSearchRepo).listNeedingSync().isEmpty)
    }
}
