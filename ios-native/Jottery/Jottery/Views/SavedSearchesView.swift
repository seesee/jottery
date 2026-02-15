import SwiftUI

struct SavedSearchesView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss

    @State private var showSaveSheet = false
    @State private var newSearchName = ""

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
                                appState.applySavedSearch(id: search.id)
                                dismiss()
                            } label: {
                                VStack(alignment: .leading, spacing: 4) {
                                    Text(search.name)
                                        .font(.headline)
                                        .foregroundStyle(.primary)
                                    Text(search.query)
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                            }
                            .swipeActions(edge: .trailing, allowsFullSwipe: true) {
                                Button(role: .destructive) {
                                    try? appState.deleteSavedSearch(id: search.id)
                                } label: {
                                    Label(L.noteListDelete, systemImage: "trash")
                                }
                            }
                        }
                    }
                }
            }
            .listStyle(.insetGrouped)
            .navigationTitle(L.savedSearchTitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L.versionHistoryDone) { dismiss() }
                }
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
        }
    }
}
