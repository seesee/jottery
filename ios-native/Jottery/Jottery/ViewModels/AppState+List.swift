import SwiftUI

/// What the note list needs beyond the plain note operations in `AppState`.
extension AppState {

    // MARK: - The Editor's Unsaved Edits

    /// Save an edit the editor is still holding for any of these notes, for an
    /// action that's about to read the note or take it away. Its debounced save
    /// would come too late: `NoteEditorView` won't save over a note that has since
    /// been archived or deleted. Returns false if the edit couldn't be saved, which
    /// the user has been told, and the action shouldn't go ahead.
    @discardableResult
    func saveEditorEdits(toNotes ids: Set<String>) -> Bool {
        guard let pending = pendingEditorNote, ids.contains(pending.id) else { return true }
        // Staged before the note became read-only, so it's no longer wanted.
        guard let latest = displayedNote(id: pending.id), !latest.locked, !latest.archived else {
            pendingEditorNote = nil
            return true
        }
        return saveEditorEdit(NoteEditorFields(note: pending))
    }

    // MARK: - Combining Notes

    /// Make one note from these, oldest first, send them to the bin and open the
    /// new one. Notes that can't be changed are left out, as `NoteCombining` says.
    /// Returns the new note's id.
    @discardableResult
    func combineNotes(ids: Set<String>, mergeTags: Bool) async throws -> String? {
        guard !isReKeying else { throw AppStateError.vaultBusy }
        guard let noteRepo, let key = keyManager.masterKey else { return nil }
        guard saveEditorEdits(toNotes: ids) else { return nil }

        let selected = NoteCombining.combinable((notes + archivedNotes).filter { ids.contains($0.id) })
        guard selected.count >= 2 else { throw NoteCombining.Failure.tooFewNotes }

        let combinedId = try await Task.detached(priority: .userInitiated) {
            try noteRepo.combine(selected, mergeTags: mergeTags, key: key)
        }.value

        try await loadNotes()
        selectedNoteId = combinedId
        scheduleAutoSync()
        return combinedId
    }

    // MARK: - Importing

    /// Bring the lists up to date once the import sheet has gone. A failure here
    /// leaves the list out of date, so it's reported rather than ignored.
    func reloadAfterImport() async {
        do {
            try await loadNotes()
        } catch {
            reportError(error.localizedDescription)
        }
    }

    // MARK: - Saved Searches

    func updateSavedSearch(id: String, name: String, query: String) throws {
        guard !isReKeying else { throw AppStateError.vaultBusy }
        guard let savedSearchRepo, let key = keyManager.masterKey else { return }
        try savedSearchRepo.update(id: id, name: name, query: query, key: key)
        if let index = savedSearches.firstIndex(where: { $0.id == id }) {
            savedSearches[index].name = name
            savedSearches[index].query = query
        }
        scheduleAutoSync()
    }

    /// What a list's `onMove` hands over.
    func moveSavedSearches(fromOffsets source: IndexSet, toOffset destination: Int) throws {
        guard let savedSearchRepo else { return }
        var moved = savedSearches
        moved.move(fromOffsets: source, toOffset: destination)
        savedSearches = try savedSearchRepo.reorder(moved)
        scheduleAutoSync()
    }

    // MARK: - Searching

    /// Remember the current search, once it's been used: submitted, or a result
    /// opened. Filtering as each letter goes in doesn't count.
    func recordRecentSearch() {
        recentSearches.record(searchQuery)
    }

    /// Show the notes with this tag, in whichever view is showing.
    func browse(tag: String) {
        searchQuery = SearchSuggestions.query(forTag: tag)
    }

    /// Change between the notes and the archive, leaving the search as it is.
    func toggleArchiveView() {
        showArchive.toggle()
        selectedNoteId = nil
    }

    // MARK: - Keyboard Commands

    /// Whether there's a vault open to act on.
    var acceptsCommands: Bool {
        !isLocked && !isFirstLaunch
    }

    /// The note a keyboard command acts on: the open one.
    var commandNote: DecryptedNote? {
        guard acceptsCommands else { return nil }
        return selectedNoteId.flatMap(displayedNote(id:))
    }

    /// Go to the search field, from wherever the user is. The list holds it, and
    /// isn't on screen while a phone shows a note or an iPad hides the sidebar.
    func focusSearch() {
        guard acceptsCommands else { return }
        listRevealRequests += 1
        searchFocused = true
    }
}
