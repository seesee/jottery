import CryptoKit
import Foundation
import SwiftUI
import Testing
import UIKit

@testable import Jottery

/// The editor keeps its own copy of a note. These run the real view, hosted in a
/// window, to check that copy can't overwrite something newer.
@Observable
final class EditorPresence {
    var isShown = true
    /// Set once SwiftUI has taken the editor away, which is what saves it.
    var didDisappear = false
}

/// Lets SwiftUI catch up. Several short sleeps rather than one long one: each
/// needs the main actor back, so a stalled actor can't use up the whole wait
/// before the run loop has turned.
private func settle() async {
    for _ in 0..<6 {
        try? await Task.sleep(for: .milliseconds(50))
    }
}

@MainActor
struct NoteEditorStaleStateTests {

    private struct Host: View {
        let appState: AppState
        let noteId: String
        let presence: EditorPresence

        var body: some View {
            Group {
                if presence.isShown, let note = appState.displayedNote(id: noteId) {
                    NoteEditorView(note: note)
                        .onDisappear { presence.didDisappear = true }
                }
            }
            .environment(appState)
        }
    }

    @MainActor
    private struct OpenEditor {
        let window: UIWindow
        let presence: EditorPresence

        /// Types into the real text view, as the keyboard would.
        func typeText(_ text: String) async throws {
            // Layout can take a moment while other tests share the main thread.
            await waitUntil { firstTextInput(in: window) != nil }
            let textView = try #require(firstTextInput(in: window))
            textView.becomeFirstResponder()
            await waitUntil { textView.isFirstResponder }
            let before = shownText
            (textView as? UIKeyInput)?.insertText(text)
            await waitUntil { shownText != before }
            await settle()
        }

        /// What the text view holds.
        var shownText: String? {
            guard let input = firstTextInput(in: window) as? UITextInput,
                  let all = input.textRange(from: input.beginningOfDocument, to: input.endOfDocument) else {
                return nil
            }
            return input.text(in: all)
        }

        /// Waits for a change from outside to reach the text view.
        func waitToShow(_ text: String) async {
            await waitUntil { shownText == text }
        }

        private func firstTextInput(in view: UIView) -> UIView? {
            if view is UITextInput, !(view is UITextField) { return view }
            return view.subviews.lazy.compactMap { firstTextInput(in: $0) }.first
        }

        /// Leaving the note is what saves anything the editor thinks has changed.
        func leave() async {
            presence.isShown = false
            await waitUntil { presence.didDisappear }
            window.isHidden = true
        }
    }

