import SwiftUI

struct NoteListView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.colorScheme) private var colorScheme
    @State private var showSettings = false

    var body: some View {
        @Bindable var state = appState

        List(selection: $state.selectedNoteId) {
            ForEach(appState.filteredNotes) { note in
                NoteRowView(note: note)
                    .tag(note.id)
                    .swipeActions(edge: .trailing, allowsFullSwipe: true) {
                        Button(role: .destructive) {
                            try? appState.deleteNote(id: note.id)
                        } label: {
                            Label("Delete", systemImage: "trash")
                        }
                    }
                    .swipeActions(edge: .leading, allowsFullSwipe: true) {
                        Button {
                            try? appState.togglePin(id: note.id)
                        } label: {
                            Label(
                                note.pinned ? "Unpin" : "Pin",
                                systemImage: note.pinned ? "pin.slash" : "pin"
                            )
                        }
                        .tint(.orange)
                    }
            }
        }
        .listStyle(.insetGrouped)
        .searchable(text: $state.searchQuery, prompt: "Search notes")
        .navigationTitle("Notes")
        .toolbar {
            ToolbarItemGroup(placement: .primaryAction) {
                Button {
                    let _ = try? appState.createNote()
                } label: {
                    Label("New Note", systemImage: "plus")
                }

                Menu {
                    Picker("Sort", selection: $state.sortOrder) {
                        ForEach(SortOrder.allCases, id: \.self) { order in
                            Text(order.displayName).tag(order)
                        }
                    }
                } label: {
                    Label("Sort", systemImage: "arrow.up.arrow.down")
                }
            }

            if appState.syncEnabled {
                ToolbarItem(placement: .primaryAction) {
                    Button {
                        Task { @MainActor in await appState.triggerSync() }
                    } label: {
                        if appState.isSyncing {
                            ProgressView()
                                .controlSize(.small)
                        } else {
                            Image(systemName: "arrow.triangle.2.circlepath")
                        }
                    }
                    .disabled(appState.isSyncing)
                }
            }

            ToolbarItemGroup(placement: .secondaryAction) {
                Button {
                    showSettings = true
                } label: {
                    Label("Settings", systemImage: "gear")
                }

                Button {
                    appState.lock()
                } label: {
                    Label("Lock", systemImage: "lock")
                }
            }
        }
        .safeAreaInset(edge: .bottom) {
            syncStatusBar
        }
        .sheet(isPresented: $showSettings) {
            SettingsView()
        }
        .overlay {
            if appState.notes.isEmpty && !appState.isSyncing {
                ContentUnavailableView(
                    "No Notes",
                    systemImage: "doc.text",
                    description: Text("Tap + to create your first note.")
                )
            } else if appState.filteredNotes.isEmpty && !appState.searchQuery.isEmpty {
                ContentUnavailableView.search(text: appState.searchQuery)
            }
        }
    }

    // MARK: - Sync Status Bar

    @ViewBuilder
    private var syncStatusBar: some View {
        if let message = appState.syncStatusMessage {
            HStack(spacing: 8) {
                if appState.isSyncing {
                    ProgressView()
                        .controlSize(.small)
                }
                Text(message)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 8)
            .background(.ultraThinMaterial)
            .transition(.move(edge: .bottom).combined(with: .opacity))
            .animation(.easeInOut(duration: 0.25), value: message)
        } else if let error = appState.syncError {
            HStack(spacing: 8) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundStyle(.red)
                    .font(.caption)
                Text(error)
                    .font(.caption)
                    .foregroundStyle(.red)
                    .lineLimit(1)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 8)
            .background(.ultraThinMaterial)
            .transition(.move(edge: .bottom).combined(with: .opacity))
        }
    }
}
