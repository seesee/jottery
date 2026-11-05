import CryptoKit
import ImageIO
import SwiftUI
import Testing
import UIKit
import WebKit

@testable import Jottery

/// Runs the real bundles, as `MarkdownPreviewSecurityTests` does.
@MainActor
@Suite(.serialized)
struct PreviewFeatureTests {

    private let noteId = "11111111-2222-4333-8444-555555555555"

    @MainActor
    private struct Hosted {
        let window: UIWindow
        let webView: WKWebView

        func text(_ expression: String) async throws -> String {
            try await withCheckedThrowingContinuation { continuation in
                webView.evaluateJavaScript("String(\(expression))") { result, error in
                    if let error {
                        continuation.resume(throwing: error)
                    } else {
                        continuation.resume(returning: result as? String ?? "")
                    }
                }
            }
        }

        func wait(until script: String) async throws {
            // An exception only means the page isn't there yet: it may be reloading.
            let succeeded = await waitUntil { (try? await text(script)) == "true" }
            if !succeeded { Issue.record("Timed out waiting for: \(script)") }
        }
    }

    private func findWebView(in view: UIView) -> WKWebView? {
        if let webView = view as? WKWebView { return webView }
        return view.subviews.lazy.compactMap { findWebView(in: $0) }.first
    }

