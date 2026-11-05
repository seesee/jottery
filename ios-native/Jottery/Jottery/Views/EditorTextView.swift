import Runestone
import UIKit

/// The way in for what doesn't come from the keyboard, such as a pasted picture
/// or a dropped file, which goes in where the user was typing.
@MainActor
final class EditorCursor {
    private weak var textView: TextView?

    func attach(_ textView: TextView) {
        self.textView = textView
    }

    /// Put `text` at the cursor, or if there isn't one, at the end. False if no
    /// text view is showing, as in the preview.
    @discardableResult
    func insert(_ text: String, replacingSelection: Bool = true) -> Bool {
        guard let textView, textView.isEditable else { return false }
        let selection = textView.selectedRange
        let range = replacingSelection ? selection : NSRange(location: selection.upperBound, length: 0)
        textView.replaceAsOneUndoStep(range, with: text)
        return true
    }
}

extension TextView {
    /// Replace `range`, so that undo takes back this and nothing typed around it.
    /// Runestone otherwise groups everything from the last second into one step.
    func replaceAsOneUndoStep(_ range: NSRange, with text: String) {
        undoManager?.endUndoGrouping()
        replace(range, withText: text)
        undoManager?.endUndoGrouping()
    }
}

/// Runestone's text view, with what it leaves to its owner.
///
/// Its own text input pastes anything with text in it, and turns down the rest,
/// which then comes up the responder chain to here. So do the system's formatting
/// commands, Command-B and Command-I on a hardware keyboard among them.
final class EditorTextView: TextView {
    var onPasteAttachment: (() -> Void)?
    /// Replaceable for tests: suites run in parallel, and some use the general one.
    var pasteboard = UIPasteboard.general

    /// The bar above the keyboard on a phone.
    var formattingToolbar: UIToolbar?
    /// The same buttons in the bar an iPad already has there.
    var formattingGroup: UIBarButtonItemGroup?
    var showsFormatting: Bool?

    override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
        switch action {
        case #selector(paste(_:)):
            return isEditing && onPasteAttachment != nil && PasteboardService.hasAttachableContent(in: pasteboard)
        case #selector(toggleBoldface(_:)), #selector(toggleItalics(_:)):
            return isEditing && showsFormatting == true
        default:
            return super.canPerformAction(action, withSender: sender)
        }
    }

    override func paste(_ sender: Any?) {
        onPasteAttachment?()
    }

    override func toggleBoldface(_ sender: Any?) {
        apply(.bold)
    }

    override func toggleItalics(_ sender: Any?) {
        apply(.italic)
    }

    func apply(_ format: MarkdownFormat) {
        guard isEditable else { return }
        let edit = format.edit(of: text, selection: selectedRange)
        replaceAsOneUndoStep(edit.range, with: edit.replacement)
        selectedRange = edit.selection
    }

    func insert(_ command: QuickCommand) {
        guard isEditable else { return }
        replaceAsOneUndoStep(selectedRange, with: command.expansion())
    }
}
