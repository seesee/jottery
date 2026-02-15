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

        let allNotes = appState.notes + appState.archivedNotes
        for id in noteIds {
            guard var note = allNotes.first(where: { $0.id == id }) else { continue }
            let existingSet = Set(note.tags)
            let tagsToAdd = newTags.filter { !existingSet.contains($0) }
            if !tagsToAdd.isEmpty {
                note.tags.append(contentsOf: tagsToAdd)
                try? appState.saveNote(note)
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
        let allNotes = appState.notes + appState.archivedNotes
        for id in noteIds {
            guard var note = allNotes.first(where: { $0.id == id }) else { continue }
            let before = note.tags.count
            note.tags.removeAll { selectedTags.contains($0) }
            if note.tags.count != before {
                try? appState.saveNote(note)
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
                            Text(name.capitalized)
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
        let allNotes = appState.notes + appState.archivedNotes
        for id in noteIds {
            guard var note = allNotes.first(where: { $0.id == id }) else { continue }
            if note.color != color {
                note.color = color
                try? appState.saveNote(note)
            }
        }
    }
}
