import SwiftUI

// MARK: - Bulk Add Tags Sheet

struct BulkAddTagsSheet: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss
    let noteIds: Set<String>
    let onComplete: () -> Void

    @State private var tagText = ""

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField(L.bulkTagsPlaceholder, text: $tagText)
                } footer: {
                    Text(L.bulkTagsHint)
                }

                Section {
                    Text(L.bulkSelected(noteIds.count))
                        .foregroundStyle(.secondary)
                }
            }
            .navigationTitle(L.bulkAddTags)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L.commonCancel) { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button(L.bulkApply) {
                        applyTags()
                        dismiss()
                        onComplete()
                    }
                    .disabled(tagText.trimmingCharacters(in: .whitespaces).isEmpty)
                }
            }
        }
    }

    private func applyTags() {
        let newTags = tagText.split(separator: ",")
            .map { String($0).trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }
        guard !newTags.isEmpty else { return }

        Task {
            let failures = try? await appState.updateNotes(ids: noteIds) { note in
                let existing = Set(note.tags)
                let tagsToAdd = newTags.filter { !existing.contains($0) }
                note.tags.append(contentsOf: tagsToAdd)
                return !tagsToAdd.isEmpty
            }
            if failures != 0 {
                appState.reportError(L.errorCouldntAddTags(failures ?? noteIds.count))
            }
        }
    }
}

// MARK: - Bulk Remove Tags Sheet

struct BulkRemoveTagsSheet: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss
    let noteIds: Set<String>
    let onComplete: () -> Void

    @State private var selectedTags: Set<String> = []

    /// Union of all tags from selected notes.
    private var allTags: [String] {
        let allNotes = appState.notes + appState.archivedNotes
        var tagSet = Set<String>()
        for id in noteIds {
            if let note = allNotes.first(where: { $0.id == id }) {
                tagSet.formUnion(note.tags)
            }
        }
        return tagSet.sorted()
    }

    var body: some View {
        NavigationStack {
            List(allTags, id: \.self) { tag in
                Button {
                    if selectedTags.contains(tag) {
                        selectedTags.remove(tag)
                    } else {
                        selectedTags.insert(tag)
                    }
                } label: {
                    HStack {
                        Text("#\(tag)")
                        Spacer()
                        if selectedTags.contains(tag) {
                            Image(systemName: "checkmark")
                                .foregroundStyle(.accent)
                        }
                    }
                }
                .foregroundStyle(.primary)
            }
            .navigationTitle(L.bulkRemoveTags)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L.commonCancel) { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button(L.bulkApply) {
                        removeTags()
                        dismiss()
                        onComplete()
                    }
                    .disabled(selectedTags.isEmpty)
                }
            }
        }
    }

    private func removeTags() {
        let removing = selectedTags
        Task {
            let failures = try? await appState.updateNotes(ids: noteIds) { note in
                let before = note.tags.count
                note.tags.removeAll { removing.contains($0) }
                return note.tags.count != before
            }
            if failures != 0 {
                appState.reportError(L.errorCouldntRemoveTags(failures ?? noteIds.count))
            }
        }
    }
}

// MARK: - Bulk Set Colour Sheet

struct BulkSetColorSheet: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss
    @Environment(\.colorScheme) private var colorScheme
    let noteIds: Set<String>
    let onComplete: () -> Void

    var body: some View {
        NavigationStack {
            List {
                ForEach(Color.noteColorNames, id: \.self) { name in
                    Button {
                        applyColor(name)
                        dismiss()
                        onComplete()
                    } label: {
                        HStack {
                            Circle()
                                .fill(Color.noteColor(named: name, scheme: colorScheme) ?? .gray)
                                .frame(width: 24, height: 24)
                            Text(appState.noteColours.displayName(for: name))
                        }
                    }
                    .foregroundStyle(.primary)
                }

                Button {
                    applyColor(nil)
                    dismiss()
                    onComplete()
                } label: {
                    HStack {
                        Image(systemName: "circle.slash")
                            .frame(width: 24, height: 24)
                        Text(L.editorCategoryNone)
                    }
                }
                .foregroundStyle(.primary)
            }
            .navigationTitle(L.bulkSetColour)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L.commonCancel) { dismiss() }
                }
            }
        }
    }

    private func applyColor(_ color: String?) {
        Task {
            let failures = try? await appState.updateNotes(ids: noteIds) { note in
                guard note.color != color else { return false }
                note.color = color
                return true
            }
            if failures != 0 {
                appState.reportError(L.errorCouldntSetColor(failures ?? noteIds.count))
            }
        }
    }
}
