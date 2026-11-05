import CoreTransferable
import Foundation
import PhotosUI
import Runestone
import SwiftUI
import Testing
import UIKit
import UniformTypeIdentifiers

@testable import Jottery

/// Getting several things into a note at once, and what stops one going in (R2-144).
@MainActor
struct AttachmentImportTests {

    private func makeState() async throws -> (AppState, String) {
        let state = try TestVaults.makeState()
        try await state.createVault(password: "notes-password-correct-horse")
        let note = try #require(try state.createNote(content: "hello"))
        return (state, note.id)
    }

    /// A folder for one test's files, so a name can be asserted on.
    private func makeFolder() throws -> URL {
        let folder = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-import-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        return folder
    }

    private func makeFile(_ name: String, bytes: Int, in folder: URL) throws -> URL {
        let url = folder.appendingPathComponent(name)
        FileManager.default.createFile(atPath: url.path, contents: nil)
        let handle = try FileHandle(forWritingTo: url)
        // Sparse, so a file over the limit costs nothing to make.
        try handle.truncate(atOffset: UInt64(bytes))
        try handle.close()
        return url
    }

    /// Its own, because any other suite's vault locking sweeps the shared one.
    private func makeFiles() throws -> TemporaryFiles {
        TemporaryFiles(root: try makeFolder())
    }

    private func item(_ name: String, bytes: Int = 4, mimeType: String = "image/png") -> IncomingAttachment {
        IncomingAttachment(filename: name, mimeType: mimeType, contents: .data(Data(repeating: 1, count: bytes)))
    }

    // MARK: - Batches

    @Test func addsAWholeBatchInOrder() async throws {
        let (state, noteId) = try await makeState()

        let result = await state.addAttachments([item("a.png"), item("b.png"), item("c.png")], to: noteId)

        #expect(result.added.map(\.filename) == ["a.png", "b.png", "c.png"])
        #expect(result.problem == nil)
        let note = try #require(state.displayedNote(id: noteId))
        #expect(note.attachments.map(\.filename) == ["a.png", "b.png", "c.png"])
    }

    @Test func theThumbnailFieldStaysUnset() async throws {
        let (state, noteId) = try await makeState()
        _ = await state.addAttachments([item("a.png")], to: noteId)
        let note = try #require(state.displayedNote(id: noteId))
        #expect(note.attachments.first?.thumbnailData == nil)
    }

    @Test func aFileOverTheLimitDoesNotStopTheRest() async throws {
        let (state, noteId) = try await makeState()
        let folder = try makeFolder()
        let big = try makeFile("big.mov", bytes: AttachmentLimits.maxBytes + 1, in: folder)

        let result = await state.addAttachments(
            [item("a.png"), IncomingAttachment(pickedFile: big), item("b.png")],
            to: noteId
        )

        #expect(result.added.map(\.filename) == ["a.png", "b.png"])
        #expect(result.tooLarge == ["big.mov"])
        #expect(result.problem != nil)
        #expect(state.displayedNote(id: noteId)?.attachments.count == 2)
    }

    @Test func exactlyTheLimitIsAllowed() async throws {
        let (state, noteId) = try await makeState()
        let result = await state.addAttachments([item("max.bin", bytes: AttachmentLimits.maxBytes)], to: noteId)
        #expect(result.added.count == 1)
        #expect(result.tooLarge.isEmpty)
    }

    @Test func pastedDataOverTheLimitIsRefused() async throws {
        let (state, noteId) = try await makeState()
        let result = await state.addAttachments([item("big.png", bytes: AttachmentLimits.maxBytes + 1)], to: noteId)
        #expect(result.added.isEmpty)
        #expect(result.tooLarge == ["big.png"])
    }

    @Test func theLimitIsTheWebsTenMegabytes() {
        #expect(AttachmentLimits.maxBytes == 10 * 1024 * 1024)
    }

    @Test func anEmptyFileIsRefused() async throws {
        let (state, noteId) = try await makeState()
        let result = await state.addAttachments([item("empty.txt", bytes: 0)], to: noteId)
        #expect(result.added.isEmpty)
        #expect(result.empty == ["empty.txt"])
        #expect(result.problem != nil)
    }

    @Test func aFileThatCantBeReadIsReported() async throws {
        let (state, noteId) = try await makeState()
        let missing = try makeFolder().appendingPathComponent("gone.pdf")

        let result = await state.addAttachments([IncomingAttachment(pickedFile: missing), item("a.png")], to: noteId)

        #expect(result.failedCount == 1)
        #expect(result.added.map(\.filename) == ["a.png"])
        #expect(result.problem != nil)
    }

    @Test(arguments: [true, false])
    func aReadOnlyNoteTakesNothing(locked: Bool) async throws {
        let (state, noteId) = try await makeState()
        if locked {
            try state.toggleLock(id: noteId)
        } else {
            try state.archiveNote(id: noteId)
        }

        let result = await state.addAttachments([item("a.png")], to: noteId)

        #expect(result.added.isEmpty)
        #expect(result.failedCount == 1)
        #expect(state.displayedNote(id: noteId)?.attachments.isEmpty != false)
    }

