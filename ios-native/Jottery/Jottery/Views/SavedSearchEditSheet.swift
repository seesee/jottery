import SwiftUI

/// Changes a saved search's name and query, leaving it where it is in the list.
struct SavedSearchEditSheet: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss
    let search: SavedSearchRepository.SavedSearch

    @State private var name: String
    @State private var query: String
    /// Shown here, as a toast would be out of sight behind this sheet.
    @State private var failed = false

    init(search: SavedSearchRepository.SavedSearch) {
        self.search = search
        _name = State(initialValue: search.name)
        _query = State(initialValue: search.query)
    }

    private var canSave: Bool {
        !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            && !query.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    var body: some View {
        NavigationStack {
            Form {
                Section(L.savedSearchName) {
                    TextField(L.savedSearchNamePlaceholder, text: $name)
                }

                Section {
                    TextField(L.savedSearchQueryPlaceholder, text: $query, axis: .vertical)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                } header: {
                    Text(L.savedSearchQuery)
                } footer: {
                    if failed {
                        Text(L.errorCouldntSaveSavedSearch)
                            .foregroundStyle(.red)
                    }
                }
            }
            .navigationTitle(L.savedSearchEdit)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L.commonCancel) { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button(L.savedSearchSaveAction) { save() }
                        .disabled(!canSave)
                }
            }
        }
    }

    private func save() {
        do {
            try appState.updateSavedSearch(id: search.id, name: name, query: query)
            dismiss()
        } catch {
            failed = true
        }
    }
}
