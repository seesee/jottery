import Foundation

/// How a note's text points at one of its attachments.
///
/// The shape is the web's: `![name](attachment:id)` for a picture, and
/// `[Attachment: name](attachment:id)` for anything else. The web's Copy Markdown
/// puts the filename in the URL instead, but only the web can follow that: the
/// preview here resolves ids, and a space in a name ends the URL. An id reads the
/// same on every client.
enum AttachmentMarkdown {

    static func reference(to attachment: AttachmentRef) -> String {
        let url = "attachment:\(attachment.id)"
        if attachment.mimeType.hasPrefix("image/") {
            return "![\(escaped((attachment.filename as NSString).deletingPathExtension))](\(url))"
        }
        return "[Attachment: \(escaped(attachment.filename))](\(url))"
    }

    /// One to a line, for several added at once.
    static func references(to attachments: [AttachmentRef]) -> String {
        attachments.map(reference(to:)).joined(separator: "\n")
    }

    /// `markdown` on a line of its own after `content`, for when there's no cursor.
    static func appending(_ markdown: String, to content: String) -> String {
        content.isEmpty || content.hasSuffix("\n") ? content + markdown : content + "\n" + markdown
    }

    /// A bracket or a backtick in a filename would end the label early.
    private static func escaped(_ label: String) -> String {
        label.reduce(into: "") { result, character in
            if "\\[]`".contains(character) { result.append("\\") }
            result.append(character)
        }
    }
}
