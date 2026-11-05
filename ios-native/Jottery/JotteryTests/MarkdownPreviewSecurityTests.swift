import CryptoKit
import SwiftUI
import Testing
import UIKit
import WebKit

@testable import Jottery

/// The preview renders a note in a web view with a bridge to native code. A note
/// can come from the server or an imported file, so these run the real bundle:
/// nothing in a note may run, and nothing may leave the device on opening it.
@MainActor
@Suite(.serialized)
struct MarkdownPreviewSecurityTests {

    @MainActor
    private struct Hosted {
        let window: UIWindow
        let webView: WKWebView

        /// `expression` as a string, which is all that needs to cross back.
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

        func count(_ selector: String) async throws -> Int {
            Int(try await text("document.querySelectorAll(\(escapeForJS(selector))).length")) ?? -1
        }

        func flag(_ name: String) async throws -> Bool {
            try await text("window.\(name) === undefined") == "false"
        }

        /// Waits for `script` to come back true.
        func wait(until script: String) async throws {
            // An exception only means the page isn't there yet: it may be reloading.
            let succeeded = await waitUntil { (try? await text(script)) == "true" }
            if !succeeded { Issue.record("Timed out waiting for: \(script)") }
        }

        /// Gives anything a note scheduled the chance to run.
        func settle() async {
            for _ in 0..<10 {
                try? await Task.sleep(for: .milliseconds(30))
            }
        }
    }

    private func findWebView(in view: UIView) -> WKWebView? {
        if let webView = view as? WKWebView { return webView }
        return view.subviews.lazy.compactMap { findWebView(in: $0) }.first
    }

