import CryptoKit
import Foundation
import Testing

@testable import Jottery

/// Stub transport scoped to this file (same rationale as
/// `ForceFullSyncStubURLProtocol` in SyncOrchestrationTests.swift: swift-testing
/// can run top-level suites concurrently, and the shared `StubURLProtocol`
/// drives its state through file-scoped statics, so a dedicated stub avoids
/// cross-suite flakiness). Unlike the existing single-fixed-response stubs,
/// this one inspects each request's body and answers per call — call 1
/// succeeds (echoing back every note id it received as accepted), call 2
/// fails outright, simulating a connection dropped mid-batch after the first
/// chunk already landed.
private final class ChunkFailureStubURLProtocol: URLProtocol, @unchecked Sendable {
    nonisolated(unsafe) static var callCount = 0
    nonisolated(unsafe) static var requestedNoteIdsByCall: [[String]] = []

    static func reset() {
        callCount = 0
        requestedNoteIdsByCall = []
    }

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        Self.callCount += 1
        let callIndex = Self.callCount

        let bodyData = request.httpBody ?? Self.readBodyStream(request.httpBodyStream)
        let noteIds: [String]
        if let bodyData, let decoded = try? JSONDecoder().decode(SyncPushRequest.self, from: bodyData) {
            noteIds = decoded.notes.map(\.id)
        } else {
            noteIds = []
        }
        Self.requestedNoteIdsByCall.append(noteIds)

        if callIndex == 1 {
            // First chunk: accept every note it carried.
            let accepted = noteIds.map { SyncAccepted(id: $0, serverVersion: 1, syncedAt: nil) }
            let response = SyncPushResponse(accepted: accepted, rejected: [], errors: nil, attachmentWarnings: nil)
            let body = (try? JSONEncoder().encode(response)) ?? Data()
            let httpResponse = HTTPURLResponse(
                url: request.url!, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: nil
            )!
            client?.urlProtocol(self, didReceive: httpResponse, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: body)
            client?.urlProtocolDidFinishLoading(self)
        } else {
            // Second (and any later) chunk: fail outright.
            let httpResponse = HTTPURLResponse(
                url: request.url!, statusCode: 500, httpVersion: "HTTP/1.1", headerFields: nil
            )!
            client?.urlProtocol(self, didReceive: httpResponse, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: Data("simulated network failure".utf8))
            client?.urlProtocolDidFinishLoading(self)
        }
    }

    override func stopLoading() {}

    private static func readBodyStream(_ stream: InputStream?) -> Data? {
        guard let stream else { return nil }
        stream.open()
        defer { stream.close() }
        var data = Data()
        let bufferSize = 4096
        var buffer = [UInt8](repeating: 0, count: bufferSize)
        while stream.hasBytesAvailable {
            let read = stream.read(&buffer, maxLength: bufferSize)
            if read > 0 {
                data.append(buffer, count: read)
            } else {
                break
            }
        }
        return data
    }
}

/// Service-level coverage for the chunked-push contract (jottery-l9dt):
/// when a later chunk fails, chunks that already got a response must stay
/// durably marked synced rather than the whole push rolling back. Achievable
/// here without new mock infra beyond a per-file stub `URLProtocol` — the
/// same pattern `SyncOrchestrationTests`/`SyncClientTransportTests` already
/// use — extended to answer per call instead of with one fixed response.
struct PushChunkingServiceTests {

    @Test func chunkFailureLeavesEarlierChunksMarkedSyncedAndAbortsRemaining() async throws {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-push-chunking-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let db = try DatabaseManager(path: dir.appendingPathComponent("test.db").path)
        let versionRepo = VersionRepository(db: db)
        let noteRepo = NoteRepository(db: db, versionRepo: versionRepo)
        let syncRepo = SyncRepository(db: db)
        let attachmentRepo = AttachmentRepository(db: db)
        let savedSearchRepo = SavedSearchRepository(db: db)
        let key = SymmetricKey(data: Data(repeating: 9, count: 32))

        // 55 notes needing sync forces exactly two chunks at the default
        // pushChunkSize (50): a full 50-note chunk, then a 5-note remainder.
        var noteIds: [String] = []
        for i in 0..<55 {
            let note = try noteRepo.create(content: "note \(i)", tags: [], key: key)
            noteIds.append(note.id)
        }
        #expect(try noteRepo.listNeedingSync().count == 55)

        ChunkFailureStubURLProtocol.reset()
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [ChunkFailureStubURLProtocol.self]
        let syncClient = SyncClient(
            endpoint: "https://push-chunking-tests.example.org",
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

        await #expect(throws: (any Error).self) {
            try await service.push()
        }

        // Exactly two chunk requests were sent: the first (50 notes)
        // succeeded, the second (5 notes) failed and aborted the loop —
        // no third request was ever made.
        #expect(ChunkFailureStubURLProtocol.callCount == 2)
        let requestSizes = ChunkFailureStubURLProtocol.requestedNoteIdsByCall.map(\.count)
        #expect(requestSizes == [50, 5])

        let firstChunkIds = Set(ChunkFailureStubURLProtocol.requestedNoteIdsByCall[0])
        let secondChunkIds = Set(ChunkFailureStubURLProtocol.requestedNoteIdsByCall[1])
        #expect(firstChunkIds.count == 50)
        #expect(secondChunkIds.count == 5)
        #expect(firstChunkIds.isDisjoint(with: secondChunkIds))
        #expect(firstChunkIds.union(secondChunkIds) == Set(noteIds))

        // Core guarantee under test: the accepted first chunk is durably
        // marked synced even though push() as a whole threw.
        for id in firstChunkIds {
            let record = try #require(try noteRepo.getRaw(id: id))
            #expect(record.needsSync == false)
        }
        // The never-sent second chunk is untouched — still pending so the
        // next sync attempt retries exactly the unsent tail.
        for id in secondChunkIds {
            let record = try #require(try noteRepo.getRaw(id: id))
            #expect(record.needsSync == true)
        }

        let stillNeeding = try noteRepo.listNeedingSync()
        #expect(stillNeeding.count == 5)
        #expect(Set(stillNeeding.map(\.id)) == secondChunkIds)
    }
}
