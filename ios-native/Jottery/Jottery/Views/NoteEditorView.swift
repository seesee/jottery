import SwiftUI

struct NoteEditorView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.colorScheme) private var colorScheme
    @State private var content: String
    @State private var tags: [String]
    @State private var syntaxLanguage: String
    @State private var wordWrap: Bool
    @State private var color: String?
    @State private var saveTask: Task<Void, Never>?

    let note: DecryptedNote

    init(note: DecryptedNote) {
        self.note = note
        _content = State(initialValue: note.content)
        _tags = State(initialValue: note.tags)
        _syntaxLanguage = State(initialValue: note.syntaxLanguage)
        _wordWrap = State(initialValue: note.wordWrap)
        _color = State(initialValue: note.color)
    }

    var body: some View {
        VStack(spacing: 0) {
            // Tags
            if !tags.isEmpty || true {
                TagInputView(tags: $tags)
                    .padding(.horizontal)
                    .padding(.vertical, 8)

                Divider()
            }

            // Editor — plain TextEditor for now, replaced by Runestone in Phase 4
            TextEditor(text: $content)
                .font(.system(.body, design: .monospaced))
                .scrollContentBackground(.hidden)
                .padding(.horizontal, 8)
                .onChange(of: content) { _, _ in
                    scheduleSave()
                }
                .onChange(of: tags) { _, _ in
                    scheduleSave()
                }
        }
        .navigationTitle(note.title)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItemGroup(placement: .primaryAction) {
                // Language picker
                Menu {
                    ForEach(syntaxLanguages, id: \.self) { lang in
                        Button(lang) {
                            syntaxLanguage = lang
                            scheduleSave()
                        }
                    }
                } label: {
                    Text(syntaxLanguage)
                        .font(.caption)
                }

                // Word wrap toggle
                Button {
                    wordWrap.toggle()
                    scheduleSave()
                } label: {
                    Image(systemName: wordWrap ? "text.word.spacing" : "arrow.right.to.line")
                }

                // Colour picker
                Menu {
                    ForEach(Color.noteColorNames, id: \.self) { name in
                        Button {
                            color = name
                            scheduleSave()
                        } label: {
                            Label(name.capitalized, systemImage: color == name ? "checkmark.circle.fill" : "circle.fill")
                        }
                    }
                    Divider()
                    Button {
                        color = nil
                        scheduleSave()
                    } label: {
                        Label("None", systemImage: color == nil ? "checkmark.circle" : "circle.slash")
                    }
                } label: {
                    Image(systemName: "paintbrush")
                        .foregroundStyle(
                            color.flatMap({ Color.noteColor(named: $0, scheme: colorScheme) }) ?? .secondary
                        )
                }

                // Pin toggle
                Button {
                    try? appState.togglePin(id: note.id)
                } label: {
                    Image(systemName: note.pinned ? "pin.fill" : "pin")
                }

                // Delete
                Button(role: .destructive) {
                    try? appState.deleteNote(id: note.id)
                } label: {
                    Image(systemName: "trash")
                }
            }
        }
        .onChange(of: note.id) { _, _ in
            // Reset state when switching notes
            content = note.content
            tags = note.tags
            syntaxLanguage = note.syntaxLanguage
            wordWrap = note.wordWrap
            color = note.color
        }
        .onDisappear {
            let hadChanges = hasChanges
            saveImmediately()
            appState.pendingEditorNote = nil
            // Trigger a background sync push after saving
            if hadChanges && appState.syncEnabled {
                Task { await appState.triggerSync() }
            }
        }
    }

    // MARK: - Auto-Save

    /// Debounced save — waits 1 second of inactivity before saving.
    private func scheduleSave() {
        appState.keyManager.recordActivity()
        // Keep AppState updated with current editor state so lock()
        // can flush pending changes before wiping the encryption key.
        updatePendingNote()
        saveTask?.cancel()
        saveTask = Task {
            try? await Task.sleep(for: .seconds(1))
            guard !Task.isCancelled else { return }
            saveImmediately()
        }
    }

    private var hasChanges: Bool {
        content != note.content ||
        tags != note.tags ||
        syntaxLanguage != note.syntaxLanguage ||
        wordWrap != note.wordWrap ||
        color != note.color
    }

    private func updatePendingNote() {
        guard hasChanges else { return }
        var updated = note
        updated.content = content
        updated.tags = tags
        updated.syntaxLanguage = syntaxLanguage
        updated.wordWrap = wordWrap
        updated.color = color
        appState.pendingEditorNote = updated
    }

    private func saveImmediately() {
        saveTask?.cancel()
        guard hasChanges else { return }
        var updated = note
        updated.content = content
        updated.tags = tags
        updated.syntaxLanguage = syntaxLanguage
        updated.wordWrap = wordWrap
        updated.color = color
        try? appState.saveNote(updated)
        appState.pendingEditorNote = nil
    }

    private var syntaxLanguages: [String] {
        ["markdown", "javascript", "typescript", "python", "perl", "json", "xml", "css", "bash", "sql", "plain"]
    }
}
