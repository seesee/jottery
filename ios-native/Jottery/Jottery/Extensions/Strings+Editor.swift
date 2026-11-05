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

    // MARK: - Formatting Bar

    static var editorFormatBold: String { String(localized: "editor.formatBold") }
    static var editorFormatItalic: String { String(localized: "editor.formatItalic") }
    static var editorFormatList: String { String(localized: "editor.formatList") }
    static var editorFormatLink: String { String(localized: "editor.formatLink") }
    static var editorInsert: String { String(localized: "editor.insert") }
    static var editorInsertDateTime: String { String(localized: "editor.insertDateTime") }
    static var editorInsertDate: String { String(localized: "editor.insertDate") }
    static var editorInsertTime: String { String(localized: "editor.insertTime") }
    static var editorInsertUUID: String { String(localized: "editor.insertUUID") }
    static var editorInsertRule: String { String(localized: "editor.insertRule") }
}