    private func host(_ view: some View, until script: String) async throws -> Hosted {
        let scene = try #require(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
        let window = UIWindow(windowScene: scene)
        window.rootViewController = UIHostingController(rootView: view)
        window.makeKeyAndVisible()
        await waitUntil { findWebView(in: window) != nil }
        let hosted = Hosted(window: window, webView: try #require(findWebView(in: window)))
        try await hosted.wait(until: script)
        return hosted
    }

    private func preview(_ content: String, onOpenNote: ((String) -> Void)? = nil) async throws -> Hosted {
        try await host(
            MarkdownPreviewView(
                content: content,
                noteTitles: [noteId: "A Note [With] Brackets"],
                onOpenNote: onOpenNote
            ),
            until: "document.getElementById('content') !== null && document.getElementById('content').childElementCount > 0"
        )
    }

    // MARK: - Note links (R2-136)

    @Test func aTitleInDoubleBracketsBecomesALink() async throws {
        let titled = "22222222-3333-4444-8555-666666666666"
        let page = try await host(
            MarkdownPreviewView(content: "See [[shopping list]] for more.", noteTitles: [titled: "Shopping List"]),
            until: "document.querySelector('.note-link') !== null"
        )
        defer { page.window.isHidden = true }

        #expect(try await page.text("document.querySelector('.note-link').dataset.noteId") == titled)
        #expect(try await page.text("document.querySelector('.note-link').textContent") == "Shopping List")
    }

    @Test func aLinkFromTheWebsCopyLinkShowsTheNotesTitle() async throws {
        let page = try await preview("Go to [](link:\(noteId)).")
        defer { page.window.isHidden = true }

        let link = "document.querySelector('.note-link[data-note-id]')"
        #expect(try await page.text("\(link).textContent") == "A Note [With] Brackets")
    }

    @Test func aLinkByIdAndOneWithItsOwnTextBothWork() async throws {
        let page = try await preview("[[\(noteId)]] and [[\(noteId)|over here]]")
        defer { page.window.isHidden = true }

        #expect(try await page.text("document.querySelectorAll('.note-link[data-note-id]').length") == "2")
        #expect(try await page.text("document.querySelectorAll('.note-link[data-note-id]')[1].textContent") == "over here")
    }

    @Test func tappingALinkOpensTheNote() async throws {
        var opened: [String] = []
        let page = try await preview("[](link:\(noteId))", onOpenNote: { opened.append($0) })
        defer { page.window.isHidden = true }

        _ = try await page.text("document.querySelector('.note-link[data-note-id]').click()")
        await waitUntil { !opened.isEmpty }

        #expect(opened == [noteId])
    }

    /// The bridge is reachable from anything that ever runs in the page.
    @Test func anIdThatIsntOneGoesNowhere() async throws {
        var opened: [String] = []
        let page = try await preview("text", onOpenNote: { opened.append($0) })
        defer { page.window.isHidden = true }

        let post = "window.webkit.messageHandlers.bridge.postMessage"
        _ = try await page.text("\(post)({type: 'openNote', id: '../../etc/passwd'})")
        _ = try await page.text("\(post)({type: 'openNote', id: '\(noteId)'})")
        await waitUntil { !opened.isEmpty }

        #expect(opened == [noteId])
    }

    @Test func aNoteThatDoesntExistIsShownAsMissing() async throws {
        let page = try await preview("[[No Such Note]]")
        defer { page.window.isHidden = true }

        #expect(try await page.text("document.querySelector('.note-link.missing').textContent") == "No Such Note")
        #expect(try await page.text("document.querySelectorAll('.note-link[data-note-id]').length") == "0")
    }

    @Test func aLinkInCodeStaysAsWritten() async throws {
        let page = try await preview("`[[\(noteId)]]`")
        defer { page.window.isHidden = true }

        #expect(try await page.text("document.querySelectorAll('.note-link').length") == "0")
        #expect(try await page.text("document.querySelector('code').textContent") == "[[\(noteId)]]")
    }

    @Test func aMissingNoteIsReported() throws {
        let state = try TestVaults.makeState()
        state.keyManager.unlockWithKeyData(Data(repeating: 7, count: 32))

        state.openLinkedNote(id: noteId)

        #expect(state.userErrorMessage == L.previewNoteNotFound)
        #expect(state.selectedNoteId == nil)
    }

    @Test func aNoteFoundInTheArchiveIsOpened() throws {
        let state = try TestVaults.makeState()
        state.keyManager.unlockWithKeyData(Data(repeating: 7, count: 32))
        let note = try #require(try state.createNote(content: "stored"))
        try state.archiveNote(id: note.id)

        state.openLinkedNote(id: note.id)

        #expect(state.selectedNoteId == note.id)
    }

    @Test func titlesAreOnlyHandedOverForContentWithLinks() throws {
        let state = try TestVaults.makeState()
        state.keyManager.unlockWithKeyData(Data(repeating: 7, count: 32))
        let note = try #require(try state.createNote(content: "My Title"))

        #expect(state.noteTitles(linkedFrom: "no links here").isEmpty)
        #expect(state.noteTitles(linkedFrom: "see [[My Title]]") == [note.id: "My Title"])
        #expect(state.noteTitles(linkedFrom: "[](link:\(note.id))") == [note.id: "My Title"])
    }

    // MARK: - Images (jottery-c6jk)

    private func ref(_ id: String) -> AttachmentRef {
        AttachmentRef(id: id, filename: "a", mimeType: "image/png", size: 1, data: id)
    }

    /// Any prefix used to match, down to one letter, so a note could pull in
    /// whichever attachment came first.
    @Test func anAttachmentHasToBeNamedInFull() {
        let attachment = ref(noteId)

        #expect(PreviewAttachments.find(noteId, in: [attachment])?.id == noteId)
        #expect(PreviewAttachments.find(noteId.replacingOccurrences(of: "-", with: ""), in: [attachment])?.id == noteId)
        #expect(PreviewAttachments.find("1111", in: [attachment]) == nil)
        #expect(PreviewAttachments.find("", in: [attachment]) == nil)
    }

    private func noisyImage(side: Int) throws -> Data {
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        format.opaque = true
        let image = UIGraphicsImageRenderer(size: CGSize(width: side, height: side), format: format).image { context in
            for x in stride(from: 0, to: side, by: 8) {
                for y in stride(from: 0, to: side, by: 8) {
                    UIColor(hue: .random(in: 0...1), saturation: 1, brightness: 1, alpha: 1).setFill()
                    context.fill(CGRect(x: x, y: y, width: 8, height: 8))
                }
            }
        }
        return try #require(image.pngData())
    }

    @Test func aLargeImageIsScaledDownForThePreview() throws {
        let data = try noisyImage(side: 3000)
        #expect(data.count > PreviewAttachments.scaleAboveBytes)

        let url = PreviewAttachments.dataURL(for: data, mimeType: "image/png")

        let encoded = try #require(url.components(separatedBy: "base64,").last)
        let bytes = try #require(Data(base64Encoded: encoded))
        let scaled = try #require(UIImage(data: bytes))
        #expect(max(scaled.size.width, scaled.size.height) * scaled.scale <= CGFloat(PreviewAttachments.maxImagePixels))
    }

    @Test func aSmallImageGoesThroughAsItIs() throws {
        let data = try noisyImage(side: 16)

        #expect(PreviewAttachments.dataURL(for: data, mimeType: "image/png")
            == "data:image/png;base64,\(data.base64EncodedString())")
    }

