import SwiftUI
import UIKit
import Runestone
import TreeSitterBashRunestone
import TreeSitterCSSRunestone
import TreeSitterHTMLRunestone
import TreeSitterJavaScriptRunestone
import TreeSitterJSONRunestone
import TreeSitterMarkdownRunestone
import TreeSitterMarkdownInlineRunestone
import TreeSitterPythonRunestone
import TreeSitterSQLRunestone
import TreeSitterTypeScriptRunestone

// MARK: - Reaching the Text from Outside

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
/// which then comes up the responder chain to here.
final class EditorTextView: TextView {
    var onPasteAttachment: (() -> Void)?
    /// Replaceable for tests: suites run in parallel, and some use the general one.
    var pasteboard = UIPasteboard.general

    override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
        if action == #selector(paste(_:)) {
            return isEditing && onPasteAttachment != nil && PasteboardService.hasAttachableContent(in: pasteboard)
        }
        return super.canPerformAction(action, withSender: sender)
    }

    override func paste(_ sender: Any?) {
        onPasteAttachment?()
    }
}

// MARK: - Runestone Editor (UIViewRepresentable)

/// Wraps Runestone's UIKit `TextView` in SwiftUI with tree-sitter syntax highlighting.
struct RunestoneEditorView: UIViewRepresentable {
    @Binding var text: String
    var syntaxLanguage: String
    var wordWrap: Bool
    var isEditable: Bool
    var fontSize: CGFloat
    var cursor: EditorCursor?
    var onPasteAttachment: (() -> Void)?
    @Environment(\.colorScheme) private var colorScheme

    init(
        text: Binding<String>,
        syntaxLanguage: String = "markdown",
        wordWrap: Bool = true,
        isEditable: Bool = true,
        fontSize: CGFloat = EditorTheme.defaultFontSize,
        cursor: EditorCursor? = nil,
        onPasteAttachment: (() -> Void)? = nil
    ) {
        _text = text
        self.syntaxLanguage = syntaxLanguage
        self.wordWrap = wordWrap
        self.isEditable = isEditable
        self.fontSize = fontSize
        self.cursor = cursor
        self.onPasteAttachment = onPasteAttachment
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(text: $text)
    }

    func makeUIView(context: Context) -> EditorTextView {
        let textView = EditorTextView()
        textView.editorDelegate = context.coordinator
        cursor?.attach(textView)
        textView.showLineNumbers = true
        textView.isLineWrappingEnabled = wordWrap
        textView.isEditable = isEditable
        textView.autocorrectionType = .no
        textView.autocapitalizationType = .none
        textView.smartDashesType = .no
        textView.smartQuotesType = .no
        textView.smartInsertDeleteType = .no
        textView.inputAccessoryView = nil
        textView.showPageGuide = false
        textView.showTabs = false
        textView.showSpaces = false
        textView.showLineBreaks = false
        textView.characterPairs = Self.characterPairs
        textView.keyboardDismissMode = .interactive
        textView.contentInset = UIEdgeInsets(top: 4, left: 0, bottom: 4, right: 0)

        // iOS 16+ find/replace
        if #available(iOS 16.0, *) {
            textView.isFindInteractionEnabled = true
        }

        textView.backgroundColor = colorScheme == .dark ? .systemBackground : .systemBackground

        // Don't call applyState here — Runestone's setState doesn't
        // render reliably before the view is in the window hierarchy.
        // updateUIView will handle the initial state application.

