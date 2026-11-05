import SwiftUI

struct NoteListView: View {
    /// Whether the tags go at the top of this list. Otherwise they're a sheet.
    /// Decided by `MainView`, as a split view's column can be told it's compact.
    let showsTagsInList: Bool

    @Environment(AppState.self) private var appState
    @Environment(\.colorScheme) private var colorScheme
    @State private var isSelectMode = false
    @State private var selectedIds: Set<String> = []
    @State private var showBulkTagSheet = false
    @State private var showBulkRemoveTagSheet = false
    @State private var showBulkColorSheet = false
    @State private var showBulkCombineSheet = false
    @State private var showBulkDeleteConfirm = false
    @State private var bulkExportFile: ExportFile?
    @State private var noteShareFile: ExportFile?
    @State private var bulkExportTask: Task<Void, Never>?
    @State private var showSavedSearches = false
    @State private var showConflicts = false
    @State private var showInbox = false
    @State private var showTags = false
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
        .searchSuggestions {
            let completions = SearchSuggestions.tagCompletions(for: appState.searchQuery, tags: appState.allTags)
            // Anything here takes the place of the results, so typing shows them.
            if appState.searchQuery.isEmpty || !completions.isEmpty {
                SearchSuggestionRows(tagCompletions: completions)
            }
        }
        .onSubmit(of: .search) { appState.recordRecentSearch() }
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

                if !isSelectMode && !showsTagsInList {
                    Button {
                        showTags = true
                    } label: {
                        Label(L.tagsTitle, systemImage: "tag")
                    }
                }

                Button {
                    toggleArchiveView()
                } label: {
                    Label(
                        appState.showArchive ? L.noteListShowNotes : L.noteListShowArchive,
                        systemImage: appState.showArchive ? "tray.full" : "archivebox"
                    )
                }

                Button {
                    appState.presentedSheet = .recycleBin
                } label: {
                    Label(L.noteListRecycleBin, systemImage: "trash")
                }

                Button {
                    appState.presentedSheet = .settings
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
        .sheet(isPresented: $showTags) {
            TagBrowserSheet()
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
        .sheet(isPresented: $showBulkCombineSheet) {
            BulkCombineSheet(noteIds: selectedIds) {
                isSelectMode = false
                selectedIds.removeAll()
            }
        }
        .background { findShortcut }
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
        .onChange(of: appState.selectedNoteId) { _, opened in
            if opened != nil { appState.recordRecentSearch() }
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
            if appState.filteredNotes.isEmpty && !appState.searchQuery.isEmpty {
                noSearchResults
            } else if appState.showArchive && appState.archivedNotes.isEmpty {
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
            }
        }
    }

    // MARK: - Find

    /// ⌘F finds in the open note, and the editor owns that. With no note open
    /// there's nothing to find in, so it goes to the search field. This is here and
    /// not in the menus so that it can't exist at the same time as the editor's.
    @ViewBuilder
    private var findShortcut: some View {
        if appState.selectedNoteId == nil {
            Button(L.commandsSearch) {
                searchIsActive = true
            }
            .keyboardShortcut("f", modifiers: .command)
            .frame(width: 0, height: 0)
            .opacity(0)
            .accessibilityHidden(true)
        }
    }

    // MARK: - No Results

    /// What a search that found nothing says, with a way to what it would have
    /// found in the other view.
    @ViewBuilder
    private var noSearchResults: some View {
        let elsewhere = appState.matchesInOtherView
        if elsewhere > 0 {
            ContentUnavailableView {
                Label(L.searchNoResults(appState.searchQuery), systemImage: "magnifyingglass")
            } actions: {
                Button(appState.showArchive ? L.searchMatchesInNotes(elsewhere) : L.searchMatchesInArchive(elsewhere)) {
                    toggleArchiveView()
                }
                .buttonStyle(.bordered)
            }
        } else {
            ContentUnavailableView.search(text: appState.searchQuery)
        }
    }

    private func toggleArchiveView() {
        appState.toggleArchiveView()
        isSelectMode = false
        selectedIds.removeAll()
    }

    // MARK: - Single-Select List (normal navigation)

    private var singleSelectList: some View {
        @Bindable var state = appState
        return List(selection: $state.selectedNoteId) {
            if showsTagsInList {
                TagSidebarSection()
            }
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

    private var noteActions: NoteActions { NoteActions(appState: appState) }

    @ViewBuilder
    private var noteRows: some View {
        ForEach(appState.filteredNotes) { note in
            NoteRowView(
                note: note,
                hasConflict: appState.pendingConflicts.contains { $0.id == note.id }
            )
                .tag(note.id)
                .contextMenu {
                    NoteRowMenu(note: note, shareFile: $noteShareFile, isSelecting: isSelectMode)
                }
                .swipeActions(edge: .trailing, allowsFullSwipe: true) {
                    if appState.showArchive {
                        Button {
                            Task { await noteActions.unarchive(note) }
                        } label: {
                            Label(L.noteListUnarchive, systemImage: "tray.and.arrow.up")
                        }
                        .tint(.blue)
                    } else {
                        Button(role: .destructive) {
                            Task { await noteActions.moveToBin(note) }
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
                            noteActions.togglePin(note)
                        } label: {
                            Label(
                                note.pinned ? L.noteListUnpin : L.noteListPin,
                                systemImage: note.pinned ? "pin.slash" : "pin"
                            )
                        }
                        .tint(.orange)

                        Button {
                            noteActions.archive(note)
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

                    if !appState.showArchive && selectedIds.count >= 2 {
                        Button {
                            showBulkCombineSheet = true
                        } label: {
                            Label(L.bulkCombine, systemImage: "rectangle.stack")
                        }
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