    @Test func aHostileMimeTypeDoesntReachTheDataURL() {
        let url = PreviewAttachments.dataURL(for: Data([1]), mimeType: "image/png;\"><script>")

        #expect(url.hasPrefix("data:application/octet-stream;base64,"))
    }

    @Test func thePreviewComesBackAfterItsProcessDies() async throws {
        let page = try await preview("# Still here")
        defer { page.window.isHidden = true }

        page.webView.navigationDelegate?.webViewWebContentProcessDidTerminate?(page.webView)

        try await page.wait(until: "document.getElementById('content') !== null && document.querySelector('#content h1') !== null")
        #expect(try await page.text("document.querySelector('#content h1').textContent") == "Still here")
    }

    // MARK: - Locked calc and outliner notes (jottery-dnxs)

    private func editor(_ bundle: String, content: Binding<String>, readOnly: Bool, until script: String) async throws -> Hosted {
        try await host(
            WebEditorView(bundleName: bundle, content: content, isReadOnly: readOnly),
            until: script
        )
    }

    private func constant(_ text: String, onChange: @escaping (String) -> Void = { _ in }) -> Binding<String> {
        Binding(get: { text }, set: onChange)
    }

    @Test func aLockedCalcNoteCantBeTypedIn() async throws {
        let page = try await editor("calc", content: constant("1 + 1"), readOnly: true, until: "document.querySelector('.cm-content') !== null")
        defer { page.window.isHidden = true }

        try await page.wait(until: "document.querySelector('.cm-content').contentEditable === 'false'")
    }

    @Test func anUnlockedCalcNoteCanBe() async throws {
        let page = try await editor("calc", content: constant("1 + 1"), readOnly: false, until: "document.querySelector('.cm-content') !== null")
        defer { page.window.isHidden = true }

        try await page.wait(until: "document.querySelector('.cm-content').contentEditable === 'true'")
    }

    @Test func aLockedOutlinerNoteCantBeTypedInOrDragged() async throws {
        let page = try await editor("outliner", content: constant("- one"), readOnly: true, until: "document.querySelector('.outliner-content') !== null")
        defer { page.window.isHidden = true }

        #expect(try await page.text("document.querySelector('.outliner-content').contentEditable") == "false")
        #expect(try await page.text("document.querySelector('.outliner-row').draggable") == "false")
    }

    @Test func anUnlockedOutlinerNoteCanBe() async throws {
        let page = try await editor("outliner", content: constant("- one"), readOnly: false, until: "document.querySelector('.outliner-content') !== null")
        defer { page.window.isHidden = true }

        #expect(try await page.text("document.querySelector('.outliner-content').contentEditable") == "true")
    }

    /// Text typed into a locked note was dropped without a word. Anything that
    /// still reaches the native side mustn't change the note.
    @Test func aChangeFromALockedEditorIsIgnored() async throws {
        var changes: [String] = []
        let page = try await editor(
            "calc", content: constant("1 + 1", onChange: { changes.append($0) }), readOnly: true,
            until: "document.querySelector('.cm-content') !== null"
        )
        defer { page.window.isHidden = true }

        _ = try await page.text("window.webkit.messageHandlers.bridge.postMessage({type: 'contentChanged', content: 'typed'})")
        // A round trip through the page, which comes after the message.
        _ = try await page.text("1")

        #expect(changes.isEmpty)
    }
}
