import SwiftUI

struct SavedSearchesView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss

    @State private var showSaveSheet = false
    @State private var newSearchName = ""
    @State private var showAddSheet = false
    @State private var addSearchName = ""
    @State private var addSearchQuery = ""
    @State private var editingSearch: SavedSearchRepository.SavedSearch?
    @State private var editMode = EditMode.inactive
    @State private var showMoveFailed = false

    var body: some View {
        NavigationStack {
            List {
                if !appState.searchQuery.isEmpty {
                    Section {
                        Button {
                            showSaveSheet = true
                        } label: {
                            Label(L.savedSearchSaveCurrent, systemImage: "plus.circle")
                        }
                    }
                }

                if appState.savedSearches.isEmpty {
                    Section {
                        ContentUnavailableView(
                            L.savedSearchEmpty,
                            systemImage: "bookmark",
                            description: Text(L.savedSearchEmptyDescription)
                        )
                    }
                } else {
                    Section(L.savedSearchTitle) {
                        ForEach(appState.savedSearches) { search in
                            Button {
                                if editMode.isEditing {
                                    editingSearch = search
                                } else {
                                    appState.applySavedSearch(id: search.id)
                                    dismiss()
                                }
                            } label: {
                                VStack(alignment: .leading, spacing: 4) {
                                    Text(search.name)
                                        .font(.headline)
                                        .foregroundStyle(Color.primary)
                                    Text(search.query)
                                        .font(.caption)
                                        .foregroundStyle(Color.secondary)
                                }
                            }
                            .swipeActions(edge: .trailing, allowsFullSwipe: true) {
                                deleteButton(for: search)
                                editButton(for: search)
                                    .tint(.blue)
                            }
                            .contextMenu {
                                editButton(for: search)
                                deleteButton(for: search)
                            }
                        }
                        .onMove { source, destination in
                            do {
                                try appState.moveSavedSearches(fromOffsets: source, toOffset: destination)
                            } catch {
                                showMoveFailed = true
                            }
                        }
                    }
                }
            }
            .listStyle(.insetGrouped)
            .environment(\.editMode, $editMode)
            .navigationTitle(L.savedSearchTitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                // Not `EditButton`: a navigation stack keeps the edit mode that
                // changes to itself, where this view couldn't see it.
                if editMode.isEditing {
                    ToolbarItem(placement: .confirmationAction) {
                        Button(L.versionHistoryDone) {
                            withAnimation { editMode = .inactive }
                        }
                    }
                } else {
                    ToolbarItem(placement: .cancellationAction) {
                        Button(L.versionHistoryDone) { dismiss() }
                    }
                    ToolbarItem(placement: .primaryAction) {
                        Button { showAddSheet = true } label: {
                            Label(L.savedSearchAddTitle, systemImage: "plus")
                        }
                    }
                    if appState.savedSearches.count > 1 {
                        ToolbarItem(placement: .primaryAction) {
                            Button(L.savedSearchEdit) {
                                withAnimation { editMode = .active }
                            }
                        }
                    }
                }
            }
            .sheet(item: $editingSearch) { search in
                SavedSearchEditSheet(search: search)
            }
            .alert(L.errorCouldntMoveSavedSearch, isPresented: $showMoveFailed) {
                Button(L.commonOK, role: .cancel) {}
            }
            .alert(L.savedSearchSaveTitle, isPresented: $showSaveSheet) {
                TextField(L.savedSearchNamePlaceholder, text: $newSearchName)
                Button(L.savedSearchSaveAction) {
                    guard !newSearchName.isEmpty else { return }
                    try? appState.saveSearch(name: newSearchName, query: appState.searchQuery)
                    newSearchName = ""
                }
                Button(L.commonCancel, role: .cancel) {
                    newSearchName = ""
                }
            } message: {
                Text(L.savedSearchSaveMessage(appState.searchQuery))
            }
            .alert(L.savedSearchAddTitle, isPresented: $showAddSheet) {
                TextField(L.savedSearchNamePlaceholder, text: $addSearchName)
                TextField(L.savedSearchQueryPlaceholder, text: $addSearchQuery)
                Button(L.savedSearchSaveAction) {
                    guard !addSearchName.isEmpty, !addSearchQuery.isEmpty else { return }
                    try? appState.saveSearch(name: addSearchName, query: addSearchQuery)
                    addSearchName = ""
                    addSearchQuery = ""
                }
                Button(L.commonCancel, role: .cancel) {
                    addSearchName = ""
                    addSearchQuery = ""
                }
            }
        }
    }

    private func editButton(for search: SavedSearchRepository.SavedSearch) -> some View {
        Button {
            editingSearch = search
        } label: {
            Label(L.savedSearchEdit, systemImage: "pencil")
        }
    }

    private func deleteButton(for search: SavedSearchRepository.SavedSearch) -> some View {
        Button(role: .destructive) {
            try? appState.deleteSavedSearch(id: search.id)
        } label: {
            Label(L.noteListDelete, systemImage: "trash")
        }
    }
}
