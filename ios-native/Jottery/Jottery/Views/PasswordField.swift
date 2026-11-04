import SwiftUI
import UIKit

/// A password field with a button that shows what was typed or pasted, so it
/// can be checked before it's sent. Shown in a monospaced font, where
/// look-alikes such as `l`, `1` and `I` are easier to tell apart.
struct PasswordField: View {
    private let title: String
    @Binding private var text: String
    private let contentType: UITextContentType?
    @State private var isRevealed = false

    /// `contentType` (`.password`, `.newPassword`) is applied to whichever
    /// field is showing, so AutoFill treats both alike.
    init(_ title: String, text: Binding<String>, contentType: UITextContentType? = nil) {
        self.title = title
        _text = text
        self.contentType = contentType
    }

    var body: some View {
        HStack {
            // Both fields stay in place and only one is shown. Swapping one
            // out instead makes iOS read the filled secure field vanishing as
            // a submitted login, and offer "Save Password?".
            ZStack {
                SecureField(title, text: $text)
                    .opacity(isRevealed ? 0 : 1)
                    .allowsHitTesting(!isRevealed)
                    .accessibilityHidden(isRevealed)
                TextField(title, text: $text)
                    .fontDesign(.monospaced)
                    .opacity(isRevealed ? 1 : 0)
                    .allowsHitTesting(isRevealed)
                    .accessibilityHidden(!isRevealed)
            }
            .textContentType(contentType)
            .autocorrectionDisabled()
            .textInputAutocapitalization(.never)

            Button {
                isRevealed.toggle()
            } label: {
                Image(systemName: isRevealed ? "eye.slash" : "eye")
                    .foregroundStyle(.secondary)
            }
            // Borderless, so inside a Form only the icon is tappable, not the row.
            .buttonStyle(.borderless)
            .accessibilityLabel(isRevealed ? L.commonHidePassword : L.commonShowPassword)
        }
    }
}
