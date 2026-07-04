import Foundation
import Testing
import UIKit

@testable import Jottery

@MainActor
struct PasteboardServiceTests {

    private func makeTestImage() -> UIImage {
        let renderer = UIGraphicsImageRenderer(size: CGSize(width: 4, height: 4))
        return renderer.image { ctx in
            UIColor.red.setFill()
            ctx.fill(CGRect(x: 0, y: 0, width: 4, height: 4))
        }
    }

    @Test func readsImageFromPasteboard() throws {
        UIPasteboard.general.items = []
        UIPasteboard.general.image = makeTestImage()

        #expect(PasteboardService.hasAttachableContent)
        let items = PasteboardService.readItems(now: Date(timeIntervalSince1970: 1_751_600_000))
        #expect(items.count == 1)
        let item = try #require(items.first)
        #expect(item.mimeType == "image/png")
        #expect(item.filename.hasPrefix("pasted-"))
        #expect(item.filename.hasSuffix(".png"))
        #expect(!item.data.isEmpty)
    }

    @Test func emptyPasteboardHasNoAttachableContent() {
        UIPasteboard.general.items = []
        #expect(!PasteboardService.hasAttachableContent)
        #expect(PasteboardService.readItems().isEmpty)
    }

    @Test func plainTextIsNotAttachable() {
        UIPasteboard.general.items = []
        UIPasteboard.general.string = "just text"
        // Text pastes natively into the editor — the attachment path must not claim it.
        #expect(!PasteboardService.hasAttachableContent)
    }
}
