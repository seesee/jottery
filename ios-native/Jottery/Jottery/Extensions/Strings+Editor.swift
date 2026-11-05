import Foundation

extension L {

    // MARK: - Adding Attachments

    static func attachmentsTooLarge(_ filename: String) -> String {
        String(format: String(localized: "attachments.tooLarge"), filename)
    }
    static func attachmentsTooLargeCount(_ count: Int) -> String {
        String(localized: "attachments.tooLargeCount \(count)")
    }
    static func attachmentsEmptyFile(_ filename: String) -> String {
        String(format: String(localized: "attachments.emptyFile"), filename)
    }
    static var attachmentsCopyMarkdown: String { String(localized: "attachments.copyMarkdown") }
    static var attachmentsMarkdownCopied: String { String(localized: "attachments.markdownCopied") }
}
