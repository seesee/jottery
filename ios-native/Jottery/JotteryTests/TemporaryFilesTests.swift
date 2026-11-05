import Foundation
import Testing

@testable import Jottery

/// Decrypted exports and attachments used to stay in tmp indefinitely, past a
/// lock and even Delete Everything.
@MainActor
struct TemporaryFilesTests {

    private func makeFiles() -> TemporaryFiles {
        TemporaryFiles(root: FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-tempfiles-tests-\(UUID().uuidString)", isDirectory: true))
    }

    private func exists(_ url: URL) -> Bool {
        FileManager.default.fileExists(atPath: url.path)
    }

    @Test func aFileKeepsItsNameInAFolderOfItsOwn() throws {
        let files = makeFiles()

        let first = try files.write(Data("a".utf8), named: "report.pdf")
        let second = try files.write(Data("b".utf8), named: "report.pdf")

        #expect(first.lastPathComponent == "report.pdf")
        #expect(first != second)
        #expect(try Data(contentsOf: first) == Data("a".utf8))
    }

    /// The hostile name from a pull: it must land inside our folder, not on the vault.
    @Test func aHostileNameCantLeaveTheFolder() throws {
        let files = makeFiles()

        let url = try files.write(Data("a".utf8), named: "../../../Library/Application Support/Jottery/jottery.db")

        #expect(url.lastPathComponent == "jottery.db")
        #expect(url.deletingLastPathComponent().deletingLastPathComponent().standardizedFileURL
            == files.root.standardizedFileURL)
    }

    @Test func aCopyHoldsTheSameBytes() throws {
        let files = makeFiles()
        let source = try files.write(Data("picked photo".utf8), named: "source.jpg")

        let copy = try files.copy(source, named: "../IMG_0001.jpg")

        #expect(copy.lastPathComponent == "IMG_0001.jpg")
        #expect(try Data(contentsOf: copy) == Data("picked photo".utf8))
    }

    @Test func aCreatedFileCanBeWrittenToAsItGrows() throws {
        let files = makeFiles()
        let url = try files.createFile(named: "export.json")

        let handle = try FileHandle(forWritingTo: url)
        try handle.write(contentsOf: Data("streamed".utf8))
        try handle.close()

        #expect(try Data(contentsOf: url) == Data("streamed".utf8))
    }

    @Test func removingAFileRemovesItsFolder() throws {
        let files = makeFiles()
        let url = try files.write(Data("a".utf8), named: "note.json")

        files.remove(url)

        #expect(!exists(url))
        #expect(!exists(url.deletingLastPathComponent()))
    }

    @Test func removeOnlyTouchesItsOwnFiles() throws {
        let files = makeFiles()
        let outside = FileManager.default.temporaryDirectory.appendingPathComponent("jottery-outside-\(UUID().uuidString)")
        try Data("a".utf8).write(to: outside)
        defer { try? FileManager.default.removeItem(at: outside) }

        files.remove(outside)

        #expect(exists(outside))
    }

    @Test func lockingSweepsThemAway() async throws {
        let state = try TestVaults.makeState()
        state.temporaryFiles = makeFiles()
        try await state.createVault(password: "notes-password-correct-horse")
        let url = try state.temporaryFiles.write(Data("decrypted".utf8), named: "passport-scan.jpg")

        state.lock()

        #expect(!exists(url))
    }

    @Test func leftoversFromOlderBuildsAreRemoved() throws {
        let folder = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-leftovers-tests-\(UUID().uuidString)", isDirectory: true)
        let attachments = folder.appendingPathComponent("jottery-attachments", isDirectory: true)
        try FileManager.default.createDirectory(at: attachments, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: folder) }
        let leftovers = [
            attachments.appendingPathComponent("passport-scan.jpg"),
            folder.appendingPathComponent("jottery-export-2026-01-01.json"),
            folder.appendingPathComponent("jottery-backup-2026-01-01.jottery"),
            folder.appendingPathComponent("jottery-restore-ABC.jottery"),
        ]
        let unrelated = folder.appendingPathComponent("something-else.json")
        for url in leftovers + [unrelated] { try Data("a".utf8).write(to: url) }

        TemporaryFiles.removeLeftovers(in: folder)

        #expect(leftovers.allSatisfy { !exists($0) })
        #expect(exists(unrelated))
    }
}
