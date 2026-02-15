import SwiftUI

struct NoteListView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.colorScheme) private var colorScheme
    @State private var showSettings = false
    @State private var showRecycleBin = false

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
                            Label(L.noteListDelete, systemImage: "trash")
                        }
                    }
                    .swipeActions(edge: .leading, allowsFullSwipe: true) {
                        Button {
                            try? appState.togglePin(id: note.id)
                        } label: {
                            Label(
                                note.pinned ? L.noteListUnpin : L.noteListPin,
                                systemImage: note.pinned ? "pin.slash" : "pin"
                            )
                        }
                        .tint(.orange)
                    }
            }
        }
        .listStyle(.insetGrouped)
        .refreshable { await appState.triggerSync() }
        .searchable(text: $state.searchQuery, prompt: L.noteListSearch)
        .navigationTitle(L.noteListTitle)
        .toolbar {
            ToolbarItemGroup(placement: .primaryAction) {
                Button {
                    let _ = try? appState.createNote()
                } label: {
                    Label(L.noteListNewNote, systemImage: "plus")
                }

                Menu {
                    Picker(L.noteListSort, selection: $state.sortOrder) {
                        ForEach(SortOrder.allCases, id: \.self) { order in
                            Text(order.displayName).tag(order)
                        }
                    }
                } label: {
                    Label(L.noteListSort, systemImage: "arrow.up.arrow.down")
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
                    showRecycleBin = true
                } label: {
                    Label(L.noteListRecycleBin, systemImage: "trash")
                }

                Button {
                    showSettings = true
                } label: {
                    Label(L.noteListSettings, systemImage: "gear")
                }

                Button {
                    appState.lock()
                } label: {
                    Label(L.noteListLock, systemImage: "lock")
                }
            }
        }
        .safeAreaInset(edge: .bottom) {
            syncStatusBar
        }
        .sheet(isPresented: $showSettings) {
            SettingsView()
        }
        .sheet(isPresented: $showRecycleBin) {
            RecycleBinView()
        }
        .overlay {
            if appState.notes.isEmpty && !appState.isSyncing {
                ContentUnavailableView(
                    L.noteListNoNotes,
                    systemImage: "doc.text",
                    description: Text(L.noteListNoNotesDescription)
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
