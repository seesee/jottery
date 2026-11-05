import Foundation
import Testing
import UIKit

@testable import Jottery

@MainActor
struct PasteboardServiceTests {

    /// Its own, not `.general`: suites run in parallel, and `SyncSetupReopenTests`
    /// pastes through the general pasteboard for real.
    private let pasteboard = UIPasteboard(name: UIPasteboard.Name(UUID().uuidString), create: true)!

    private func makeTestImage() -> UIImage {
        let renderer = UIGraphicsImageRenderer(size: CGSize(width: 4, height: 4))
        return renderer.image { ctx in
            UIColor.red.setFill()
            ctx.fill(CGRect(x: 0, y: 0, width: 4, height: 4))
        }
    }

    @Test func readsImageFromPasteboard() throws {
        pasteboard.image = makeTestImage()

        #expect(PasteboardService.hasAttachableContent(in: pasteboard))
        let items = PasteboardService.readItems(from: pasteboard, now: Date(timeIntervalSince1970: 1_751_600_000))
        #expect(items.count == 1)
        let item = try #require(items.first)
        #expect(item.mimeType == "image/png")
        #expect(item.filename.hasPrefix("pasted-"))
        #expect(item.filename.hasSuffix(".png"))
        #expect(!item.data.isEmpty)
    }

    @Test func emptyPasteboardHasNoAttachableContent() {
        #expect(!PasteboardService.hasAttachableContent(in: pasteboard))
        #expect(PasteboardService.readItems(from: pasteboard).isEmpty)
    }

    @Test func plainTextIsNotAttachable() {
        pasteboard.string = "just text"
        // Text pastes natively into the editor — the attachment path must not claim it.
        #expect(!PasteboardService.hasAttachableContent(in: pasteboard))
    }
}
