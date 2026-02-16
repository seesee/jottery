import SwiftUI

// MARK: - Calculator Editor (WebView)

/// CodeMirror 6 + math.js REPL calculator editor, loaded from the calc.html bundle.
/// Results are displayed inline next to each expression.
struct WebCalcEditorView: View {
    @Binding var content: String
    var fontSize: CGFloat = EditorTheme.defaultFontSize

    var body: some View {
        WebEditorView(bundleName: "calc", content: $content, fontSize: fontSize)
    }
}
