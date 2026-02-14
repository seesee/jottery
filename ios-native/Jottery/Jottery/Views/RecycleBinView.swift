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
                            Label("Restore", systemImage: "arrow.uturn.backward")
                        }
                        .tint(.green)
                    }
                    .swipeActions(edge: .trailing, allowsFullSwipe: false) {
                        Button(role: .destructive) {
                            permanentlyDelete(id: note.id)
                        } label: {
                            Label("Delete Forever", systemImage: "trash.slash")
                        }
                    }
                }
            }
            .listStyle(.insetGrouped)
            .navigationTitle("Recycle Bin")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Done") { dismiss() }
                }
                if !deletedNotes.isEmpty {
                    ToolbarItem(placement: .destructiveAction) {
                        Button("Empty Bin", role: .destructive) {
                            showEmptyConfirmation = true
                        }
                    }
                }
            }
            .alert("Empty Recycle Bin?", isPresented: $showEmptyConfirmation) {
                Button("Delete All", role: .destructive) {
                    emptyBin()
                }
                Button("Cancel", role: .cancel) {}
            } message: {
                Text("This will permanently delete \(deletedNotes.count) note\(deletedNotes.count == 1 ? "" : "s"). This cannot be undone.")
            }
            .overlay {
                if deletedNotes.isEmpty {
                    ContentUnavailableView(
                        "Recycle Bin Empty",
                        systemImage: "trash",
                        description: Text("Deleted notes will appear here.")
                    )
                }
            }
            .onAppear {
                loadDeletedNotes()
            }
        }
    }

    // MARK: - Actions

    private func loadDeletedNotes() {
        guard let noteRepo = appState.noteRepo,
              let key = appState.keyManager.masterKey else { return }
        deletedNotes = (try? noteRepo.listDeleted(key: key)) ?? []
    }

    private func restoreNote(id: String) {
        try? appState.restoreNote(id: id)
        deletedNotes.removeAll { $0.id == id }
    }

    private func permanentlyDelete(id: String) {
        try? appState.noteRepo?.hardDelete(id: id)
        deletedNotes.removeAll { $0.id == id }
    }

    private func emptyBin() {
        for note in deletedNotes {
            try? appState.noteRepo?.hardDelete(id: note.id)
        }
        deletedNotes.removeAll()
    }
}
