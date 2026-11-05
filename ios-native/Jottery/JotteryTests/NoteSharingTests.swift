import Foundation
import Testing
import WebKit

@testable import Jottery

@MainActor
struct NoteSharingTests {

    private func note(content: String, language: String = "markdown") throws -> DecryptedNote {
        let state = try TestVaults.makeState()
        state.keyManager.unlockWithKeyData(Data(repeating: 7, count: 32))
        var note = try #require(try state.createNote(content: content))
        note.syntaxLanguage = language
        return note
    }

    @Test func aCopiedLinkIsTheOneTheWebMakes() {
        #expect(NoteSharing.link(to: "abc") == "[](link:abc)")
    }

    @Test func aFileIsNamedFromTheFirstLineAndTheLanguage() throws {
        #expect(NoteSharing.filename(for: try note(content: "# My Notes\nmore")) == "My_Notes.md")
        #expect(NoteSharing.filename(for: try note(content: "print('hi')", language: "python")) == "print_hi.py")
        #expect(NoteSharing.filename(for: try note(content: "text", language: "something-else")) == "text.txt")
    }

    @Test func aNoteWithNoFirstLineIsNamedByDate() throws {
        let blank = try note(content: "\n\nlater")

        #expect(NoteSharing.filename(for: blank) == "\(blank.createdAt.iso8601.prefix(10)).md")
    }

    @Test func aFirstLineCantReachOutsideTheFolder() throws {
        let name = NoteSharing.filename(for: try note(content: "../../etc/passwd", language: "plain"))

        #expect(!name.contains("/"))
    }

    @Test func sharingWritesTheTextAndOffersItToo() throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-share-tests-\(UUID().uuidString)", isDirectory: true)
        let files = TemporaryFiles(root: root)
        defer { files.removeAll() }
        let shared = try note(content: "# Title\nbody")

        let file = try NoteSharing.shareFile(for: shared, files: files)

        #expect(file.url.lastPathComponent == "Title.md")
        #expect(try String(contentsOf: file.url, encoding: .utf8) == "# Title\nbody")
        #expect(file.text == "# Title\nbody")
    }

    @Test func onlyMarkdownPrintsRendered() throws {
        #expect(NotePrinter.printsRendered(try note(content: "# Hi")))
        #expect(!NotePrinter.printsRendered(try note(content: "1 + 1", language: "calc")))
    }

    @Test func markdownIsRenderedBeforeItsPrinted() async throws {
        let rendered = try note(content: "# Heading\n\nSome **bold** text")

        let (printer, webView) = await NotePrinter.render(rendered)
        _ = printer

        let page = try #require(webView)
        let html = try await page.evaluateJavaScript("document.querySelector('#content h1').textContent") as? String
        #expect(html == "Heading")
    }

    @Test func plainTextNeedsNoPage() async throws {
        let (_, webView) = await NotePrinter.render(try note(content: "1 + 1", language: "calc"))

        #expect(webView == nil)
    }
}
