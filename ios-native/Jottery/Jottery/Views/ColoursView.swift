import SwiftUI

/// Names for the eight colours, and which colour each tag has. Pushed from
/// Settings. Everything saves as it changes.
struct ColoursView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.colorScheme) private var colorScheme
    @State private var tagFilter = ""

    var body: some View {
        Form {
            Section {
                ForEach(NoteColourSettings.keys, id: \.self) { key in
                    HStack(spacing: 12) {
                        swatch(key)
                        TextField(L.colourName(key), text: categoryName(key))
                            .autocorrectionDisabled()
                            .accessibilityLabel(L.coloursCategoryLabel(L.colourName(key)))
                    }
                }
            } header: {
                Text(L.coloursCategoriesHeader)
            } footer: {
                Text(L.coloursCategoriesFooter)
            }

            Section {
                if tags.isEmpty {
                    Text(L.coloursNoTags)
                        .foregroundStyle(.secondary)
                }
                ForEach(tags, id: \.self) { tag in
                    Picker(selection: tagColour(tag)) {
                        Text(L.editorCategoryNone).tag(String?.none)
                        ForEach(NoteColourSettings.keys, id: \.self) { key in
                            Text(appState.noteColours.displayName(for: key)).tag(String?.some(key))
                        }
                    } label: {
                        HStack(spacing: 12) {
                            if let key = appState.noteColours.colourKey(forTag: tag) {
                                swatch(key)
                            }
                            Text(verbatim: "#\(tag)")
                        }
                    }
                }
            } header: {
                Text(L.coloursTagsHeader)
            } footer: {
                Text(L.coloursTagsFooter)
            }
        }
        .navigationTitle(L.coloursTitle)
        .navigationBarTitleDisplayMode(.inline)
        .searchable(text: $tagFilter, prompt: L.coloursFilterTags)
    }

    /// Tags in use, and any that still have a colour but no longer a note.
    private var tags: [String] {
        let all = Set(appState.allTags + appState.noteColours.tagColours.keys)
            .filter { !DecryptedNote.isTitleTag($0) }
            .sorted { $0.localizedCaseInsensitiveCompare($1) == .orderedAscending }
        guard !tagFilter.isEmpty else { return all }
        return all.filter { $0.localizedCaseInsensitiveContains(tagFilter) }
    }

    private func swatch(_ key: String) -> some View {
        Circle()
            .fill(Color.noteColor(named: key, scheme: colorScheme) ?? .gray)
            .overlay(Circle().strokeBorder(.secondary.opacity(0.4)))
            .frame(width: 20, height: 20)
            .accessibilityHidden(true)
    }

    private func categoryName(_ key: String) -> Binding<String> {
        Binding(
            get: { appState.noteColours.categoryNames[key] ?? "" },
            set: { name in save { try appState.setCategoryName(name, for: key) } }
        )
    }

    private func tagColour(_ tag: String) -> Binding<String?> {
        Binding(
            get: { appState.noteColours.colourKey(forTag: tag) },
            set: { key in save { try appState.setTagColour(key, forTag: tag) } }
        )
    }

    private func save(_ change: () throws -> Void) {
        do {
            try change()
        } catch {
            appState.reportError(L.errorCouldntSaveSettings)
        }
    }
}
