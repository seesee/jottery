import Foundation

/// The parts of a note the editor changes. Everything else (pins, locks,
/// attachments, sync state) belongs to whatever changed it last, so a save applies
/// these onto the note as it stands then, not as it stood when the editor opened.
struct NoteEditorFields: Equatable {
    let noteId: String
    var content: String
    var tags: [String]
    var syntaxLanguage: String
    var wordWrap: Bool
    var color: String?
    var showPreview: Bool

    init(note: DecryptedNote) {
        noteId = note.id
        content = note.content
        tags = note.tags
        syntaxLanguage = note.syntaxLanguage
        wordWrap = note.wordWrap
        color = note.color
        showPreview = note.showPreview
    }

    func applied(to note: DecryptedNote) -> DecryptedNote {
        var updated = note
        updated.content = content
        updated.tags = tags
        updated.syntaxLanguage = syntaxLanguage
        updated.wordWrap = wordWrap
        updated.color = color
        updated.showPreview = showPreview
        return updated
    }

    /// What to do when the stored note changes while it's open.
    enum Reconciliation: Equatable {
        /// Nothing new: it's the editor's own save coming back.
        case nothing
        /// No unsaved edits, so show what arrived.
        case adopt
        /// There are unsaved edits. They stay, and what arrived goes into version
        /// history, so the save that follows doesn't lose it.
        case keepEdits
    }

    /// `baseline` is what the editor last loaded or saved. Only a difference from
    /// that is a user's edit: a difference from the live note may just mean the
    /// note changed elsewhere.
    static func reconcile(editor: Self, baseline: Self, incoming: Self) -> Reconciliation {
        if incoming == baseline || incoming == editor { return .nothing }
        return editor == baseline ? .adopt : .keepEdits
    }
}
