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
}