    private func host(
        _ content: String,
        attachments: [AttachmentRef] = [],
        attachmentRepo: AttachmentRepository? = nil,
        key: SymmetricKey? = nil
    ) async throws -> Hosted {
        let scene = try #require(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
        let window = UIWindow(windowScene: scene)
        window.rootViewController = UIHostingController(rootView: MarkdownPreviewView(
            content: content,
            attachments: attachments,
            attachmentRepo: attachmentRepo,
            encryptionKey: key
        ))
        window.makeKeyAndVisible()

        await waitUntil { findWebView(in: window) != nil }
        let hosted = Hosted(window: window, webView: try #require(findWebView(in: window)))
        try await hosted.wait(until: "document.getElementById('content') !== null && document.getElementById('content').childElementCount > 0")
        return hosted
    }

    @Test func ordinaryMarkdownStillRenders() async throws {
        let preview = try await host("""
        # Heading

        Some **bold** text and a [link](https://example.com).

        ```swift
        let a = 1
        ```

        | A | B |
        |---|---|
        | 1 | 2 |
        """)
        defer { preview.window.isHidden = true }

        #expect(try await preview.count("#content h1") == 1)
        #expect(try await preview.count("#content strong") == 1)
        #expect(try await preview.count("#content a[href='https://example.com']") == 1)
        #expect(try await preview.count("#content pre code") == 1)
        #expect(try await preview.count("#content table") == 1)
    }

    /// Sanitising removes form controls, which is what marked makes a task's box
    /// from. Whether a task is done still has to show.
    @Test func aTaskListStillShowsWhatsDone() async throws {
        let preview = try await host("""
        - [x] done
        - [ ] to do
        """)
        defer { preview.window.isHidden = true }

        let text = try await preview.text("document.getElementById('content').textContent")
        #expect(text.contains("\u{2611}"))
        #expect(text.contains("\u{2610}"))
        #expect(try await preview.count("#content input") == 0)
    }

    @Test func anEventHandlerInANoteDoesntRun() async throws {
        let preview = try await host("""
        before

        <img src=x onerror="window.pwned = 1">

        <svg onload="window.pwned = 2"></svg>

        <details open ontoggle="window.pwned = 3">x</details>
        """)
        defer { preview.window.isHidden = true }
        await preview.settle()

        #expect(try await preview.flag("pwned") == false)
        #expect(try await preview.count("#content [onerror], #content [onload], #content [ontoggle]") == 0)
    }

    @Test func aScriptInANoteDoesntRun() async throws {
        let preview = try await host("""
        before

        <script>window.pwned = 1</script>

        <iframe src="javascript:parent.pwned = 2"></iframe>

        <object data="https://example.com/x"></object>
        """)
        defer { preview.window.isHidden = true }
        await preview.settle()

        #expect(try await preview.flag("pwned") == false)
        #expect(try await preview.count("#content script, #content iframe, #content object") == 0)
    }

    @Test func aJavascriptLinkIsntALink() async throws {
        let preview = try await host("""
        [click](javascript:window.pwned=1) and <a href="javascript:window.pwned=2">this</a>
        """)
        defer { preview.window.isHidden = true }

        #expect(try await preview.count("#content a[href^='javascript']") == 0)
    }

    @Test func aMarkdownAttributeCantBreakOut() async throws {
        let preview = try await host(#"""
        before

        ![x" onerror="window.pwned=1](attachment:abc)

        [x](https://example.com "t\" onmouseover=\"window.pwned=2")
        """#)
        defer { preview.window.isHidden = true }
        await preview.settle()

        #expect(try await preview.flag("pwned") == false)
        #expect(try await preview.count("#content [onerror], #content [onmouseover]") == 0)
    }

    /// A remote image is a tracking beacon: opening the note would tell its host
    /// this device's address. In markdown it becomes a link to follow by choice.
    @Test func aRemoteImageIsNotLoaded() async throws {
        let preview = try await host("""
        before

        ![pixel](https://example.com/pixel.png)
        """)
        defer { preview.window.isHidden = true }

        #expect(try await preview.count("#content img") == 0)
        #expect(try await preview.count("#content a[href='https://example.com/pixel.png']") == 1)
    }

    /// Raw HTML keeps its tag, so the policy is what stops this one loading.
    @Test func aRawRemoteImageIsBlockedByThePolicy() async throws {
        let preview = try await host("""
        before

        <img src="https://example.com/raw.png">
        """)
        defer { preview.window.isHidden = true }

        let policy = try await preview.text(
            "document.querySelector('meta[http-equiv=\"Content-Security-Policy\"]')?.content ?? ''"
        )
        #expect(policy.contains("img-src data:;"))
    }

    @Test func thePageHasAContentSecurityPolicy() async throws {
        let preview = try await host("before")
        defer { preview.window.isHidden = true }

        let policy = try await preview.text(
            "document.querySelector('meta[http-equiv=\"Content-Security-Policy\"]')?.content ?? ''"
        )
        #expect(policy.contains("default-src 'none'"))
        #expect(policy.contains("script-src 'sha256-"))
        #expect(!policy.contains("unsafe-eval"))
    }

    /// The policy on its own, as if the sanitiser had let something through.
    @Test func thePolicyStopsWhatTheSanitiserMissed() async throws {
        let preview = try await host("before")
        defer { preview.window.isHidden = true }

        _ = try await preview.text("""
        (() => {
            document.getElementById('content').innerHTML = '<img src="data:," onerror="window.pwned = 1">';
            const script = document.createElement('script');
            script.textContent = 'window.pwned = 2';
            document.body.appendChild(script);
        })()
        """)
        await preview.settle()

        #expect(try await preview.flag("pwned") == false)
    }

    /// Hardening mustn't cost the feature: an attachment still loads as a data URL.
    @Test func anAttachmentImageStillResolves() async throws {
        let state = try TestVaults.makeState()
        try await state.createVault(password: "notes-password-correct-horse")
        let key = try #require(state.keyManager.masterKey)
        let note = try #require(try state.createNote(content: "note"))
        let png = try #require(Data(base64Encoded: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg=="))
        try state.addAttachment(to: note.id, data: png, filename: "dot.png", mimeType: "image/png")
        let stored = try #require(try state.noteRepo?.get(id: note.id, key: key))
        let attachment = try #require(stored.attachments.first)

        let preview = try await host(
            "before\n\n![dot](attachment:\(attachment.id))",
            attachments: stored.attachments,
            attachmentRepo: state.attachmentRepo,
            key: key
        )
        defer { preview.window.isHidden = true }

        try await preview.wait(until: "document.querySelector('#content img')?.src.startsWith('data:image/png') === true")
        try await preview.wait(until: "document.querySelector('#content img')?.naturalWidth === 1")
    }
}