    private func open(_ noteId: String, in state: AppState) async -> OpenEditor {
        let presence = EditorPresence()
        // Attached to the app's scene: without one, nothing lays out.
        let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: UIScreen.main.bounds)
        window.rootViewController = UIHostingController(
            rootView: Host(appState: state, noteId: noteId, presence: presence)
        )
        window.makeKeyAndVisible()
        let editor = OpenEditor(window: window, presence: presence)
        await waitUntil { editor.shownText != nil }
        return editor
    }

    private func makeVault() async throws -> AppState {
        let state = try TestVaults.makeState()
        try await state.createVault(password: "correct-horse-battery-staple")
        return state
    }

    /// A note showing its text editor rather than the preview.
    private func editableNote(in state: AppState) throws -> DecryptedNote {
        var note = try #require(try state.createNote(content: "start"))
        note.showPreview = false
        try state.saveNote(note)
        return try #require(state.displayedNote(id: note.id))
    }

    private func storedContent(of noteId: String, in state: AppState) throws -> String? {
        try #require(state.noteRepo).get(id: noteId, key: #require(state.keyManager.masterKey))?.content
    }

    @Test func aRestoredVersionSurvivesLeavingTheNote() async throws {
        let state = try await makeVault()
        var note = try #require(try state.createNote(content: "the earlier text"))
        let noteRepo = try #require(state.noteRepo)
        let versionRepo = try #require(state.versionRepo)
        try versionRepo.createVersion(from: #require(try noteRepo.getRaw(id: note.id)), reason: "sync")
        note.content = "the later text"
        try state.saveNote(note)
        let version = try #require(try versionRepo.getVersions(noteId: note.id).first)

        let editor = await open(note.id, in: state)
        try state.restoreVersion(noteId: note.id, version: version)
        await editor.waitToShow("the earlier text")
        await editor.leave()

        #expect(try storedContent(of: note.id, in: state) == "the earlier text")
    }

    @Test func typedTextIsSavedOnLeaving() async throws {
        let state = try await makeVault()
        let note = try editableNote(in: state)

        let editor = await open(note.id, in: state)
        try await editor.typeText("typed ")
        await editor.leave()

        let stored = try #require(try storedContent(of: note.id, in: state))
        #expect(stored.contains("typed"))
        #expect(stored.contains("start"))
    }

    @Test func typedTextWinsOverAnArrivingChangeWhichGoesToHistory() async throws {
        let state = try await makeVault()
        var note = try editableNote(in: state)
        let versionRepo = try #require(state.versionRepo)

        let editor = await open(note.id, in: state)
        try await editor.typeText("typed ")
        note.content = "arrived from elsewhere"
        try state.saveNote(note)
        // The editor keeps the typing, and puts what arrived in history.
        try await waitUntil {
            try versionRepo.getVersions(noteId: note.id).contains { $0.reason == "pre-overwrite" }
        }
        await editor.leave()

        let stored = try #require(try storedContent(of: note.id, in: state))
        #expect(stored.contains("typed"))
        let versions = try versionRepo.getVersions(noteId: note.id)
        #expect(versions.contains { $0.reason == "pre-overwrite" })
    }

    @Test func aPinMadeAfterTheEditorLoadedIsKeptWhenItSaves() async throws {
        let state = try await makeVault()
        var note = try #require(try state.createNote(content: "first"))
        let opened = NoteEditorFields(note: note)

        try state.togglePin(id: note.id)
        var edited = opened
        edited.content = "second"
        try state.saveEditorFields(edited)

        note = try #require(state.displayedNote(id: note.id))
        #expect(note.content == "second")
        #expect(note.pinned)
    }

    @Test func keepingTheCurrentVersionAddsItToHistory() async throws {
        let state = try await makeVault()
        let note = try #require(try state.createNote(content: "arrived from elsewhere"))
        let versionRepo = try #require(state.versionRepo)
        let before = try versionRepo.getVersions(noteId: note.id).count

        try state.keepCurrentVersion(ofNote: note.id)

        #expect(try versionRepo.getVersions(noteId: note.id).count == before + 1)
    }

    @Test func reconcilingWithTheEditorsOwnSaveDoesNothing() throws {
        var note = NoteEditorFields(note: try sampleNote())
        note.content = "saved"
        #expect(NoteEditorFields.reconcile(editor: note, baseline: note, incoming: note) == .nothing)
    }

    @Test func reconcilingWithNoEditsAdoptsWhatArrived() throws {
        let loaded = NoteEditorFields(note: try sampleNote())
        var incoming = loaded
        incoming.content = "from elsewhere"
        #expect(NoteEditorFields.reconcile(editor: loaded, baseline: loaded, incoming: incoming) == .adopt)
    }

    @Test func reconcilingWithEditsKeepsThem() throws {
        let loaded = NoteEditorFields(note: try sampleNote())
        var editor = loaded
        editor.content = "being typed"
        var incoming = loaded
        incoming.content = "from elsewhere"
        #expect(NoteEditorFields.reconcile(editor: editor, baseline: loaded, incoming: incoming) == .keepEdits)
    }

    @Test func reconcilingWhenTheSameTextArrivedDoesNothing() throws {
        let loaded = NoteEditorFields(note: try sampleNote())
        var editor = loaded
        editor.content = "same on both"
        #expect(NoteEditorFields.reconcile(editor: editor, baseline: loaded, incoming: editor) == .nothing)
    }

    private func sampleNote() throws -> DecryptedNote {
        let state = try TestVaults.makeState()
        state.keyManager.unlockWithKeyData(Data(repeating: 7, count: 32))
        return try #require(try state.createNote(content: "loaded"))
    }

    @Test func aChangeFromElsewhereSurvivesLeavingTheNote() async throws {
        let state = try await makeVault()
        var note = try #require(try state.createNote(content: "what the editor loaded"))

        let editor = await open(note.id, in: state)
        note.content = "edited on another device"
        try state.saveNote(note)
        await editor.waitToShow("edited on another device")
        await editor.leave()

        #expect(try storedContent(of: note.id, in: state) == "edited on another device")
    }
}
