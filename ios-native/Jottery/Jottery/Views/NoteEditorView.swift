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
    @State private var didSaveDuringSession = false
    @State private var showVersionHistory = false

    let note: DecryptedNote

    init(note: DecryptedNote) {
        self.note = note
        _content = State(initialValue: note.content)
        _tags = State(initialValue: note.tags)
        _syntaxLanguage = State(initialValue: note.syntaxLanguage)
        _wordWrap = State(initialValue: note.wordWrap)
        _color = State(initialValue: note.color)
    }

    /// Whether the note is read-only (locked or archived).
    private var isReadOnly: Bool {
        note.locked || note.archived
    }

    var body: some View {
        VStack(spacing: 0) {
            // Tags
            if !tags.isEmpty || !isReadOnly {
                TagInputView(tags: isReadOnly ? .constant(tags) : $tags)
                    .disabled(isReadOnly)
                    .padding(.horizontal)
                    .padding(.vertical, 8)

                Divider()
            }

            // Syntax-highlighted editor (Runestone with tree-sitter)
            RunestoneEditorView(
                text: $content,
                syntaxLanguage: syntaxLanguage,
                wordWrap: wordWrap,
                isEditable: !isReadOnly
            )
            .onChange(of: content) { _, _ in
                scheduleSave()
            }
            .onChange(of: tags) { _, _ in
                scheduleSave()
            }

            // Attachments (read-only display of synced attachments)
            if !note.attachments.isEmpty, let attachmentRepo = appState.attachmentRepo,
               let key = appState.keyManager.masterKey {
                Divider()
                AttachmentListView(
                    attachments: note.attachments,
                    attachmentRepo: attachmentRepo,
                    encryptionKey: key
                )
            }
        }
        .navigationTitle(note.title)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Menu {
                    // Pin toggle
                    Button {
                        try? appState.togglePin(id: note.id)
                    } label: {
                        Label(note.pinned ? L.editorUnpin : L.editorPin, systemImage: note.pinned ? "pin.slash" : "pin")
                    }

                    // Word wrap toggle
                    Button {
                        wordWrap.toggle()
                        scheduleSave()
                    } label: {
                        Label(
                            wordWrap ? L.editorDisableWordWrap : L.editorEnableWordWrap,
                            systemImage: wordWrap ? "arrow.right.to.line" : "text.word.spacing"
                        )
                    }

                    // Language picker
                    Menu {
                        ForEach(syntaxLanguages, id: \.self) { lang in
                            Button {
                                syntaxLanguage = lang
                                scheduleSave()
                            } label: {
                                if lang == syntaxLanguage {
                                    Label(lang.capitalized, systemImage: "checkmark")
                                } else {
                                    Text(lang.capitalized)
                                }
                            }
                        }
                    } label: {
                        Label(L.editorLanguage(syntaxLanguage.capitalized), systemImage: "chevron.left.forwardslash.chevron.right")
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
                            Label(L.editorCategoryNone, systemImage: color == nil ? "checkmark.circle" : "circle.slash")
                        }
                    } label: {
                        Label(L.editorCategory, systemImage: "paintbrush")
                    }

                    Divider()

                    // Lock toggle
                    Button {
                        try? appState.toggleLock(id: note.id)
                    } label: {
                        Label(
                            note.locked ? L.editorUnlock : L.editorLock,
                            systemImage: note.locked ? "lock.open" : "lock"
                        )
                    }

                    // Archive toggle
                    if note.archived {
                        Button {
                            try? appState.unarchiveNote(id: note.id)
                        } label: {
                            Label(L.editorUnarchive, systemImage: "tray.and.arrow.up")
                        }
                    } else {
                        Button {
                            try? appState.archiveNote(id: note.id)
                        } label: {
                            Label(L.editorArchive, systemImage: "archivebox")
                        }
                    }

                    // Version history
                    if note.version > 0 {
                        Button {
                            showVersionHistory = true
                        } label: {
                            Label(L.editorVersionHistory, systemImage: "clock.arrow.circlepath")
                        }
                    }

                    Divider()

                    // Delete
                    Button(role: .destructive) {
                        try? appState.deleteNote(id: note.id)
                    } label: {
                        Label(L.editorDelete, systemImage: "trash")
                    }
                } label: {
                    Image(systemName: "ellipsis.circle")
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
            didSaveDuringSession = false
        }
        .sheet(isPresented: $showVersionHistory) {
            VersionHistoryView(note: note)
        }
        .onDisappear {
            saveImmediately()
            appState.pendingEditorNote = nil
            // Trigger a background sync if any save happened during this editing session
            if didSaveDuringSession && appState.syncEnabled {
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
        didSaveDuringSession = true
    }

    private var syntaxLanguages: [String] {
        ["markdown", "javascript", "typescript", "python", "perl", "json", "xml", "css", "bash", "sql", "plain"]
    }
}