    @Test func nothingIsLeftBehindInTemporaryFiles() async throws {
        let (state, noteId) = try await makeState()
        let folder = try makeFolder()
        let source = try makeFile("photo.png", bytes: 8, in: folder)
        let copied = IncomingAttachment.copying(source, filename: "photo.png", mimeType: "image/png", files: try makeFiles())
        guard case .temporaryFile(let copy, _) = copied.contents else {
            Issue.record("Expected a copy")
            return
        }
        #expect(FileManager.default.fileExists(atPath: copy.path))

        let result = await state.addAttachments([copied], to: noteId)

        #expect(result.added.count == 1)
        #expect(!FileManager.default.fileExists(atPath: copy.path))
    }

    @Test func aCopyIsRemovedEvenIfTheNoteRefusesIt() async throws {
        let (state, noteId) = try await makeState()
        try state.toggleLock(id: noteId)
        let source = try makeFile("photo.png", bytes: 8, in: try makeFolder())
        let copied = IncomingAttachment.copying(source, filename: "photo.png", mimeType: "image/png", files: try makeFiles())
        guard case .temporaryFile(let copy, _) = copied.contents else {
            Issue.record("Expected a copy")
            return
        }

        _ = await state.addAttachments([copied], to: noteId)

        #expect(!FileManager.default.fileExists(atPath: copy.path))
    }

    @Test func aFileOverTheLimitIsNeverCopied() throws {
        let big = try makeFile("big.mov", bytes: AttachmentLimits.maxBytes + 1, in: try makeFolder())
        let copied = IncomingAttachment.copying(big, filename: "big.mov", mimeType: "video/quicktime")
        guard case .refused(.tooLarge) = copied.contents else {
            Issue.record("Expected it to be refused")
            return
        }
    }

    // MARK: - Messages

    @Test func oneFileIsNamedAndSeveralAreCounted() {
        var one = AttachmentBatchResult()
        one.tooLarge = ["big.mov"]
        var two = AttachmentBatchResult()
        two.tooLarge = ["big.mov", "bigger.mov"]

        #expect(one.problem == L.attachmentsTooLarge("big.mov"))
        #expect(two.problem == L.attachmentsTooLargeCount(2))
    }

    // MARK: - Drops and the Pickers

    private func drop(_ provider: NSItemProvider) async throws -> DroppedAttachment {
        try await withCheckedThrowingContinuation { continuation in
            _ = provider.loadTransferable(type: DroppedAttachment.self) { continuation.resume(with: $0) }
        }
    }

    @Test func aDropKeepsTheFilesName() async throws {
        // As a drag from another app offers it: its data, and a name to go with it.
        let provider = NSItemProvider(item: Data([1, 2, 3]) as NSData, typeIdentifier: UTType.png.identifier)
        provider.suggestedName = "holiday photo.png"

        let dropped = try await drop(provider)
        defer { dropped.incoming.discard() }

        #expect(dropped.incoming.filename == "holiday photo.png")
        #expect(dropped.incoming.mimeType == "image/png")
    }

    /// A drop has to use the shared folder, where the lock can reach it, so another
    /// suite's vault locking can take a copy away. That's why this gets more than
    /// one go. Each is a whole new drop.
    @Test func aDropArrivesWhole() async throws {
        let root = TemporaryFiles.shared.root.standardizedFileURL.path
        let arrived = try await waitUntil {
            let provider = NSItemProvider(item: Data([1, 2, 3]) as NSData, typeIdentifier: UTType.png.identifier)
            let dropped = try await drop(provider)
            defer { dropped.incoming.discard() }
            guard case .temporaryFile(let copy, _) = dropped.incoming.contents else { return false }
            return copy.standardizedFileURL.path.hasPrefix(root) && (try? dropped.incoming.read()) == Data([1, 2, 3])
        }
        #expect(arrived)
    }

    @Test func aDropOverTheLimitIsRefusedWithoutCopying() async throws {
        let big = try makeFile("big.mov", bytes: AttachmentLimits.maxBytes + 1, in: try makeFolder())
        let provider = try #require(NSItemProvider(contentsOf: big))

        let dropped = try await drop(provider)

        guard case .refused(.tooLarge) = dropped.incoming.contents else {
            Issue.record("Expected it to be refused")
            return
        }
    }

    @Test func thePhotoPickerTakesAnyNumber() {
        #expect(PhotoPickerView.configuration.selectionLimit == 0)
    }

    @Test func thePhotoPickerLoadsAPicture() async throws {
        let source = try makeFile("IMG_0001.png", bytes: 8, in: try makeFolder())
        let provider = try #require(NSItemProvider(contentsOf: source))

        let incoming = await PhotoPickerView.Coordinator.load(provider, files: try makeFiles())
        defer { incoming.discard() }

        #expect(incoming.filename.hasSuffix(".png"))
        #expect(incoming.mimeType == "image/png")
        #expect(try incoming.read().count == 8)
    }

    @Test func somethingThePhotoPickerCantLoadIsReportedNotLost() async throws {
        let provider = NSItemProvider(item: "not a picture" as NSString, typeIdentifier: UTType.plainText.identifier)
        let incoming = await PhotoPickerView.Coordinator.load(provider)
        guard case .refused(.unreadable) = incoming.contents else {
            Issue.record("Expected it to be refused")
            return
        }
    }
}
