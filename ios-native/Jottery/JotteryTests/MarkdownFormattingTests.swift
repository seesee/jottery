import Foundation
import Testing
import UIKit

@testable import Jottery

/// A `|` marks the cursor, and a pair of them the selection.
private func marked(_ marked: String) -> (text: String, selection: NSRange) {
    let parts = marked.components(separatedBy: "|")
    let text = parts.joined()
    let start = parts[0].utf16.count
    let length = parts.count > 2 ? parts[1].utf16.count : 0
    return (text, NSRange(location: start, length: length))
}

private func applying(_ format: MarkdownFormat, to input: String) -> String {
    let (text, selection) = marked(input)
    let edit = format.edit(of: text, selection: selection)
    let result = (text as NSString).replacingCharacters(in: edit.range, with: edit.replacement) as NSString
    let start = edit.selection.location
    let end = edit.selection.upperBound
    if edit.selection.length == 0 {
        return result.substring(to: start) + "|" + result.substring(from: start)
    }
    return result.substring(to: start) + "|" + result.substring(with: edit.selection) + "|" + result.substring(from: end)
}

private let boldCases = [
    ("|word|", "**|word|**"),
    ("a |word| b", "a **|word|** b"),
    ("|", "**|**"),
    ("**|word|**", "|word|"),
    ("|**word**|", "|word|"),
    ("**|**", "|"),
    ("*|word|*", "***|word|***"),
    ("***|word|***", "*|word|*"),
]

private let italicCases = [
    ("|word|", "*|word|*"),
    ("|", "*|*"),
    ("*|word|*", "|word|"),
    ("|*word*|", "|word|"),
    ("**|word|**", "***|word|***"),
    ("***|word|***", "**|word|**"),
]

private let listCases = [
    ("wo|rd", "- wo|rd"),
    ("|", "- |"),
    ("- wo|rd", "wo|rd"),
    ("* wo|rd", "wo|rd"),
    ("  in|dented", "  - in|dented"),
    ("  - in|dented", "  in|dented"),
    ("first\nse|cond\nthird", "first\n- se|cond\nthird"),
    ("|one\ntwo|", "|- one\n- two|"),
    ("|- one\n- two|", "|one\ntwo|"),
    ("|one\n- two|", "|- one\n- two|"),
    ("|one\n\ntwo|", "|- one\n\n- two|"),
    ("|one\n|two", "|- one|\ntwo"),
    ("-not a bullet|", "- -not a bullet|"),
]

private let linkCases = [
    ("|words|", "[words](https://|)"),
    ("|", "[|](https://)"),
    ("see |https://example.com/a| now", "see [|](https://example.com/a) now"),
    ("|http://example.com|", "[|](http://example.com)"),
    ("|https://a b|", "[https://a b](https://|)"),
]

/// What the bar does to a note's text (R2-145).
@MainActor
struct MarkdownFormattingTests {

    @Test(arguments: boldCases)
    func bold(input: String, expected: String) {
        #expect(applying(.bold, to: input) == expected)
    }

    @Test(arguments: italicCases)
    func italic(input: String, expected: String) {
        #expect(applying(.italic, to: input) == expected)
    }

    @Test(arguments: listCases)
    func list(input: String, expected: String) {
        #expect(applying(.bulletList, to: input) == expected)
    }

    @Test(arguments: linkCases)
    func link(input: String, expected: String) {
        #expect(applying(.link, to: input) == expected)
    }

    @Test func offsetsAreInUTF16() {
        #expect(applying(.bold, to: "😀 |word|") == "😀 **|word|**")
    }

    @Test func aWindowsLineEndingSurvives() {
        #expect(applying(.bulletList, to: "|one\r\ntwo|") == "|- one\r\n- two|")
    }

    // MARK: - In the Editor

    @Test func theBarWrapsTheSelection() {
        let editor = HeadlessEditor(text: "a word")
        editor.textView.selectedRange = NSRange(location: 2, length: 4)

        editor.textView.apply(.bold)

        #expect(editor.textView.text == "a **word**")
        #expect(editor.bound == "a **word**")
        #expect(editor.textView.selectedRange == NSRange(location: 4, length: 4))
    }

    @Test(arguments: MarkdownFormat.allCases)
    func undoPutsItBack(format: MarkdownFormat) throws {
        let editor = HeadlessEditor(text: "a word")
        editor.textView.selectedRange = NSRange(location: 2, length: 4)

        editor.textView.apply(format)
        #expect(editor.textView.text != "a word")
        try #require(editor.textView.undoManager).undo()

        #expect(editor.textView.text == "a word")
        #expect(editor.bound == "a word")
        #expect(editor.textView.selectedRange == NSRange(location: 2, length: 4))
    }

