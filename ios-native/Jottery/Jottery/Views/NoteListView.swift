import SwiftUI

struct NoteListView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.colorScheme) private var colorScheme
    @State private var showSettings = false
    @State private var showRecycleBin = false
    @State private var isSelectMode = false
    @State private var selectedIds: Set<String> = []
    @State private var showBulkTagSheet = false
    @State private var showBulkRemoveTagSheet = false
    @State private var showBulkColorSheet = false
    @State private var showBulkDeleteConfirm = false
    @State private var bulkExportFile: ExportFile?
    @State private var showSavedSearches = false
    @State private var showConflicts = false
    @State private var showInbox = false

    var body: some View {
        @Bindable var state = appState

        Group {
            if isSelectMode {
                multiSelectList
            } else {
                singleSelectList
            }
        }
        .listStyle(.insetGrouped)
        .refreshable { await appState.triggerSync() }
        .searchable(text: $state.searchQuery, prompt: L.noteListSearch)
        .navigationTitle(appState.showArchive ? L.noteListArchiveTitle : L.noteListTitle)
        .toolbar {
            ToolbarItemGroup(placement: .primaryAction) {
                if isSelectMode {
                    Button(L.bulkDone) {
                        isSelectMode = false
                        selectedIds.removeAll()
                    }
                } else {
                    if !appState.showArchive {
                        Button {
                            let _ = try? appState.createNote()
                        } label: {
                            Label(L.noteListNewNote, systemImage: "plus")
                        }
                    }

                    Button {
                        showSavedSearches = true
                    } label: {
                        Label(L.savedSearchTitle, systemImage: "bookmark")
                    }

                    if appState.syncEnabled {
                        Button {
                            showInbox = true
                        } label: {
                            Label(L.inboxTitle, systemImage: "tray.and.arrow.down")
                        }
                        .badge(appState.inboxCount)
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
            }

            if appState.syncEnabled && !isSelectMode {
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
                if !isSelectMode && !appState.filteredNotes.isEmpty {
                    Button {
                        isSelectMode = true
                        selectedIds.removeAll()
                    } label: {
                        Label(L.bulkSelect, systemImage: "checkmark.circle")
                    }
                }

                Button {
                    appState.showArchive.toggle()
                    appState.selectedNoteId = nil
                    isSelectMode = false
                    selectedIds.removeAll()
                } label: {
                    Label(
                        appState.showArchive ? L.noteListShowNotes : L.noteListShowArchive,
                        systemImage: appState.showArchive ? "tray.full" : "archivebox"
                    )
                }

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
            VStack(spacing: 0) {
                if isSelectMode && !selectedIds.isEmpty {
                    bulkOperationsToolbar
                }
                if !appState.searchQuery.isEmpty && !appState.displayedNotes.isEmpty {
                    Text(L.noteListSearchCount(appState.filteredCount, appState.noteCount))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 4)
                }
                if !appState.pendingConflicts.isEmpty {
                    conflictBanner
                }
                syncStatusBar
            }
        }
        .sheet(isPresented: $showConflicts) {
            ConflictResolutionView()
        }
        .sheet(isPresented: $showInbox) {
            InboxView()
        }
        .sheet(isPresented: $showSavedSearches) {
            SavedSearchesView()
        }
        .sheet(isPresented: $showSettings) {
            SettingsView()
        }
        .sheet(isPresented: $showRecycleBin) {
            RecycleBinView()
        }
        .sheet(isPresented: $showBulkTagSheet) {
            BulkAddTagsSheet(noteIds: selectedIds) {
                isSelectMode = false
                selectedIds.removeAll()
            }
        }
        .sheet(isPresented: $showBulkRemoveTagSheet) {
            BulkRemoveTagsSheet(noteIds: selectedIds) {
                isSelectMode = false
                selectedIds.removeAll()
            }
        }
        .sheet(isPresented: $showBulkColorSheet) {
            BulkSetColorSheet(noteIds: selectedIds) {
                isSelectMode = false
                selectedIds.removeAll()
            }
        }
        .sheet(item: $bulkExportFile) { file in
            ActivityView(activityItems: [file.url])
        }
        .alert(L.bulkDeleteConfirmTitle, isPresented: $showBulkDeleteConfirm) {
            Button(L.bulkDeleteConfirmAction, role: .destructive) {
                for id in selectedIds {
                    try? appState.deleteNote(id: id)
                }
                isSelectMode = false
                selectedIds.removeAll()
            }
            Button(L.commonCancel, role: .cancel) {}
        } message: {
            Text(L.bulkDeleteConfirmMessage(selectedIds.count))
        }
        .onChange(of: appState.searchQuery) { _, _ in appState.scheduleSearch() }
        .onChange(of: appState.sortOrder) { _, _ in appState.scheduleSearch() }
        .onChange(of: appState.showArchive) { _, _ in appState.scheduleSearch() }
        .onChange(of: appState.notes.count) { _, _ in appState.scheduleSearch() }
        .onChange(of: appState.archivedNotes.count) { _, _ in appState.scheduleSearch() }
        .overlay {
            if appState.showArchive && appState.archivedNotes.isEmpty {
                ContentUnavailableView(
                    L.noteListArchiveEmpty,
                    systemImage: "archivebox",
                    description: Text(L.noteListArchiveEmptyDescription)
                )
            } else if appState.displayedNotes.isEmpty && !appState.isSyncing {
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

    // MARK: - Single-Select List (normal navigation)

    private var singleSelectList: some View {
        @Bindable var state = appState
        return List(selection: $state.selectedNoteId) {
            noteRows
        }
    }

    // MARK: - Multi-Select List

    private var multiSelectList: some View {
        List(selection: $selectedIds) {
            noteRows
        }
        .environment(\.editMode, .constant(.active))
    }

    // MARK: - Note Rows

    @ViewBuilder
    private var noteRows: some View {
        ForEach(appState.filteredNotes) { note in
            NoteRowView(note: note)
                .tag(note.id)
                .swipeActions(edge: .trailing, allowsFullSwipe: true) {
                    if appState.showArchive {
                        Button {
                            try? appState.unarchiveNote(id: note.id)
                        } label: {
                            Label(L.noteListUnarchive, systemImage: "tray.and.arrow.up")
                        }
                        .tint(.blue)
                    } else {
                        Button(role: .destructive) {
                            try? appState.deleteNote(id: note.id)
                        } label: {
                            Label(L.noteListDelete, systemImage: "trash")
                        }
                    }
                }
                .swipeActions(edge: .leading, allowsFullSwipe: true) {
                    if appState.showArchive {
                        // No pin action in archive mode
                    } else {
                        Button {
                            try? appState.togglePin(id: note.id)
                        } label: {
                            Label(
                                note.pinned ? L.noteListUnpin : L.noteListPin,
                                systemImage: note.pinned ? "pin.slash" : "pin"
                            )
                        }
                        .tint(.orange)

                        Button {
                            try? appState.archiveNote(id: note.id)
                        } label: {
                            Label(L.noteListArchive, systemImage: "archivebox")
                        }
                        .tint(.indigo)
                    }
                }
        }
    }

    // MARK: - Bulk Operations Toolbar

    private var bulkOperationsToolbar: some View {
        VStack(spacing: 0) {
            Divider()
            HStack(spacing: 0) {
                Text(L.bulkSelected(selectedIds.count))
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .padding(.leading, 16)

                Spacer()

                // Select All
                Button {
                    selectedIds = Set(appState.filteredNotes.map(\.id))
                } label: {
                    Text(L.bulkSelectAll)
                        .font(.caption)
                }
                .padding(.trailing, 8)

                // Deselect All
                Button {
                    selectedIds.removeAll()
                } label: {
                    Text(L.bulkDeselectAll)
                        .font(.caption)
                }
                .padding(.trailing, 16)
            }
            .padding(.vertical, 6)

            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 12) {
                    Button {
                        showBulkTagSheet = true
                    } label: {
                        Label(L.bulkAddTags, systemImage: "tag")
                    }

                    Button {
                        showBulkRemoveTagSheet = true
                    } label: {
                        Label(L.bulkRemoveTags, systemImage: "tag.slash")
                    }

                    Button {
                        showBulkColorSheet = true
                    } label: {
                        Label(L.bulkSetColour, systemImage: "paintbrush")
                    }

                    if !appState.showArchive {
                        Button {
                            for id in selectedIds {
                                try? appState.archiveNote(id: id)
                            }
                            isSelectMode = false
                            selectedIds.removeAll()
                        } label: {
                            Label(L.noteListArchive, systemImage: "archivebox")
                        }
                    }

                    Button {
                        exportSelectedNotes()
                    } label: {
                        Label(L.bulkExport, systemImage: "square.and.arrow.up")
                    }

                    Button(role: .destructive) {
                        showBulkDeleteConfirm = true
                    } label: {
                        Label(L.noteListDelete, systemImage: "trash")
                    }
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .padding(.horizontal, 16)
            }
            .padding(.vertical, 8)
        }
        .background(.ultraThinMaterial)
    }

    // MARK: - Bulk Export

    private func exportSelectedNotes() {
        guard let key = appState.keyManager.masterKey else { return }
        let allNotes = appState.notes + appState.archivedNotes
        guard let data = try? ExportService.exportSelected(
            notes: allNotes,
            ids: selectedIds,
            attachmentRepo: appState.attachmentRepo,
            key: key
        ) else { return }

        let filename = ExportService.defaultFilename()
        let tempURL = FileManager.default.temporaryDirectory.appendingPathComponent(filename)
        try? data.write(to: tempURL)
        bulkExportFile = ExportFile(url: tempURL)
    }

    // MARK: - Conflict Banner

    private var conflictBanner: some View {
        Button {
            showConflicts = true
        } label: {
            HStack(spacing: 8) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundStyle(.orange)
                    .font(.caption)
                Text(L.conflictBanner(appState.pendingConflicts.count))
                    .font(.caption)
                    .foregroundStyle(.primary)
                Spacer()
                Image(systemName: "chevron.right")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
            .background(.ultraThinMaterial)
        }
        .buttonStyle(.plain)
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
