import Foundation
import Runestone
import SwiftUI
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

    private func makeEditor(_ text: String) -> (EditorCursor, EditorTextView, Binding<String>, () -> String) {
        final class Box { var text: String; init(_ text: String) { self.text = text } }
        let box = Box(text)
        let binding = Binding(get: { box.text }, set: { box.text = $0 })
        let view = RunestoneEditorView(text: binding)
        let coordinator = view.makeCoordinator()
        let textView = EditorTextView()
        textView.editorDelegate = coordinator
        textView.text = text
        let cursor = EditorCursor()
        cursor.attach(textView)
        // The delegate is held weakly, so something has to keep it.
        objc_setAssociatedObject(textView, &coordinatorKey, coordinator, .OBJC_ASSOCIATION_RETAIN)
        return (cursor, textView, binding, { box.text })
    }

    @Test func goesInWhereTheCursorIs() {
        let (cursor, textView, _, text) = makeEditor("before after")
        textView.selectedRange = NSRange(location: 7, length: 0)

        #expect(cursor.insert("![a](attachment:x)"))

        #expect(textView.text == "before ![a](attachment:x)after")
        #expect(text() == textView.text)
        #expect(textView.selectedRange == NSRange(location: 25, length: 0))
    }

    @Test func aPasteReplacesTheSelection() {
        let (cursor, textView, _, _) = makeEditor("one two three")
        textView.selectedRange = NSRange(location: 4, length: 3)

        cursor.insert("X")

        #expect(textView.text == "one X three")
    }

    @Test func aDropLeavesTheSelectionAlone() {
        let (cursor, textView, _, _) = makeEditor("one two three")
        textView.selectedRange = NSRange(location: 4, length: 3)

        cursor.insert("X", replacingSelection: false)

        #expect(textView.text == "one twoX three")
    }

    @Test func undoTakesItBack() throws {
        let (cursor, textView, _, text) = makeEditor("hello")
        textView.selectedRange = NSRange(location: 5, length: 0)

        cursor.insert("![a](attachment:x)")
        #expect(textView.text == "hello![a](attachment:x)")
        let undoManager = try #require(textView.undoManager)
        #expect(undoManager.canUndo)
        undoManager.undo()

        #expect(textView.text == "hello")
        #expect(text() == "hello")
    }

    @Test func undoTakesBackOnlyThatAndNotWhatWasTypedBeforeIt() throws {
        let (cursor, textView, _, _) = makeEditor("")
        textView.selectedRange = NSRange(location: 0, length: 0)
        textView.insertText("typed")

        cursor.insert("!")
        try #require(textView.undoManager).undo()

        #expect(textView.text == "typed")
    }

    @Test func aReadOnlyEditorTakesNothing() {
        let (cursor, textView, _, _) = makeEditor("locked")
        textView.isEditable = false

        #expect(!cursor.insert("X"))
        #expect(textView.text == "locked")
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

nonisolated(unsafe) private var coordinatorKey: UInt8 = 0
