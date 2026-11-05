import Foundation
import Testing

@testable import Jottery

/// A row's menu acts on a note the editor may have open beside it, with an edit
/// still waiting for its debounced save.
@MainActor
struct NoteRowActionsTests {

    private func makeUnlockedState() throws -> AppState {
        let state = try TestVaults.makeState()
        state.keyManager.unlockWithKeyData(Data(repeating: 7, count: 32))
        state.isLocked = false
        return state
    }

    /// What `NoteEditorView` stages on every keystroke.
    private func stageEdit(_ content: String, of note: DecryptedNote, in state: AppState) {
        var edited = note
        edited.content = content
        state.pendingEditorNote = edited
    }

    private func stored(_ id: String, in state: AppState) throws -> DecryptedNote {
        let key = try #require(state.keyManager.masterKey)
        let repo = try #require(state.noteRepo)
        return try #require(try repo.get(id: id, key: key))
    }

    @Test func archivingKeepsAnEditStillWaitingToSave() throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "before"))
        stageEdit("typed just now", of: note, in: state)

        try state.archiveNote(id: note.id)

        let saved = try stored(note.id, in: state)
        #expect(saved.content == "typed just now")
        #expect(saved.archived)
        #expect(state.pendingEditorNote == nil)
    }

    @Test func deletingKeepsAnEditStillWaitingToSave() throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "before"))
        stageEdit("typed just now", of: note, in: state)

        try state.deleteNote(id: note.id)

        let saved = try stored(note.id, in: state)
        #expect(saved.content == "typed just now")
        #expect(saved.deleted)
    }

    @Test func anEditToAnotherNoteIsLeftAlone() throws {
        let state = try makeUnlockedState()
        let open = try #require(try state.createNote(content: "open"))
        let other = try #require(try state.createNote(content: "other"))
        stageEdit("typed just now", of: open, in: state)

        try state.archiveNote(id: other.id)

        #expect(state.pendingEditorNote?.id == open.id)
        #expect(try stored(open.id, in: state).content == "open")
    }

    /// `NoteEditorView.saveImmediately` won't save over a locked note, so neither
    /// may this. The edit was staged before the lock.
    @Test func aStagedEditDoesNotReachANoteLockedSince() throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "before"))
        stageEdit("typed just now", of: note, in: state)
        try state.toggleLock(id: note.id)

        try state.archiveNote(id: note.id)

        #expect(try stored(note.id, in: state).content == "before")
        #expect(state.pendingEditorNote == nil)
    }

    @Test func colourIsSavedWithAnEditStillWaitingToSave() throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "before"))
        stageEdit("typed just now", of: note, in: state)

        NoteActions(appState: state).setColour("blue", on: note)

        let saved = try stored(note.id, in: state)
        #expect(saved.color == "blue")
        #expect(saved.content == "typed just now")
        #expect(state.notes.first { $0.id == note.id }?.color == "blue")
    }

    @Test func colourCanBeCleared() throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "text"))
        let actions = NoteActions(appState: state)
        actions.setColour("red", on: note)

        actions.setColour(nil, on: note)

        #expect(try stored(note.id, in: state).color == nil)
    }

    @Test func aLockedNoteKeepsItsColour() throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "text"))
        try state.toggleLock(id: note.id)
        let locked = try stored(note.id, in: state)

        NoteActions(appState: state).setColour("red", on: locked)

        #expect(try stored(note.id, in: state).color == nil)
    }

    @Test func duplicatingCopiesAnEditStillWaitingToSave() throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "before"))
        stageEdit("typed just now", of: note, in: state)

        NoteActions(appState: state).duplicate(note)

        let copy = try #require(state.notes.first { $0.id != note.id })
        #expect(copy.content == "typed just now")
        #expect(state.selectedNoteId == copy.id)
    }

    @Test func pinning() throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "text"))

        NoteActions(appState: state).togglePin(note)

        #expect(try stored(note.id, in: state).pinned)
        #expect(state.notes.first?.pinned == true)
    }

    @Test func archivingAndUnarchivingFromARow() async throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "text"))
        let actions = NoteActions(appState: state)

        actions.archive(note)
        #expect(state.notes.isEmpty)
        #expect(state.archivedNotes.map(\.id) == [note.id])

        await actions.unarchive(try stored(note.id, in: state))
        #expect(state.archivedNotes.isEmpty)
        #expect(state.notes.map(\.id) == [note.id])
    }

    @Test func movingANoteToTheBin() async throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "text"))

        await NoteActions(appState: state).moveToBin(note)

        #expect(state.notes.isEmpty)
        #expect(try stored(note.id, in: state).deleted)
    }

    /// `deleteNote` only knows the active list, so an archived row would stay on
    /// screen, in the bin and the archive at once, until the next reload.
    @Test func movingAnArchivedNoteToTheBinTakesItOutOfTheArchive() async throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "text"))
        try state.archiveNote(id: note.id)
        let archived = try #require(state.archivedNotes.first)

        await NoteActions(appState: state).moveToBin(archived)

        #expect(state.archivedNotes.isEmpty)
        #expect(try stored(note.id, in: state).deleted)
    }
}