    @Test func anInsertIsItsOwnUndoStep() throws {
        let editor = HeadlessEditor(text: "")
        editor.textView.insertText("typed ")

        editor.textView.insert(try #require(QuickCommands.all.last))
        #expect(editor.textView.text == "typed ---")
        try #require(editor.textView.undoManager).undo()

        #expect(editor.textView.text == "typed ")
    }

    @Test func theInsertReplacesTheSelection() throws {
        let editor = HeadlessEditor(text: "old")
        editor.textView.selectedRange = NSRange(location: 0, length: 3)

        editor.textView.insert(try #require(QuickCommands.all.last))

        #expect(editor.textView.text == "---")
    }

    @Test func aReadOnlyNoteCantBeFormatted() throws {
        let editor = HeadlessEditor(text: "locked")
        editor.textView.isEditable = false
        editor.textView.selectedRange = NSRange(location: 0, length: 6)

        editor.textView.apply(.bold)
        editor.textView.insert(try #require(QuickCommands.all.last))

        #expect(editor.textView.text == "locked")
    }

    // MARK: - The Bar

    @Test func onAPhoneItsAToolbarAboveTheKeyboard() throws {
        let textView = EditorTextView()
        let systemGroups = textView.inputAssistantItem.trailingBarButtonGroups

        EditorFormattingBar.install(on: textView, showsFormatting: true, idiom: .phone)

        let items = try #require(textView.formattingToolbar?.items)
        // Four formats, the space that pushes the rest over, and the insert menu.
        #expect(items.count == 6)
        #expect(textView.formattingGroup == nil)
        #expect(textView.inputAssistantItem.trailingBarButtonGroups == systemGroups)
    }

    @Test func onAnIPadItJoinsTheSystemsBarAndAddsNoSecondOne() throws {
        let textView = EditorTextView()
        let systemGroups = textView.inputAssistantItem.trailingBarButtonGroups

        EditorFormattingBar.install(on: textView, showsFormatting: true, idiom: .pad)

        #expect(textView.formattingToolbar == nil)
        let group = try #require(textView.formattingGroup)
        #expect(group.barButtonItems.count == 5)
        // The system's own buttons are still there, with ours after them.
        #expect(textView.inputAssistantItem.trailingBarButtonGroups == systemGroups + [group])
    }

    @Test func onAnIPadChangingLanguageReplacesOnlyOurButtons() throws {
        let textView = EditorTextView()
        let systemGroups = textView.inputAssistantItem.trailingBarButtonGroups

        EditorFormattingBar.install(on: textView, showsFormatting: true, idiom: .pad)
        EditorFormattingBar.install(on: textView, showsFormatting: false, idiom: .pad)

        let group = try #require(textView.formattingGroup)
        #expect(group.barButtonItems.count == 1)
        #expect(textView.inputAssistantItem.trailingBarButtonGroups == systemGroups + [group])
    }

    @Test func outsideMarkdownOnlyTheInsertMenuIsOffered() throws {
        let textView = EditorTextView()

        EditorFormattingBar.install(on: textView, showsFormatting: false, idiom: .phone)

        let items = try #require(textView.formattingToolbar?.items)
        #expect(items.count == 2)
        #expect(items.last?.menu != nil)
    }

    @Test func changingLanguageChangesTheBar() throws {
        let textView = EditorTextView()

        EditorFormattingBar.install(on: textView, showsFormatting: true, idiom: .phone)
        let toolbar = try #require(textView.formattingToolbar)
        EditorFormattingBar.install(on: textView, showsFormatting: false, idiom: .phone)

        #expect(textView.formattingToolbar === toolbar)
        #expect(toolbar.items?.count == 2)
    }

    @Test func everyButtonHasASymbolAndAVoiceOverLabel() throws {
        let textView = EditorTextView()
        EditorFormattingBar.install(on: textView, showsFormatting: true, idiom: .pad)

        let group = try #require(textView.formattingGroup)
        for item in group.barButtonItems {
            #expect(item.image != nil)
            #expect(item.accessibilityLabel?.isEmpty == false)
        }
    }

    @Test func theInsertMenuOffersEveryCommand() throws {
        let textView = EditorTextView()
        EditorFormattingBar.install(on: textView, showsFormatting: false, idiom: .phone)

        let menu = try #require(textView.formattingToolbar?.items?.last?.menu)
        #expect(menu.children.count == QuickCommands.all.count)
        for symbol in QuickCommands.all.map(\.symbolName) {
            #expect(UIImage(systemName: symbol) != nil)
        }
    }

    @Test func everySymbolExists() {
        for format in MarkdownFormat.allCases {
            #expect(UIImage(systemName: format.symbolName) != nil)
        }
    }
}
