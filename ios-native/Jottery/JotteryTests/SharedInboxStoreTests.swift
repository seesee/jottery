import Foundation
import Testing

@testable import Jottery

struct SharedInboxStoreTests {

    private func makeRoot() throws -> URL {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("shared-inbox-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        return root
    }

    @Test func writeThenReadRoundTrip() throws {
        let root = try makeRoot()
        let imageData = Data([0x89, 0x50, 0x4E, 0x47])

        try SharedInboxStore.write(
            text: "Shared note text",
            urls: ["https://example.com"],
            files: [(data: imageData, filename: "photo.png", mimeType: "image/png")],
            in: root
        )

        let items = SharedInboxStore.pendingItems(in: root)
        #expect(items.count == 1)
        let item = try #require(items.first)
        #expect(item.manifest.text == "Shared note text")
        #expect(item.manifest.urls == ["https://example.com"])
        #expect(item.manifest.files.count == 1)
        let file = try #require(item.manifest.files.first)
        #expect(file.filename == "photo.png")
        #expect(file.mimeType == "image/png")
        let storedData = try Data(contentsOf: item.directory.appendingPathComponent(file.storedName))
        #expect(storedData == imageData)
    }

    @Test func removeDeletesTheItem() throws {
        let root = try makeRoot()
        try SharedInboxStore.write(text: "bye", urls: [], files: [], in: root)
        let item = try #require(SharedInboxStore.pendingItems(in: root).first)

        SharedInboxStore.remove(item.directory)

        #expect(SharedInboxStore.pendingItems(in: root).isEmpty)
    }

    @Test func pendingItemsIgnoresCorruptEntries() throws {
        let root = try makeRoot()
        let junk = root.appendingPathComponent("junk", isDirectory: true)
        try FileManager.default.createDirectory(at: junk, withIntermediateDirectories: true)
        try Data("not json".utf8).write(to: junk.appendingPathComponent("manifest.json"))

        #expect(SharedInboxStore.pendingItems(in: root).isEmpty)
    }

    // MARK: - Backups (R2-093)

    /// File protection doesn't reach backup contents, and this is plaintext.
    @Test func stagedItemsAreKeptOutOfDeviceBackups() throws {
        let root = try makeRoot()

        try SharedInboxStore.write(text: "a reset link", urls: [], files: [], in: root)

        #expect(try root.resourceValues(forKeys: [.isExcludedFromBackupKey]).isExcludedFromBackup == true)
    }

    // MARK: - Nothing left behind (jottery-sa51)

    @Test func aFileIsCopiedInWithoutBeingReadIntoMemory() throws {
        let root = try makeRoot()
        let source = root.appendingPathComponent("source.bin")
        try Data([1, 2, 3]).write(to: source)

        let staging = try SharedInboxStore.begin(in: root)
        let file = try staging.add(copying: source, index: 0, filename: "report.pdf", mimeType: "application/pdf")
        try staging.commit(text: nil, urls: [], files: [file])

        let item = try #require(SharedInboxStore.pendingItems(in: root).first)
        #expect(file.storedName == "file-0.pdf")
        #expect(try Data(contentsOf: item.directory.appendingPathComponent(file.storedName)) == Data([1, 2, 3]))
    }

    @Test func aFileThatCantBeCopiedThrowsAndCanBeDiscarded() throws {
        let root = try makeRoot()
        let staging = try SharedInboxStore.begin(in: root)

        #expect(throws: (any Error).self) {
            _ = try staging.add(
                copying: root.appendingPathComponent("missing"), index: 0, filename: "a.txt", mimeType: "text/plain"
            )
        }
        staging.discard()

        #expect(!FileManager.default.fileExists(atPath: staging.directory.path))
    }

    @Test func anItemWithoutAManifestIsInvisibleAndSweptOnceItIsOld() throws {
        let root = try makeRoot()
        let staging = try SharedInboxStore.begin(in: root)
        _ = try staging.add(data: Data("secret".utf8), index: 0, filename: "a.txt", mimeType: "text/plain")
        #expect(SharedInboxStore.pendingItems(in: root).isEmpty)

        // It might still be being written.
        SharedInboxStore.removeAbandoned(olderThan: 3600, in: root)
        #expect(FileManager.default.fileExists(atPath: staging.directory.path))

        SharedInboxStore.removeAbandoned(olderThan: -1, in: root)
        #expect(!FileManager.default.fileExists(atPath: staging.directory.path))
    }

    @Test func sweepingLeavesFinishedItemsAlone() throws {
        let root = try makeRoot()
        try SharedInboxStore.write(text: "keep me", urls: [], files: [], in: root)

        SharedInboxStore.removeAbandoned(olderThan: -1, in: root)

        #expect(SharedInboxStore.pendingItems(in: root).count == 1)
    }

    /// A shared file is plaintext until the app imports it.
    @Test func aCopiedFileIsProtected() throws {
        let root = try makeRoot()
        let source = root.appendingPathComponent("source.txt")
        try Data("shared".utf8).write(to: source)
        let staging = try SharedInboxStore.begin(in: root)

        var protected: [URL] = []
        let file = try staging.add(copying: source, index: 0, filename: "a.txt", mimeType: "text/plain") {
            protected.append($0)
        }

        #expect(protected == [staging.directory.appendingPathComponent(file.storedName)])
    }

    @Test func removeAllTakesEverything() throws {
        let root = try makeRoot()
        try SharedInboxStore.write(text: "gone", urls: [], files: [], in: root)

        SharedInboxStore.removeAll(in: root)

        #expect(SharedInboxStore.pendingItems(in: root).isEmpty)
    }
}