        return textView
    }

    func updateUIView(_ textView: EditorTextView, context: Context) {
        let isDark = colorScheme == .dark
        let languageChanged = context.coordinator.currentLanguage != syntaxLanguage
        let themeChanged = context.coordinator.currentIsDark != isDark
        let fontSizeChanged = context.coordinator.currentFontSize != fontSize
        let textChanged = textView.text != text && !context.coordinator.isEditing

        if languageChanged || themeChanged || fontSizeChanged || textChanged {
            let theme = EditorTheme(isDark: isDark, fontSize: fontSize)
            context.coordinator.currentFontSize = fontSize
            textView.backgroundColor = isDark ? .systemBackground : .systemBackground

            // Only re-apply full state if language, theme, or font size changed, or external text change
            if languageChanged || themeChanged || fontSizeChanged {
                // Preserve cursor position across re-parse
                let selectedRange = textView.selectedRange
                let currentText = textChanged ? text : textView.text

                // Update coordinator tracking immediately to prevent re-entry
                context.coordinator.currentLanguage = syntaxLanguage
                context.coordinator.currentIsDark = isDark

                if textView.window != nil {
                    // View is in the hierarchy — apply state synchronously
                    applyState(to: textView, text: currentText, theme: theme)
                    if selectedRange.location + selectedRange.length <= currentText.utf16.count {
                        textView.selectedRange = selectedRange
                    }
                } else {
                    // View isn't in the hierarchy yet — defer to next run loop
                    // so Runestone can render once it has a window
                    DispatchQueue.main.async {
                        self.applyState(to: textView, text: currentText, theme: theme)
                        if selectedRange.location + selectedRange.length <= currentText.utf16.count {
                            textView.selectedRange = selectedRange
                        }
                    }
                }
            } else if textChanged {
                applyState(to: textView, theme: theme)
            }
        }

        textView.isEditable = isEditable
        textView.isLineWrappingEnabled = wordWrap
        textView.onPasteAttachment = onPasteAttachment
    }

    // MARK: - State Application

    private func applyState(to textView: TextView, text: String? = nil, theme: EditorTheme? = nil) {
        let resolvedText = text ?? self.text
        let resolvedTheme = theme ?? EditorTheme(isDark: colorScheme == .dark)

        let state: TextViewState
        if let language = Self.treeSitterLanguage(for: syntaxLanguage) {
            state = TextViewState(
                text: resolvedText,
                theme: resolvedTheme,
                language: language,
                languageProvider: Self.languageProvider
            )
        } else {
            state = TextViewState(text: resolvedText, theme: resolvedTheme)
        }
        textView.setState(state)
    }

    // MARK: - Language Provider

    /// Resolves injected language names (e.g. `markdown_inline` inside markdown,
    /// `css`/`javascript` inside HTML) to their tree-sitter grammars.
    private static let languageProvider = LanguageProvider()

    private class LanguageProvider: TreeSitterLanguageProvider {
        func treeSitterLanguage(named name: String) -> TreeSitterLanguage? {
            switch name {
            case "markdown_inline":  return .markdownInline
            case "markdown":         return .markdown
            case "javascript":       return .javaScript
            case "typescript":       return .typeScript
            case "python":           return .python
            case "json":             return .json
            case "html":             return .html
            case "css":              return .css
            case "bash", "sh":       return .bash
            case "sql":              return .sql
            default:                 return nil
            }
        }
    }

    // MARK: - Language Mapping

    static func treeSitterLanguage(for name: String) -> TreeSitterLanguage? {
        switch name {
        case "markdown":   return .markdown
        case "javascript": return .javaScript
        case "typescript": return .typeScript
        case "python":     return .python
        case "perl":       return nil    // Perl grammar crashes Runestone's RedBlackTree
        case "json":       return .json
        case "xml":        return .html  // closest available grammar
        case "css":        return .css
        case "bash":       return .bash
        case "sql":        return .sql
        case "plain":      return nil
        default:           return nil
        }
    }

    // MARK: - Character Pairs

    private struct Pair: CharacterPair {
        let leading: String
        let trailing: String
    }

    static let characterPairs: [CharacterPair] = [
        Pair(leading: "(", trailing: ")"),
        Pair(leading: "[", trailing: "]"),
        Pair(leading: "{", trailing: "}"),
        Pair(leading: "\"", trailing: "\""),
        Pair(leading: "'", trailing: "'"),
        Pair(leading: "`", trailing: "`"),
    ]

    // MARK: - Coordinator

    class Coordinator: NSObject, @preconcurrency TextViewDelegate {
        var text: Binding<String>
        var isEditing = false
        var currentLanguage: String = ""
        var currentIsDark: Bool = false
        var currentFontSize: CGFloat = 0
        private var endEditingWorkItem: DispatchWorkItem?

        init(text: Binding<String>) {
            self.text = text
        }

        @MainActor
        func textViewDidBeginEditing(_ textView: TextView) {
            endEditingWorkItem?.cancel()
            isEditing = true
        }

        @MainActor
        func textViewDidEndEditing(_ textView: TextView) {
            // Delay clearing isEditing to allow dictation's final text
            // replacement to land without triggering a re-entrant setState.
            endEditingWorkItem?.cancel()
            let item = DispatchWorkItem { [weak self] in
                self?.isEditing = false
            }
            endEditingWorkItem = item
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5, execute: item)
        }

        @MainActor
        func textViewDidChange(_ textView: TextView) {
            text.wrappedValue = textView.text
        }
    }
}
