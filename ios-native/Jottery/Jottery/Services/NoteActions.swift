import UIKit

/// What can be done to a note from outside its editor: a row's swipe actions and
/// menu, and the keyboard commands. Failures go to the user as a toast, because
/// nothing that calls these has anywhere else to put them.
@MainActor
struct NoteActions {
    let appState: AppState

    func togglePin(_ note: DecryptedNote) {
        do {
            try appState.togglePin(id: note.id)
        } catch {
            appState.reportError(note.pinned ? L.errorCouldntUnpinNote : L.errorCouldntPinNote)
        }
    }

    /// `duplicateNote` copies the note as stored, so it's saved first.
    func duplicate(_ note: DecryptedNote) {
        guard appState.saveEditorEdits(toNotes: [note.id]) else { return }
        do {
            try appState.duplicateNote(id: note.id)
        } catch {
            appState.reportError(L.errorCouldntDuplicateNote)
        }
    }

    /// Does nothing to a read-only note, as in the editor.
    func setColour(_ color: String?, on note: DecryptedNote) {
        guard appState.saveEditorEdits(toNotes: [note.id]),
              var latest = appState.displayedNote(id: note.id),
              !latest.locked, !latest.archived else { return }
        latest.color = color
        do {
            try appState.saveNote(latest)
        } catch {
            appState.reportError(L.errorCouldntSetColor(1))
        }
    }

    /// What's in the editor, which may be newer than what's in the list.
    func copy(_ note: DecryptedNote, to pasteboard: UIPasteboard = .general) {
        guard appState.saveEditorEdits(toNotes: [note.id]) else { return }
        pasteboard.string = appState.displayedNote(id: note.id)?.content ?? note.content
    }

    func archive(_ note: DecryptedNote) {
        do {
            try appState.archiveNote(id: note.id)
        } catch {
            appState.reportError(L.errorCouldntArchiveNote)
        }
    }

    func unarchive(_ note: DecryptedNote) async {
        do {
            try await appState.unarchiveNote(id: note.id)
        } catch {
            appState.reportError(L.errorCouldntUnarchiveNote)
        }
    }

    /// `deleteNote` only updates the active list, so an archived note takes the
    /// route that reloads.
    func moveToBin(_ note: DecryptedNote) async {
        do {
            if note.archived {
                try await appState.deleteNotes(ids: [note.id])
            } else {
                try appState.deleteNote(id: note.id)
            }
        } catch {
            appState.reportError(L.errorCouldntDeleteNote)
        }
    }
}
