import UIKit
import UniformTypeIdentifiers

/// Reads attachable content (images, PDFs) from the general pasteboard.
///
/// Runestone's editor only pastes strings, so images copied from other
/// apps are attached via the editor toolbar instead.
@MainActor
enum PasteboardService {

    struct PastedItem {
        let data: Data
        let filename: String
        let mimeType: String
    }

    /// Whether the pasteboard holds content that can be attached to a note.
    /// Only inspects types — does not read data, so no paste banner is shown.
    static var hasAttachableContent: Bool {
        let pasteboard = UIPasteboard.general
        return pasteboard.hasImages
            || pasteboard.contains(pasteboardTypes: [UTType.pdf.identifier])
    }

    /// Extract attachable items. Reading contents may show the iOS paste banner.
    static func readItems(now: Date = Date()) -> [PastedItem] {
        let pasteboard = UIPasteboard.general
        var items: [PastedItem] = []

        let formatter = DateFormatter()
        formatter.dateFormat = "yyyyMMdd-HHmmss"
        formatter.locale = Locale(identifier: "en_GB_POSIX")
        let stamp = formatter.string(from: now)

        for index in 0..<pasteboard.numberOfItems {
            let suffix = pasteboard.numberOfItems > 1 ? "-\(index + 1)" : ""
            let itemSet = IndexSet(integer: index)

            if let data = pasteboard.data(forPasteboardType: UTType.png.identifier, inItemSet: itemSet)?.first {
                items.append(PastedItem(data: data, filename: "pasted-\(stamp)\(suffix).png", mimeType: "image/png"))
            } else if let data = pasteboard.data(forPasteboardType: UTType.jpeg.identifier, inItemSet: itemSet)?.first {
                items.append(PastedItem(data: data, filename: "pasted-\(stamp)\(suffix).jpg", mimeType: "image/jpeg"))
            } else if let images = pasteboard.images, index < images.count, let data = images[index].pngData() {
                // Image in a non-PNG/JPEG representation — re-encode as PNG
                items.append(PastedItem(data: data, filename: "pasted-\(stamp)\(suffix).png", mimeType: "image/png"))
            } else if let data = pasteboard.data(forPasteboardType: UTType.pdf.identifier, inItemSet: itemSet)?.first {
                items.append(PastedItem(data: data, filename: "pasted-\(stamp)\(suffix).pdf", mimeType: "application/pdf"))
            }
        }
        return items
    }
}
