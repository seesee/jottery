import SwiftUI
import UIKit
import Runestone
import TreeSitterBashRunestone
import TreeSitterCSSRunestone
import TreeSitterHTMLRunestone
import TreeSitterJavaScriptRunestone
import TreeSitterJSONRunestone
import TreeSitterMarkdownRunestone
import TreeSitterPythonRunestone
import TreeSitterSQLRunestone
import TreeSitterTypeScriptRunestone

// MARK: - Runestone Editor (UIViewRepresentable)

/// Wraps Runestone's UIKit `TextView` in SwiftUI with tree-sitter syntax highlighting.
struct RunestoneEditorView: UIViewRepresentable {
    @Binding var text: String
    var syntaxLanguage: String
    var wordWrap: Bool
    var isEditable: Bool
    @Environment(\.colorScheme) private var colorScheme

    init(
        text: Binding<String>,
        syntaxLanguage: String = "markdown",
        wordWrap: Bool = true,
        isEditable: Bool = true
    ) {
        _text = text
        self.syntaxLanguage = syntaxLanguage
        self.wordWrap = wordWrap
        self.isEditable = isEditable
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(text: $text)
    }

    func makeUIView(context: Context) -> TextView {
        let textView = TextView()
        textView.editorDelegate = context.coordinator
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

        let isDark = colorScheme == .dark
        let theme = EditorTheme(isDark: isDark)
        textView.backgroundColor = isDark ? .systemBackground : .systemBackground

        applyState(to: textView, theme: theme)

        context.coordinator.currentLanguage = syntaxLanguage
        context.coordinator.currentIsDark = isDark

        return textView
    }

    func updateUIView(_ textView: TextView, context: Context) {
        let isDark = colorScheme == .dark
        let languageChanged = context.coordinator.currentLanguage != syntaxLanguage
        let themeChanged = context.coordinator.currentIsDark != isDark
        let textChanged = textView.text != text && !context.coordinator.isEditing

        if languageChanged || themeChanged || textChanged {
            let theme = EditorTheme(isDark: isDark)
            textView.backgroundColor = isDark ? .systemBackground : .systemBackground

            // Only re-apply full state if language or theme changed, or external text change
            if languageChanged || themeChanged {
                // Preserve cursor position across re-parse
                let selectedRange = textView.selectedRange
                let currentText = textChanged ? text : textView.text
                applyState(to: textView, text: currentText, theme: theme)
                // Restore cursor if within bounds
                if selectedRange.location + selectedRange.length <= currentText.utf16.count {
                    textView.selectedRange = selectedRange
                }
                context.coordinator.currentLanguage = syntaxLanguage
                context.coordinator.currentIsDark = isDark
            } else if textChanged {
                applyState(to: textView, theme: theme)
            }
        }

        textView.isEditable = isEditable
        textView.isLineWrappingEnabled = wordWrap
    }

    // MARK: - State Application

    private func applyState(to textView: TextView, text: String? = nil, theme: EditorTheme? = nil) {
        let resolvedText = text ?? self.text
        let resolvedTheme = theme ?? EditorTheme(isDark: colorScheme == .dark)

        let state: TextViewState
        if let language = Self.treeSitterLanguage(for: syntaxLanguage) {
            state = TextViewState(text: resolvedText, theme: resolvedTheme, language: language)
        } else {
            state = TextViewState(text: resolvedText, theme: resolvedTheme)
        }
        textView.setState(state)
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
        case "xml":        return .html   // closest available grammar
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

        init(text: Binding<String>) {
            self.text = text
        }

        @MainActor
        func textViewDidBeginEditing(_ textView: TextView) {
            isEditing = true
        }

        @MainActor
        func textViewDidEndEditing(_ textView: TextView) {
            isEditing = false
        }

        @MainActor
        func textViewDidChange(_ textView: TextView) {
            text.wrappedValue = textView.text
        }
    }
}
