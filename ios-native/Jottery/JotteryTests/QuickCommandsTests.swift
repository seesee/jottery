import Foundation
import SwiftUI
import Testing
import UIKit

@testable import Jottery

// 2026-03-07 04:05:09 UTC.
private let stamp = Date(timeIntervalSince1970: 1_772_856_309)
private let utc = TimeZone(identifier: "UTC")!

private let triggerCases: [(String, String?)] = [
    ("/now", "/now"),
    ("hello /now", "/now"),
    ("line\n/date", "/date"),
    ("\t/time", "/time"),
    ("/uuid", "/uuid"),
    ("/hr", "/hr"),
    ("/NOW", "/now"),
    ("date/time", nil),
    ("/nope", nil),
    ("/now2", nil),
    ("/", nil),
    ("", nil),
    ("/now ", nil),
    ("http://example.com/now", nil),
]

/// `/now` and the rest, in the web's formats (R2-145).
@MainActor
struct QuickCommandsTests {

    private func command(_ trigger: String) throws -> QuickCommand {
        try #require(QuickCommands.all.first { $0.trigger == trigger })
    }

    @Test func theWebsDefaultsAreAllHere() {
        #expect(QuickCommands.all.map(\.trigger) == ["/now", "/date", "/time", "/uuid", "/hr"])
    }

    @Test func theFormatsMatchTheWebs() throws {
        #expect(try command("/now").expansion(now: stamp, timeZone: utc) == "2026-03-07 04:05:09")
        #expect(try command("/date").expansion(now: stamp, timeZone: utc) == "2026-03-07")
        #expect(try command("/time").expansion(now: stamp, timeZone: utc) == "04:05:09")
        #expect(try command("/hr").expansion(now: stamp, timeZone: utc) == "---")
    }

    @Test func itsTheLocalTime() throws {
        let tokyo = try #require(TimeZone(identifier: "Asia/Tokyo"))
        #expect(try command("/now").expansion(now: stamp, timeZone: tokyo) == "2026-03-07 13:05:09")
    }

    @Test func aUUIDIsLowerCaseAndNewEachTime() throws {
        let uuid = try command("/uuid")
        let first = uuid.expansion()
        #expect(first == first.lowercased())
        #expect(UUID(uuidString: first) != nil)
        #expect(first != uuid.expansion())
    }

    @Test(arguments: triggerCases)
    func triggers(text: String, expected: String?) {
        #expect(QuickCommands.match(before: text)?.trigger == expected)
    }

    @Test func aSpaceOrANewLineFinishesOne() {
        #expect(QuickCommands.endsCommand(" "))
        #expect(QuickCommands.endsCommand("\n"))
        #expect(QuickCommands.endsCommand("\r\n"))
        #expect(!QuickCommands.endsCommand("a"))
        #expect(!QuickCommands.endsCommand("  "))
    }

    // MARK: - In the Editor

    private func makeEditor(_ text: String) -> HeadlessEditor {
        HeadlessEditor(text: text)
    }

    @Test func typingASpaceAfterACommandExpandsIt() {
        let editor = makeEditor("stamp /hr")
        editor.textView.insertText(" ")
        #expect(editor.textView.text == "stamp --- ")
        #expect(editor.bound == "stamp --- ")
        #expect(editor.textView.selectedRange == NSRange(location: 10, length: 0))
    }

    @Test func aNewLineExpandsItToo() {
        let editor = makeEditor("/hr")
        editor.textView.insertText("\n")
        #expect(editor.textView.text == "---\n")
    }

    @Test func theDateIsWrittenInFull() throws {
        let editor = makeEditor("/date")
        editor.textView.insertText(" ")
        #expect(editor.textView.text.wholeMatch(of: #/\d{4}-\d{2}-\d{2} /#) != nil)
    }

    @Test func undoBringsTheCommandBack() throws {
        let editor = makeEditor("/hr")
        editor.textView.insertText(" ")
        #expect(editor.textView.text == "--- ")

        try #require(editor.textView.undoManager).undo()

        #expect(editor.textView.text == "/hr")
        #expect(editor.bound == "/hr")
    }

    @Test func midWordItStaysAsTyped() {
        let editor = makeEditor("either/hr")
        editor.textView.insertText(" ")
        #expect(editor.textView.text == "either/hr ")
    }

    @Test func anOrdinarySpaceIsJustASpace() {
        let editor = makeEditor("hello")
        editor.textView.insertText(" ")
        #expect(editor.textView.text == "hello ")
    }

    @Test func aReadOnlyNoteIsLeftAlone() {
        let editor = makeEditor("/hr")
        editor.textView.isEditable = false
        let allowed = editor.coordinator.textView(
            editor.textView,
            shouldChangeTextIn: NSRange(location: 3, length: 0),
            replacementText: " "
        )
        #expect(allowed)
        #expect(editor.textView.text == "/hr")
    }

    @Test func replacingASelectionIsNotACommand() {
        let editor = makeEditor("/hr word")
        editor.textView.selectedRange = NSRange(location: 3, length: 5)
        editor.textView.insertText(" ")
        #expect(editor.textView.text == "/hr ")
    }
}

/// A text view wired up as `RunestoneEditorView` does it, without a window.
@MainActor
final class HeadlessEditor {
    let textView = EditorTextView()
    let coordinator: RunestoneEditorView.Coordinator
    private(set) var bound: String

    init(text: String) {
        bound = text
        var write: (String) -> Void = { _ in }
        let binding = Binding<String>(get: { text }, set: { write($0) })
        coordinator = RunestoneEditorView.Coordinator(text: binding)
        textView.editorDelegate = coordinator
        textView.text = text
        textView.selectedRange = NSRange(location: text.utf16.count, length: 0)
        write = { [weak self] in self?.bound = $0 }
    }
}
