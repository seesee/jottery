import SwiftUI

// MARK: - Outliner Editor (WebView)

/// Vanilla JS tree-based outliner editor, loaded from the outliner.html bundle.
/// Supports indent/outdent, expand/collapse, drag-and-drop reordering,
/// and serialises to the markdown-compatible format (- expanded / + collapsed).
struct WebOutlinerEditorView: View {
    @Binding var content: String
    var fontSize: CGFloat = EditorTheme.defaultFontSize

    var body: some View {
        WebEditorView(bundleName: "outliner", content: $content, fontSize: fontSize)
    }
}
