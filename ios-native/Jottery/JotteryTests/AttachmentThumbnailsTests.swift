import CoreGraphics
import Foundation
import Testing
import UIKit

@testable import Jottery

/// Thumbnails show what's in an encrypted attachment, so they live in memory only
/// and go with the lock (R2-144).
@MainActor
struct AttachmentThumbnailsTests {

    private func ref(id: String = UUID().uuidString, mimeType: String = "image/png", size: Int = 4) -> AttachmentRef {
        AttachmentRef(id: id, filename: "a", mimeType: mimeType, size: size, data: id)
    }

    private func png(width: CGFloat = 800, height: CGFloat = 400) throws -> Data {
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        let image = UIGraphicsImageRenderer(size: CGSize(width: width, height: height), format: format).image { context in
            UIColor.blue.setFill()
            context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        }
        return try #require(image.pngData())
    }

    private func pdf() -> Data {
        let bounds = CGRect(x: 0, y: 0, width: 612, height: 792)
        return UIGraphicsPDFRenderer(bounds: bounds).pdfData { context in
            context.beginPage()
            UIColor.red.setFill()
            context.fill(bounds)
        }
    }

    // MARK: - Drawing

    @Test func aPictureIsDrawnSmall() throws {
        let image = try #require(AttachmentThumbnails.draw(try png(), mimeType: "image/png"))
        #expect(max(image.size.width, image.size.height) <= CGFloat(AttachmentThumbnails.maxPixels))
        #expect(image.size.width > image.size.height)
    }

    @Test func aPDFShowsItsFirstPage() throws {
        let image = try #require(AttachmentThumbnails.draw(pdf(), mimeType: "application/pdf"))
        #expect(max(image.size.width, image.size.height) <= CGFloat(AttachmentThumbnails.maxPixels))
        #expect(image.size.height > image.size.width)

        // Red, from the page, and not the white it's drawn on.
        let cgImage = try #require(image.cgImage)
        var pixel: [UInt8] = [0, 0, 0, 0]
        let context = try #require(CGContext(
            data: &pixel, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ))
        context.draw(cgImage, in: CGRect(x: 0, y: 0, width: 1, height: 1))
        #expect(pixel[0] > 200 && pixel[1] < 80)
    }

    @Test func somethingThatIsntWhatItSaysDrawsNothing() {
        #expect(AttachmentThumbnails.draw(Data("not a picture".utf8), mimeType: "image/png") == nil)
        #expect(AttachmentThumbnails.draw(Data("not a pdf".utf8), mimeType: "application/pdf") == nil)
    }

    @Test func otherTypesKeepTheirIcon() async throws {
        let thumbnails = AttachmentThumbnails()
        let data = try png()
        #expect(await thumbnails.image(for: ref(mimeType: "video/mp4")) { data } == nil)
    }

    @Test func somethingHugeIsNotDecryptedJustToListIt() async throws {
        let thumbnails = AttachmentThumbnails()
        let data = try png()
        let attachment = ref(size: AttachmentThumbnails.maxSourceBytes + 1)
        #expect(await thumbnails.image(for: attachment) { data } == nil)
    }

    // MARK: - Keeping Them

    @Test func eachIsDecryptedOnce() async throws {
        let thumbnails = AttachmentThumbnails()
        let attachment = ref()
        let data = try png()

        #expect(await thumbnails.image(for: attachment) { data } != nil)
        // A second fetch would come back empty, so a picture here has to be the kept one.
        #expect(await thumbnails.image(for: attachment) { nil } != nil)
    }

    @Test func aFailedDecryptIsNotAPicture() async {
        let thumbnails = AttachmentThumbnails()
        struct Failure: Error {}
        #expect(await thumbnails.image(for: ref()) { throw Failure() } == nil)
    }

    @Test func removeAllForgetsEverything() async throws {
        let thumbnails = AttachmentThumbnails()
        let attachment = ref()
        let data = try png()
        _ = await thumbnails.image(for: attachment) { data }
        #expect(thumbnails.cached(attachment.id) != nil)

        thumbnails.removeAll()

        #expect(thumbnails.cached(attachment.id) == nil)
    }

    @Test func aPictureFinishedAfterALockIsNotKept() async throws {
        let thumbnails = AttachmentThumbnails()
        let attachment = ref()
        let data = try png()
        let gate = AsyncGate()

        let drawing = Task {
            await thumbnails.image(for: attachment) {
                await gate.wait()
                return data
            }
        }
        await waitUntil { await gate.waitingCount == 1 }

        thumbnails.removeAll()
        await gate.open()

        #expect(await drawing.value == nil)
        #expect(thumbnails.cached(attachment.id) == nil)
    }

    @Test func theyAreDrawnOneAtATime() async throws {
        let thumbnails = AttachmentThumbnails()
        let data = try png()
        let gate = AsyncGate()
        let events = Events()

        let first = Task {
            await thumbnails.image(for: ref()) {
                await gate.wait()
                await events.add("first finished")
                return data
            }
        }
        await waitUntil { await gate.waitingCount == 1 }
        let second = Task {
            await thumbnails.image(for: ref()) {
                await events.add("second started")
                return data
            }
        }
        // Plenty of chances to jump the queue, if it could.
        for _ in 0..<200 { await Task.yield() }
        await gate.open()

        _ = await first.value
        _ = await second.value
        #expect(await events.list == ["first finished", "second started"])
    }

    @Test func aRowThatGoesAwayStopsWaiting() async throws {
        let thumbnails = AttachmentThumbnails()
        let data = try png()
        let gate = AsyncGate()

        let first = Task {
            await thumbnails.image(for: ref()) {
                await gate.wait()
                return data
            }
        }
        await waitUntil { await gate.waitingCount == 1 }
        let fetched = Flag()
        let abandoned = Task {
            await thumbnails.image(for: ref()) {
                await fetched.set()
                return data
            }
        }
        await Task.yield()
        abandoned.cancel()
        await gate.open()

        _ = await first.value
        #expect(await abandoned.value == nil)
        #expect(await !fetched.value)
    }

    // MARK: - The Lock

    @Test func lockingTheVaultClearsThem() async throws {
        let state = try TestVaults.makeState()
        try await state.createVault(password: "notes-password-correct-horse")
        // Its own: other suites' vaults lock while this runs, and clear the shared one.
        let thumbnails = AttachmentThumbnails()
        state.attachmentThumbnails = thumbnails
        let attachment = ref()
        let data = try png()
        _ = await thumbnails.image(for: attachment) { data }
        #expect(thumbnails.cached(attachment.id) != nil)

        state.lock()

        #expect(thumbnails.cached(attachment.id) == nil)
    }

    @Test func removingAnAttachmentTakesItsPictureToo() async throws {
        let state = try TestVaults.makeState()
        try await state.createVault(password: "notes-password-correct-horse")
        let thumbnails = AttachmentThumbnails()
        state.attachmentThumbnails = thumbnails
        let note = try #require(try state.createNote(content: "hello"))
        let data = try png()
        let added = try #require(try state.addAttachment(to: note.id, data: data, filename: "a.png", mimeType: "image/png"))
        _ = await thumbnails.image(for: added) { data }
        #expect(thumbnails.cached(added.id) != nil)

        try state.removeAttachment(from: note.id, attachmentId: added.id)

        #expect(thumbnails.cached(added.id) == nil)
    }
}

private actor Flag {
    private(set) var value = false
    func set() { value = true }
}

private actor Events {
    private(set) var list: [String] = []
    func add(_ event: String) { list.append(event) }
}
