import SwiftUI

struct RecycleBinView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss

    @State private var deletedNotes: [DecryptedNote] = []
    @State private var showEmptyConfirmation = false

    var body: some View {
        NavigationStack {
            List {
                ForEach(deletedNotes) { note in
                    VStack(alignment: .leading, spacing: 4) {
                        Text(note.title)
                            .font(.headline)
                            .lineLimit(1)
                        if !note.preview.isEmpty {
                            Text(note.preview)
                                .font(.subheadline)
                                .foregroundStyle(.secondary)
                                .lineLimit(2)
                        }
                        if let deletedAt = note.deletedAt {
                            Text("Deleted \(deletedAt, format: .relative(presentation: .named))")
                                .font(.caption)
                                .foregroundStyle(.tertiary)
                        }
                    }
                    .swipeActions(edge: .leading, allowsFullSwipe: true) {
                        Button {
                            restoreNote(id: note.id)
                        } label: {
                            Label(L.recycleBinRestore, systemImage: "arrow.uturn.backward")
                        }
                        .tint(.green)
                    }
                    .swipeActions(edge: .trailing, allowsFullSwipe: false) {
                        Button(role: .destructive) {
                            permanentlyDelete(id: note.id)
                        } label: {
                            Label(L.recycleBinDeleteForever, systemImage: "trash.slash")
                        }
                    }
                }
            }
            .listStyle(.insetGrouped)
            .navigationTitle(L.recycleBinTitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L.recycleBinDone) { dismiss() }
                }
                if !deletedNotes.isEmpty {
                    ToolbarItem(placement: .destructiveAction) {
                        Button(L.recycleBinEmptyBin, role: .destructive) {
                            showEmptyConfirmation = true
                        }
                    }
                }
            }
            .alert(L.recycleBinEmptyConfirmTitle, isPresented: $showEmptyConfirmation) {
                Button(L.recycleBinEmptyConfirmAction, role: .destructive) {
                    emptyBin()
                }
                Button(L.commonCancel, role: .cancel) {}
            } message: {
                Text("This will permanently delete \(deletedNotes.count) note\(deletedNotes.count == 1 ? "" : "s"). This cannot be undone.")
            }
            .overlay {
                if deletedNotes.isEmpty {
                    ContentUnavailableView(
                        L.recycleBinEmpty,
                        systemImage: "trash",
                        description: Text(L.recycleBinEmptyDescription)
                    )
                }
            }
            .onAppear {
                loadDeletedNotes()
            }
        }
        // RecycleBinView is presented as a `.sheet` from NoteListView, which
        // sits in its own presentation layer above MainView — the toast
        // mounted there (see ToastView's doc comment) wouldn't be visible
        // here, so this view mounts its own copy bound to the same
        // `appState.userErrorMessage`.
        .overlay {
            ToastView()
        }
    }

    // MARK: - Actions

    private func loadDeletedNotes() {
        guard let noteRepo = appState.noteRepo,
              let key = appState.keyManager.masterKey else { return }
        deletedNotes = (try? noteRepo.listDeleted(key: key)) ?? []
    }

    private func restoreNote(id: String) {
        Task {
            do {
                try await appState.restoreNote(id: id)
                deletedNotes.removeAll { $0.id == id }
            } catch {
                // Restore failed — leave the note in the recycle bin list.
                appState.reportError(L.errorCouldntRestoreNote)
            }
        }
    }

    private func permanentlyDelete(id: String) {
        do {
            try appState.noteRepo?.hardDelete(id: id)
            deletedNotes.removeAll { $0.id == id }
        } catch {
            appState.reportError(L.errorCouldntDeleteNoteForever)
        }
    }

    private func emptyBin() {
        var failureCount = 0
        for note in deletedNotes {
            do {
                try appState.noteRepo?.hardDelete(id: note.id)
            } catch {
                failureCount += 1
            }
        }
        deletedNotes.removeAll()
        if failureCount > 0 {
            appState.reportError(L.errorCouldntEmptyRecycleBin)
        }
    }
}
