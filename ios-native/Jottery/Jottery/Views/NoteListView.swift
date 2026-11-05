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
    @State private var noteShareFile: ExportFile?
    @State private var bulkExportTask: Task<Void, Never>?
    @State private var showSavedSearches = false
    @State private var showConflicts = false
    @State private var showInbox = false
    @State private var searchIsActive = false

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
        .searchable(text: $state.searchQuery, isPresented: $searchIsActive, prompt: L.noteListSearch)
        // As in the editor: search terms stay out of the keyboard's learned words.
        .autocorrectionDisabled()
        .textInputAutocapitalization(.never)
        .navigationTitle(appState.showArchive ? L.noteListArchiveTitle : L.noteListTitle)
        .toolbar {
            ToolbarItemGroup(placement: .primaryAction) {
                if isSelectMode {
                    Button(L.bulkDone) {
                        isSelectMode = false
                        selectedIds.removeAll()
                    }
                } else {
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
                        Picker(L.noteListSort, selection: Binding(
                            get: { appState.sortOrder },
                            set: { appState.setSortOrder($0) }
                        )) {
                            ForEach(SortOrder.allCases, id: \.self) { order in
                                Text(order.displayName).tag(order)
                            }
                        }
                    } label: {
                        Label(L.noteListSort, systemImage: "arrow.up.arrow.down")
                    }
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
        .shareTemporaryFile($bulkExportFile)
        .shareTemporaryFile($noteShareFile)
        .onDisappear {
            // The app can lock during an export, which takes this view away.
            bulkExportTask?.cancel()
        }
        .alert(L.bulkDeleteConfirmTitle, isPresented: $showBulkDeleteConfirm) {
            Button(L.bulkDeleteConfirmAction, role: .destructive) {
                let ids = selectedIds
                isSelectMode = false
                selectedIds.removeAll()
                Task {
                    do {
                        try await appState.deleteNotes(ids: ids)
                    } catch {
                        appState.reportError(L.errorCouldntDeleteNotes(ids.count))
                    }
                }
            }
            Button(L.commonCancel, role: .cancel) {}
        } message: {
            Text(L.bulkDeleteConfirmMessage(selectedIds.count))
        }
        .onChange(of: appState.searchFocused) { _, focused in
            if focused {
                searchIsActive = true
                appState.searchFocused = false
            }
        }
        .onAppear {
            if appState.searchFocused {
                Task { @MainActor in
                    try? await Task.sleep(for: .milliseconds(300))
                    searchIsActive = true
                    appState.searchFocused = false
                }
            }
        }
        .onChange(of: appState.searchQuery) { _, _ in appState.scheduleSearch() }
        .onChange(of: appState.sortOrder) { _, _ in appState.scheduleSearch() }
        .onChange(of: appState.showArchive) { _, _ in appState.scheduleSearch() }
        .onChange(of: appState.notes.count) { _, _ in appState.scheduleSearch() }
        .onChange(of: appState.archivedNotes.count) { _, _ in appState.scheduleSearch() }
        .overlay(alignment: .bottomTrailing) {
            if !appState.showArchive && !isSelectMode {
                Button {
                    let _ = try? appState.createNote()
                } label: {
                    Image(systemName: "plus")
                        .font(.title2.weight(.semibold))
                        .foregroundStyle(.white)
                        .frame(width: 56, height: 56)
                        .background(Color.accentColor, in: Circle())
                        .shadow(color: .black.opacity(0.2), radius: 4, y: 2)
                }
                .padding(.trailing, 20)
                .padding(.bottom, 20)
                .accessibilityLabel(L.noteListNewNote)
            }
        }
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
            NoteRowView(
                note: note,
                hasConflict: appState.pendingConflicts.contains { $0.id == note.id }
            )
                .tag(note.id)
                .contextMenu {
                    NoteShareActions(note: note, shareFile: $noteShareFile)
                }
                .swipeActions(edge: .trailing, allowsFullSwipe: true) {
                    if appState.showArchive {
                        Button {
                            Task {
                                do {
                                    try await appState.unarchiveNote(id: note.id)
                                } catch {
                                    appState.reportError(L.errorCouldntUnarchiveNote)
                                }
                            }
                        } label: {
                            Label(L.noteListUnarchive, systemImage: "tray.and.arrow.up")
                        }
                        .tint(.blue)
                    } else {
                        Button(role: .destructive) {
                            do {
                                try appState.deleteNote(id: note.id)
                            } catch {
                                appState.reportError(L.errorCouldntDeleteNote)
                            }
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
                            let wasPinned = note.pinned
                            do {
                                try appState.togglePin(id: note.id)
                            } catch {
                                appState.reportError(wasPinned ? L.errorCouldntUnpinNote : L.errorCouldntPinNote)
                            }
                        } label: {
                            Label(
                                note.pinned ? L.noteListUnpin : L.noteListPin,
                                systemImage: note.pinned ? "pin.slash" : "pin"
                            )
                        }
                        .tint(.orange)

                        Button {
                            do {
                                try appState.archiveNote(id: note.id)
                            } catch {
                                appState.reportError(L.errorCouldntArchiveNote)
                            }
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
                            let ids = selectedIds
                            isSelectMode = false
                            selectedIds.removeAll()
                            Task {
                                do {
                                    try await appState.archiveNotes(ids: ids)
                                } catch {
                                    appState.reportError(L.errorCouldntArchiveNotes(ids.count))
                                }
                            }
                        } label: {
                            Label(L.noteListArchive, systemImage: "archivebox")
                        }
                    }

                    Button {
                        exportSelectedNotes()
                    } label: {
                        if bulkExportTask != nil {
                            ProgressView()
                        } else {
                            Label(L.bulkExport, systemImage: "square.and.arrow.up")
                        }
                    }
                    .disabled(bulkExportTask != nil)

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
        let selected = (appState.notes + appState.archivedNotes).filter { selectedIds.contains($0.id) }
        bulkExportTask = Task {
            defer { bulkExportTask = nil }
            do {
                let url = try await ExportService.exportToTemporaryFile(
                    notes: selected,
                    attachmentRepo: appState.attachmentRepo,
                    key: key
                )
                bulkExportFile = ExportFile(url: url)
            } catch is CancellationError {
                // The list went away, so there's nobody to tell.
            } catch {
                appState.reportError(L.errorCouldntExport)
            }
        }
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
