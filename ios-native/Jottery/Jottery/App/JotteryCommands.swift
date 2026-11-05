import SwiftUI

/// The keyboard shortcuts, in one place so they can be checked against each other
/// and against text editing. They follow the web client's where those work here:
/// web's Alt combinations gain ⌘, as Option alone types a character on iPad.
enum CommandShortcut {
    static let newNote = KeyboardShortcut("n", modifiers: .command)
    static let lock = KeyboardShortcut("l", modifiers: .command)
    static let settings = KeyboardShortcut(",", modifiers: .command)
    static let search = KeyboardShortcut("k", modifiers: .command)
    static let togglePin = KeyboardShortcut("p", modifiers: [.command, .option])
    static let duplicate = KeyboardShortcut("d", modifiers: .command)
    static let toggleArchive = KeyboardShortcut("a", modifiers: [.command, .control])
    static let copyNote = KeyboardShortcut("c", modifiers: [.command, .option])
    static let noteInfo = KeyboardShortcut("i", modifiers: [.command, .option])
    static let versionHistory = KeyboardShortcut("h", modifiers: [.command, .option])
    static let togglePreview = KeyboardShortcut("p", modifiers: [.command, .shift])
    static let toggleWordWrap = KeyboardShortcut("w", modifiers: [.command, .shift])
    static let recycleBin = KeyboardShortcut("r", modifiers: [.command, .shift])
    /// Not ⌘⌫, which deletes to the start of the line in a text view.
    static let moveToBin = KeyboardShortcut(.delete, modifiers: [.command, .shift])

    static let all = [
        newNote, lock, settings, search, togglePin, duplicate, toggleArchive, copyNote, noteInfo,
        versionHistory, togglePreview, toggleWordWrap, recycleBin, moveToBin,
    ]
}

/// What the open editor lets the menus do. These change what's on screen, so they
/// go through the editor rather than round it. Nil for a read-only note.
struct NoteEditorCommands {
    let isPreviewing: Bool
    let wrapsWords: Bool
    let togglePreview: () -> Void
    let toggleWordWrap: () -> Void
}

extension FocusedValues {
    @Entry var noteEditorCommands: NoteEditorCommands?
}

/// The app's menus, which are also what the iPadOS shortcut overlay lists.
struct JotteryCommands: Commands {
    let appState: AppState

    var body: some Commands {
        CommandGroup(after: .newItem) {
            FileCommandItems(appState: appState)
        }

        CommandGroup(replacing: .appSettings) {
            SettingsCommandItem(appState: appState)
        }

        CommandMenu(L.commandsNoteMenu) {
            NoteCommandItems(appState: appState)
        }
    }
}

// Views rather than `Commands` content, so that SwiftUI follows what they read and
// keeps the menus current.

private struct FileCommandItems: View {
    let appState: AppState

    var body: some View {
        Button(L.noteListNewNote) {
            let _ = try? appState.createNote()
        }
        .keyboardShortcut(CommandShortcut.newNote)
        .disabled(!appState.acceptsCommands)

        Button(L.noteListLock) {
            appState.lock()
        }
        .keyboardShortcut(CommandShortcut.lock)
        .disabled(!appState.acceptsCommands)
    }
}

private struct SettingsCommandItem: View {
    let appState: AppState

    var body: some View {
        Button(L.noteListSettings) {
            appState.presentedSheet = .settings
        }
        .keyboardShortcut(CommandShortcut.settings)
        .disabled(!appState.acceptsCommands)
    }
}

private struct NoteCommandItems: View {
    let appState: AppState
    @FocusedValue(\.noteEditorCommands) private var editor

    private var actions: NoteActions { NoteActions(appState: appState) }

    var body: some View {
        let note = appState.commandNote

        Button(L.commandsSearch) {
            appState.focusSearch()
        }
        .keyboardShortcut(CommandShortcut.search)
        .disabled(!appState.acceptsCommands)

        Button(L.noteListRecycleBin) {
            appState.presentedSheet = .recycleBin
        }
        .keyboardShortcut(CommandShortcut.recycleBin)
        .disabled(!appState.acceptsCommands)

        Divider()

        Button(note?.pinned == true ? L.noteListUnpin : L.noteListPin) {
            if let note { actions.togglePin(note) }
        }
        .keyboardShortcut(CommandShortcut.togglePin)
        .disabled(note == nil || note?.archived == true)

        Button(L.editorDuplicate) {
            if let note { actions.duplicate(note) }
        }
        .keyboardShortcut(CommandShortcut.duplicate)
        .disabled(note == nil)

        Button(note?.archived == true ? L.noteListUnarchive : L.noteListArchive) {
            guard let note else { return }
            if note.archived {
                Task { await actions.unarchive(note) }
            } else {
                actions.archive(note)
            }
        }
        .keyboardShortcut(CommandShortcut.toggleArchive)
        .disabled(note == nil)

        Button(L.editorCopy) {
            if let note { actions.copy(note) }
        }
        .keyboardShortcut(CommandShortcut.copyNote)
        .disabled(note == nil)

        Divider()

        Button(L.noteInfoTitle) {
            if let note { appState.presentedSheet = .noteInfo(note) }
        }
        .keyboardShortcut(CommandShortcut.noteInfo)
        .disabled(note == nil)

        Button(L.editorVersionHistory) {
            if let note { appState.presentedSheet = .versionHistory(note) }
        }
        .keyboardShortcut(CommandShortcut.versionHistory)
        .disabled((note?.version ?? 0) == 0)

        Divider()

        Button(editor?.isPreviewing == true ? L.editorHidePreview : L.editorShowPreview) {
            editor?.togglePreview()
        }
        .keyboardShortcut(CommandShortcut.togglePreview)
        .disabled(editor == nil)

        Button(editor?.wrapsWords == true ? L.editorDisableWordWrap : L.editorEnableWordWrap) {
            editor?.toggleWordWrap()
        }
        .keyboardShortcut(CommandShortcut.toggleWordWrap)
        .disabled(editor == nil)

        Divider()

        Button(L.noteListDelete, role: .destructive) {
            if let note { Task { await actions.moveToBin(note) } }
        }
        .keyboardShortcut(CommandShortcut.moveToBin)
        .disabled(note == nil)
    }
}
