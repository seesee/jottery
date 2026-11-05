import Foundation
import Testing
import UIKit

@testable import Jottery

private let attachmentId = "0b7e2a64-5f3e-4c8e-9a3e-1f2d3c4b5a69"

/// What a note's text says about an attachment has to read the same on every
/// client, and still resolve in the preview (R2-144).
@MainActor
struct AttachmentMarkdownTests {

    private func ref(_ filename: String, mimeType: String = "image/png") -> AttachmentRef {
        AttachmentRef(id: attachmentId, filename: filename, mimeType: mimeType, size: 4, data: attachmentId)
    }

    @Test func aPictureIsWrittenAsTheWebPastesIt() {
        // EditorPane.svelte: `![${altText}](attachment:${attachment.id})`, the
        // name without its extension.
        #expect(AttachmentMarkdown.reference(to: ref("holiday.png")) == "![holiday](attachment:\(attachmentId))")
    }

    @Test func anythingElseIsWrittenAsTheWebWritesALink() {
        #expect(
            AttachmentMarkdown.reference(to: ref("report.pdf", mimeType: "application/pdf"))
                == "[Attachment: report.pdf](attachment:\(attachmentId))"
        )
    }

    @Test func aNameWithSpacesCantBreakTheLink() {
        #expect(AttachmentMarkdown.reference(to: ref("my holiday.png")) == "![my holiday](attachment:\(attachmentId))")
    }

    @Test func aNameCantCloseTheLabelEarly() {
        #expect(
            AttachmentMarkdown.reference(to: ref("a]b[c`d\\e.png"))
                == "![a\\]b\\[c\\`d\\\\e](attachment:\(attachmentId))"
        )
    }

    @Test func severalGoOnALineEach() {
        let markdown = AttachmentMarkdown.references(to: [ref("a.png"), ref("b.png")])
        #expect(markdown == "![a](attachment:\(attachmentId))\n![b](attachment:\(attachmentId))")
    }

    /// The URL has to be one the preview bundle accepts and `find` resolves: a
    /// filename here shows as bare alt text on iOS.
    @Test func theReferenceResolvesInThePreview() throws {
        let attachment = ref("holiday.png")
        let markdown = AttachmentMarkdown.reference(to: attachment)
        let target = try #require(markdown.split(separator: "(").last?.dropLast())
        let reference = try #require(target.split(separator: ":").last.map(String.init))

        // ATTACHMENT_ID in webview-bundles/src/preview/index.ts.
        #expect(reference.wholeMatch(of: /[0-9a-zA-Z-]{1,64}/) != nil)
        #expect(PreviewAttachments.find(reference, in: [attachment]) == attachment)
    }

    @Test func withNoCursorItGoesOnANewLineAtTheEnd() {
        #expect(AttachmentMarkdown.appending("![a](x)", to: "") == "![a](x)")
        #expect(AttachmentMarkdown.appending("![a](x)", to: "hello") == "hello\n![a](x)")
        #expect(AttachmentMarkdown.appending("![a](x)", to: "hello\n") == "hello\n![a](x)")
    }

    // MARK: - At the Cursor

    private func makeEditor(_ text: String) -> (EditorCursor, HeadlessEditor) {
        let editor = HeadlessEditor(text: text)
        let cursor = EditorCursor()
        cursor.attach(editor.textView)
        return (cursor, editor)
    }

    @Test func goesInWhereTheCursorIs() {
        let (cursor, editor) = makeEditor("before after")
        editor.textView.selectedRange = NSRange(location: 7, length: 0)

        #expect(cursor.insert("![a](attachment:x)"))

        #expect(editor.textView.text == "before ![a](attachment:x)after")
        #expect(editor.bound == editor.textView.text)
        #expect(editor.textView.selectedRange == NSRange(location: 25, length: 0))
    }

    @Test func aPasteReplacesTheSelection() {
        let (cursor, editor) = makeEditor("one two three")
        editor.textView.selectedRange = NSRange(location: 4, length: 3)

        cursor.insert("X")

        #expect(editor.textView.text == "one X three")
    }

    @Test func aDropLeavesTheSelectionAlone() {
        let (cursor, editor) = makeEditor("one two three")
        editor.textView.selectedRange = NSRange(location: 4, length: 3)

        cursor.insert("X", replacingSelection: false)

        #expect(editor.textView.text == "one twoX three")
    }

    @Test func undoTakesItBack() throws {
        let (cursor, editor) = makeEditor("hello")

        cursor.insert("![a](attachment:x)")
        #expect(editor.textView.text == "hello![a](attachment:x)")
        let undoManager = try #require(editor.textView.undoManager)
        #expect(undoManager.canUndo)
        undoManager.undo()

        #expect(editor.textView.text == "hello")
        #expect(editor.bound == "hello")
    }

    @Test func undoTakesBackOnlyThatAndNotWhatWasTypedBeforeIt() throws {
        let (cursor, editor) = makeEditor("")
        editor.textView.insertText("typed")

        cursor.insert("!")
        try #require(editor.textView.undoManager).undo()

        #expect(editor.textView.text == "typed")
    }

    @Test func aReadOnlyEditorTakesNothing() {
        let (cursor, editor) = makeEditor("locked")
        editor.textView.isEditable = false

        #expect(!cursor.insert("X"))
        #expect(editor.textView.text == "locked")
    }

    @Test func withNoEditorShowingItSaysSo() {
        #expect(!EditorCursor().insert("X"))
    }

    // MARK: - Pasting

    private func pasteboardWithImage() -> UIPasteboard {
        let pasteboard = UIPasteboard(name: UIPasteboard.Name(UUID().uuidString), create: true)!
        pasteboard.image = UIGraphicsImageRenderer(size: CGSize(width: 4, height: 4)).image { context in
            UIColor.red.setFill()
            context.fill(CGRect(x: 0, y: 0, width: 4, height: 4))
        }
        return pasteboard
    }

    @Test func pasteIsNotOfferedWhenTheEditorIsntInUse() {
        let textView = EditorTextView()
        textView.pasteboard = pasteboardWithImage()
        textView.onPasteAttachment = {}
        #expect(!textView.canPerformAction(#selector(UIResponderStandardEditActions.paste(_:)), withSender: nil))
    }

    @Test func pastingReachesTheOwner() {
        var pasted = 0
        let textView = EditorTextView()
        textView.onPasteAttachment = { pasted += 1 }
        textView.paste(nil)
        #expect(pasted == 1)
    }
}
