import SwiftUI

/// Tag editing chip input — allows adding/removing tags with autocomplete suggestions.
struct TagInputView: View {
    @Environment(AppState.self) private var appState
    @Binding var tags: [String]
    @State private var newTag = ""
    @State private var showSuggestions = false
    @FocusState private var isInputFocused: Bool

    /// Filtered tag suggestions based on current input.
    private var suggestions: [String] {
        let input = newTag.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !input.isEmpty else { return [] }
        return appState.allTags
            .filter { $0.lowercased().contains(input) && !tags.contains($0) }
            .prefix(5)
            .map { $0 }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
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
                            } else {
                                showSuggestions = !suggestions.isEmpty
                            }
                        }
                        .onChange(of: isInputFocused) { _, focused in
                            if !focused {
                                // Delay hiding so tap on suggestion registers
                                DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) {
                                    showSuggestions = false
                                }
                            }
                        }
                }
            }

            // Autocomplete suggestions
            if showSuggestions && !suggestions.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        ForEach(suggestions, id: \.self) { suggestion in
                            Button {
                                selectSuggestion(suggestion)
                            } label: {
                                Text("#\(suggestion)")
                                    .font(.caption)
                                    .padding(.horizontal, 8)
                                    .padding(.vertical, 4)
                                    .background(Color.secondary.opacity(0.1))
                                    .clipShape(Capsule())
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    .padding(.top, 4)
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
        showSuggestions = false
        isInputFocused = true
    }

    private func selectSuggestion(_ tag: String) {
        guard !tags.contains(tag) else { return }
        tags.append(tag)
        newTag = ""
        showSuggestions = false
        isInputFocused = true
    }
}

private struct TagChip: View {
    let tag: String
    let onRemove: () -> Void

    private var isTitle: Bool { DecryptedNote.isTitleTag(tag) }
    private var titleValue: String? { DecryptedNote.titleTagValue(tag) }

    var body: some View {
        HStack(spacing: 4) {
            if isTitle, let value = titleValue {
                Text("title: \(value)")
                    .font(.callout)
                    .italic()
            } else {
                Text("#\(tag)")
                    .font(.callout)
            }

            Button(action: onRemove) {
                Image(systemName: "xmark")
                    .font(.caption2)
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(isTitle ? Color.secondary.opacity(0.15) : Color.accentColor.opacity(0.15))
        .clipShape(Capsule())
    }
}
