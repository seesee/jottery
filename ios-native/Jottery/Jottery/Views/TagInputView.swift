import SwiftUI

/// Tag editing chip input — allows adding/removing tags.
struct TagInputView: View {
    @Binding var tags: [String]
    @State private var newTag = ""
    @FocusState private var isInputFocused: Bool

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                ForEach(tags, id: \.self) { tag in
                    TagChip(tag: tag) {
                        tags.removeAll { $0 == tag }
                    }
                }

                TextField(L.tagsAddTag, text: $newTag)
                    .font(.callout)
                    .frame(minWidth: 80, maxWidth: 150)
                    .focused($isInputFocused)
                    .onSubmit {
                        addTag()
                    }
                    .onChange(of: newTag) { _, newValue in
                        // Auto-submit on space or comma
                        if newValue.hasSuffix(" ") || newValue.hasSuffix(",") {
                            addTag()
                        }
                    }
            }
        }
    }

    private func addTag() {
        let trimmed = newTag.trimmingCharacters(in: .whitespacesAndNewlines)
            .trimmingCharacters(in: CharacterSet(charactersIn: ","))
            .lowercased()
        guard !trimmed.isEmpty, !tags.contains(trimmed) else {
            newTag = ""
            return
        }
        tags.append(trimmed)
        newTag = ""
        isInputFocused = true
    }
}

private struct TagChip: View {
    let tag: String
    let onRemove: () -> Void

    var body: some View {
        HStack(spacing: 4) {
            Text("#\(tag)")
                .font(.callout)

            Button(action: onRemove) {
                Image(systemName: "xmark")
                    .font(.caption2)
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(.accent.opacity(0.15))
        .clipShape(Capsule())
    }
}
