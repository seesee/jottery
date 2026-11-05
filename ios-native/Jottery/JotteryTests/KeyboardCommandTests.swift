import SwiftUI
import Testing
import UIKit

@testable import Jottery

/// The commands' menu items and key presses can't be reached from here. What they
/// do can, and so can the list of shortcuts.
@MainActor
struct KeyboardCommandTests {

    private func makeUnlockedState() throws -> AppState {
        let state = try TestVaults.makeState()
        state.keyManager.unlockWithKeyData(Data(repeating: 7, count: 32))
        state.isLocked = false
        state.isFirstLaunch = false
        return state
    }

    // MARK: - The Shortcuts

    @Test func noTwoCommandsShareAShortcut() {
        let shortcuts = CommandShortcut.all
        #expect(Set(shortcuts).count == shortcuts.count)
    }

    /// What a text view already does with these has to keep working while typing.
    /// ⌘F belongs to find in the note, and is handled in the note list instead.
    @Test func nothingTakesAKeyFromTextEditing() {
        let reserved: [KeyboardShortcut] = ["a", "b", "c", "f", "g", "i", "u", "v", "x", "z"]
            .map { KeyboardShortcut(KeyEquivalent(Character($0)), modifiers: .command) }
            + [
                KeyboardShortcut("z", modifiers: [.command, .shift]),
                KeyboardShortcut("g", modifiers: [.command, .shift]),
                KeyboardShortcut(.delete, modifiers: .command),
                KeyboardShortcut(.delete, modifiers: .option),
            ]
        #expect(Set(CommandShortcut.all).isDisjoint(with: reserved))
    }

    /// Option alone types a character, and iPadOS keeps ⌥⌘D for the Dock.
    @Test func everyShortcutNeedsCommand() {
        for shortcut in CommandShortcut.all {
            #expect(shortcut.modifiers.contains(.command))
        }
        #expect(!CommandShortcut.all.contains(KeyboardShortcut("d", modifiers: [.command, .option])))
    }

    @Test func theOnesWebHasMatch() {
        #expect(CommandShortcut.search == KeyboardShortcut("k", modifiers: .command))
        #expect(CommandShortcut.settings == KeyboardShortcut(",", modifiers: .command))
        #expect(CommandShortcut.duplicate == KeyboardShortcut("d", modifiers: .command))
        #expect(CommandShortcut.recycleBin == KeyboardShortcut("r", modifiers: [.command, .shift]))
        #expect(CommandShortcut.togglePreview == KeyboardShortcut("p", modifiers: [.command, .shift]))
        #expect(CommandShortcut.toggleWordWrap == KeyboardShortcut("w", modifiers: [.command, .shift]))
    }

    // MARK: - What They Act On

    @Test func theyActOnTheOpenNote() throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "text"))

        #expect(state.commandNote?.id == note.id)

        state.selectedNoteId = nil
        #expect(state.commandNote == nil)
    }

    @Test func anOpenArchivedNoteCounts() throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "text"))
        try state.archiveNote(id: note.id)
        state.selectedNoteId = note.id

        #expect(state.commandNote?.id == note.id)
    }

    @Test func nothingIsAvailableBehindTheLock() throws {
        let state = try makeUnlockedState()
        _ = try state.createNote(content: "text")
        #expect(state.acceptsCommands)

        state.lock()

        #expect(!state.acceptsCommands)
        #expect(state.commandNote == nil)
    }

    @Test func aVaultNotYetMadeTakesNoCommands() throws {
        let state = try TestVaults.makeState()
        state.isLocked = false
        state.isFirstLaunch = true

        #expect(!state.acceptsCommands)
    }

    // MARK: - Search

    @Test func searchAsksForTheFieldAndTheList() throws {
        let state = try makeUnlockedState()
        let requests = state.listRevealRequests

        state.focusSearch()

        #expect(state.searchFocused)
        #expect(state.listRevealRequests == requests + 1)
    }

    @Test func searchDoesNothingBehindTheLock() throws {
        let state = try makeUnlockedState()
        state.lock()

        state.focusSearch()

        #expect(!state.searchFocused)
    }

    // MARK: - Sheets

    @Test func aSheetDoesNotOutliveTheLock() throws {
        let state = try makeUnlockedState()
        state.presentedSheet = .settings

        state.lock()

        #expect(state.presentedSheet == nil)
    }

    /// `.noteInfo` holds a decrypted note.
    @Test func aSheetDoesNotOutliveAWipe() throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "text"))
        state.presentedSheet = .noteInfo(note)

        state.wipeAllData(sharedInboxRoot: FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString))

        #expect(state.presentedSheet == nil)
    }

    @Test func sheetsHaveTheirOwnIdentity() throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "text"))
        let ids = [AppSheet.settings, .recycleBin, .noteInfo(note), .versionHistory(note)].map(\.id)
        #expect(Set(ids).count == 4)
    }

    // MARK: - Copying

    @Test func copyTakesTheEditStillWaitingToSave() throws {
        let state = try makeUnlockedState()
        let note = try #require(try state.createNote(content: "before"))
        var edited = note
        edited.content = "typed just now"
        state.pendingEditorNote = edited
        let pasteboard = UIPasteboard.withUniqueName()

        NoteActions(appState: state).copy(note, to: pasteboard)

        #expect(pasteboard.string == "typed just now")
    }
}
