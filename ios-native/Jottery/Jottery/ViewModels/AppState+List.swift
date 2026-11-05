import Foundation

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
}
