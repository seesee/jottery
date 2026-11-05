import Foundation
import Runestone
import Testing
import UIKit

@testable import Jottery

/// What only happens with a real first responder in a real window (R2-144, R2-145).
@MainActor
@Suite(.serialized)
struct EditorResponderTests {

    private let paste = #selector(UIResponderStandardEditActions.paste(_:))
    private let bold = #selector(UIResponderStandardEditActions.toggleBoldface(_:))
    private let italic = #selector(UIResponderStandardEditActions.toggleItalics(_:))

    private func makeWindow(_ textView: EditorTextView) throws -> UIWindow {
        let scene = try #require(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
        let window = UIWindow(windowScene: scene)
        window.frame = CGRect(x: 0, y: 0, width: 390, height: 600)
        textView.frame = window.bounds
        window.addSubview(textView)
        window.isHidden = false
        return window
    }

    /// The view that actually holds the keyboard, which is inside Runestone.
    private func firstResponder(in textView: UIView) -> UIResponder? {
        textView.subviews.first { $0.isFirstResponder }
    }

    private func pasteboard(image: Bool) -> UIPasteboard {
        let pasteboard = UIPasteboard(name: UIPasteboard.Name(UUID().uuidString), create: true)!
        if image {
            pasteboard.image = UIGraphicsImageRenderer(size: CGSize(width: 4, height: 4)).image { context in
                UIColor.red.setFill()
                context.fill(CGRect(x: 0, y: 0, width: 4, height: 4))
            }
        }
        return pasteboard
    }

    /// Runestone's own input answers first, going by the general pasteboard, which
    /// other suites use. So this checks the two halves that don't depend on it:
    /// what it turns down comes here next, and here says yes.
    @Test func pastingAPictureComesUpTheChainToTheEditor() throws {
        let textView = EditorTextView()
        let window = try makeWindow(textView)
        defer { window.isHidden = true }
        textView.pasteboard = pasteboard(image: true)
        textView.onPasteAttachment = {}

        #expect(textView.becomeFirstResponder())
        let responder = try #require(firstResponder(in: textView))

        #expect(responder.next === textView)
        #expect(textView.canPerformAction(paste, withSender: nil))
    }

    @Test func withNothingToAttachPasteIsNotOffered() throws {
        let textView = EditorTextView()
        let window = try makeWindow(textView)
        defer { window.isHidden = true }
        textView.pasteboard = pasteboard(image: false)
        textView.onPasteAttachment = {}

        #expect(textView.becomeFirstResponder())

        #expect(!textView.canPerformAction(paste, withSender: nil))
    }

    @Test func commandBAndCommandIReachTheEditor() throws {
        let textView = EditorTextView()
        let window = try makeWindow(textView)
        defer { window.isHidden = true }
        EditorFormattingBar.install(on: textView, showsFormatting: true, idiom: .phone)
        textView.text = "word"

        #expect(textView.becomeFirstResponder())
        let responder = try #require(firstResponder(in: textView))
        textView.selectedRange = NSRange(location: 0, length: 4)

        #expect(responder.target(forAction: bold, withSender: nil) as? EditorTextView === textView)
        #expect(responder.target(forAction: italic, withSender: nil) as? EditorTextView === textView)
        textView.toggleBoldface(nil)
        #expect(textView.text == "**word**")
    }

    @Test func outsideMarkdownCommandBDoesNothing() throws {
        let textView = EditorTextView()
        let window = try makeWindow(textView)
        defer { window.isHidden = true }
        EditorFormattingBar.install(on: textView, showsFormatting: false, idiom: .phone)

        #expect(textView.becomeFirstResponder())
        let responder = try #require(firstResponder(in: textView))

        #expect(responder.target(forAction: bold, withSender: nil) as? EditorTextView == nil)
    }

    @Test func theToolbarShowsWhileEditing() throws {
        let textView = EditorTextView()
        let window = try makeWindow(textView)
        defer { window.isHidden = true }
        EditorFormattingBar.install(on: textView, showsFormatting: true, idiom: .phone)

        #expect(textView.becomeFirstResponder())

        #expect(textView.inputAccessoryView === textView.formattingToolbar)
    }

    @Test func aReadOnlyNoteHasNoToolbarOrKeyboard() throws {
        let textView = EditorTextView()
        let window = try makeWindow(textView)
        defer { window.isHidden = true }
        EditorFormattingBar.install(on: textView, showsFormatting: true, idiom: .phone)
        textView.isEditable = false

        #expect(!textView.becomeFirstResponder())
        #expect(textView.inputAccessoryView == nil)
    }
}
