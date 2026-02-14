import SwiftUI
import UIKit

// MARK: - Runestone Editor (UIViewRepresentable)
// This wraps Runestone's UIKit-based TextView in SwiftUI.
// Requires the Runestone SPM package to be added to the project.
//
// For now, this provides a UITextView wrapper that will be upgraded
// to Runestone once the package dependency is configured in Xcode.

struct RunestoneEditorView: UIViewRepresentable {
    @Binding var text: String
    var syntaxLanguage: String
    var wordWrap: Bool
    var isEditable: Bool

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

    func makeUIView(context: Context) -> UITextView {
        let textView = UITextView()
        textView.delegate = context.coordinator
        textView.font = .monospacedSystemFont(ofSize: 15, weight: .regular)
        textView.autocorrectionType = .no
        textView.autocapitalizationType = .none
        textView.smartDashesType = .no
        textView.smartQuotesType = .no
        textView.smartInsertDeleteType = .no
        textView.textContainerInset = UIEdgeInsets(top: 12, left: 8, bottom: 12, right: 8)
        textView.isEditable = isEditable
        textView.isScrollEnabled = true
        textView.alwaysBounceVertical = true
        textView.keyboardDismissMode = .interactive

        // Line wrapping
        if wordWrap {
            textView.textContainer.lineBreakMode = .byWordWrapping
        } else {
            textView.textContainer.lineBreakMode = .byClipping
            textView.textContainer.widthTracksTextView = false
            textView.textContainer.size = CGSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        }

        textView.text = text
        return textView
    }

    func updateUIView(_ textView: UITextView, context: Context) {
        // Only update text if it changed externally (not from user typing)
        if textView.text != text && !context.coordinator.isEditing {
            textView.text = text
        }

        textView.isEditable = isEditable

        // Update theme based on current trait collection
        let isDark = textView.traitCollection.userInterfaceStyle == .dark
        textView.backgroundColor = isDark ? UIColor.systemBackground : UIColor.systemBackground
        textView.textColor = isDark ? .white : .black
    }

    // MARK: - Coordinator

    class Coordinator: NSObject, UITextViewDelegate {
        var text: Binding<String>
        var isEditing = false

        init(text: Binding<String>) {
            self.text = text
        }

        func textViewDidBeginEditing(_ textView: UITextView) {
            isEditing = true
        }

        func textViewDidEndEditing(_ textView: UITextView) {
            isEditing = false
        }

        func textViewDidChange(_ textView: UITextView) {
            text.wrappedValue = textView.text
        }
    }
}
