import Foundation
import Testing

@testable import Jottery

/// Pure-function coverage for `SyncService.chunked(_:size:)` — the chunking
/// arithmetic `push()` uses to split `listNeedingSync()` into sequential
/// requests of at most `pushChunkSize` notes (jottery-l9dt). Covered here
/// directly (empty / exact multiple / remainder) rather than through a full
/// push() round-trip, mirroring the established pattern for other pure
/// helpers on SyncService (see `collectRepairAttachments` in
/// SyncAttachmentWarningTests.swift).
struct PushChunkingTests {

    /// Build `count` distinct records; only identity (id) matters for these
    /// tests, so content/tags can be empty placeholders.
    private func makeRecords(_ count: Int) -> [NoteRecord] {
        (0..<count).map { i in
            NoteRecord.new(encryptedContent: "{}", encryptedTags: "{}").with { $0.id = "note-\(i)" }
        }
    }

    @Test func emptyInputProducesNoChunks() {
        let chunks = SyncService.chunked([], size: 50)
        #expect(chunks.isEmpty)
    }

    @Test func exactMultipleSplitsIntoEvenChunks() {
        let records = makeRecords(100)
        let chunks = SyncService.chunked(records, size: 50)

        #expect(chunks.count == 2)
        #expect(chunks[0].count == 50)
        #expect(chunks[1].count == 50)
        // Order preserved and every record accounted for exactly once.
        #expect(chunks[0].map(\.id) == records[0..<50].map(\.id))
        #expect(chunks[1].map(\.id) == records[50..<100].map(\.id))
    }

    @Test func remainderProducesTrailingPartialChunk() {
        let records = makeRecords(125)
        let chunks = SyncService.chunked(records, size: 50)

        #expect(chunks.count == 3)
        #expect(chunks[0].count == 50)
        #expect(chunks[1].count == 50)
        #expect(chunks[2].count == 25)
        #expect(chunks.flatMap(\.self).map(\.id) == records.map(\.id))
    }

    @Test func fewerRecordsThanChunkSizeProducesSingleChunk() {
        let records = makeRecords(3)
        let chunks = SyncService.chunked(records, size: 50)

        #expect(chunks.count == 1)
        #expect(chunks[0].count == 3)
    }

    @Test func chunkSizeOfOneProducesOneChunkPerRecord() {
        let records = makeRecords(4)
        let chunks = SyncService.chunked(records, size: 1)

        #expect(chunks.count == 4)
        #expect(chunks.allSatisfy { $0.count == 1 })
    }

    /// Defensive: a non-positive size must not infinite-loop or crash —
    /// falls back to a single chunk containing everything.
    @Test func nonPositiveSizeFallsBackToSingleChunk() {
        let records = makeRecords(5)
        let chunks = SyncService.chunked(records, size: 0)

        #expect(chunks.count == 1)
        #expect(chunks[0].count == 5)
    }

    @Test func defaultPushChunkSizeIs50() {
        #expect(SyncService.pushChunkSize == 50)
    }
}

private extension NoteRecord {
    /// Small builder helper so tests can tweak one field without repeating
    /// the full memberwise initialiser.
    func with(_ mutate: (inout NoteRecord) -> Void) -> NoteRecord {
        var copy = self
        mutate(&copy)
        return copy
    }
}
