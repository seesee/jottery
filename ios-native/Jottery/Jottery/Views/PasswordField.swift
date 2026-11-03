import SwiftUI

/// A password field with a button that shows what was typed or pasted, so it
/// can be checked before it's sent. Shown in a monospaced font, where
/// look-alikes such as `l`, `1` and `I` are easier to tell apart.
struct PasswordField: View {
    private let title: String
    @Binding private var text: String
    @State private var isRevealed = false

    init(_ title: String, text: Binding<String>) {
        self.title = title
        _text = text
    }

    var body: some View {
        HStack {
            Group {
                if isRevealed {
                    TextField(title, text: $text)
                        .fontDesign(.monospaced)
                } else {
                    SecureField(title, text: $text)
                }
            }
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
