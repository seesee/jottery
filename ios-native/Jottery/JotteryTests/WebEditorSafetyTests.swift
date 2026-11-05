import SwiftUI
import Testing
import UIKit
import WebKit

@testable import Jottery

/// Notes arrive through sync, so a calc note can ask for anything. One that took the
/// web process down every time it opened left a blank view with no way to read the
/// note on this device (R2-109). These run the real bundles.
@MainActor
@Suite(.serialized)
struct WebEditorSafetyTests {

    private func webViews(in view: UIView) -> [WKWebView] {
        ((view as? WKWebView).map { [$0] } ?? []) + view.subviews.flatMap { webViews(in: $0) }
    }

    private func host(_ view: some View) throws -> UIWindow {
        let scene = try #require(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
        let window = UIWindow(windowScene: scene)
        window.rootViewController = UIHostingController(rootView: view)
        window.makeKeyAndVisible()
        return window
    }

    private func text(_ expression: String, in webView: WKWebView) async throws -> String {
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

    /// The last line is what shows the rest were refused and not that it hung: it
    /// only gets an answer once every line above it has come back.
    @Test func aNoteCantAskForMoreThanTheEditorCanHold() async throws {
        let note = """
        ones(60000, 60000)
        n = 60000
        zeros(n, n)
        identity(n)
        range(0, 20000000)
        1:20000000
        2 + 2
        """
        let window = try host(WebCalcEditorView(content: .constant(note)))
        defer { window.isHidden = true }
        await waitUntil { !webViews(in: window).isEmpty }
        let webView = try #require(webViews(in: window).first)

        let answered = await waitUntil {
            (try? await text("document.body.innerText.includes('4')", in: webView)) == "true"
        }
        #expect(answered)
        let refused = try await text("document.querySelectorAll('.cm-calc-error-line').length", in: webView)
        #expect(refused == "5")
    }

    @Test func ordinaryMatricesStillWork() async throws {
        let window = try host(WebCalcEditorView(content: .constant("sum(ones(100, 100))\nsum(range(1, 4))")))
        defer { window.isHidden = true }
        await waitUntil { !webViews(in: window).isEmpty }
        let webView = try #require(webViews(in: window).first)

        let answered = await waitUntil {
            (try? await text("document.querySelectorAll('.cm-calc-result').length", in: webView)) == "2"
        }

        #expect(answered)
        #expect(try await text("document.body.innerText.includes('10000')", in: webView) == "true")
        #expect(try await text("document.querySelectorAll('.cm-calc-error-line').length", in: webView) == "0")
    }

    private final class Unusable {
        var count = 0
    }

    /// Calling the delegate stands in for the process dying, which can't be arranged.
    @Test func aPageThatKeepsDyingIsGivenUpOn() async throws {
        let unusable = Unusable()
        let window = try host(WebCalcEditorView(content: .constant("2 + 2"), onUnusable: { unusable.count += 1 }))
        defer { window.isHidden = true }
        await waitUntil { !webViews(in: window).isEmpty }
        let webView = try #require(webViews(in: window).first)
        let delegate = try #require(webView.navigationDelegate)

        delegate.webViewWebContentProcessDidTerminate?(webView)
        delegate.webViewWebContentProcessDidTerminate?(webView)
        #expect(unusable.count == 0, "worth reloading")

        delegate.webViewWebContentProcessDidTerminate?(webView)
        #expect(unusable.count == 1)
    }

    @Test func aPageThatDiesOnceComesBackWithItsContent() async throws {
        let window = try host(WebCalcEditorView(content: .constant("40 + 2")))
        defer { window.isHidden = true }
        await waitUntil { !webViews(in: window).isEmpty }
        let webView = try #require(webViews(in: window).first)
        await waitUntil { (try? await text("document.body.innerText.includes('42')", in: webView)) == "true" }

        // A reload that loses the content shows nothing, so look for it coming back.
        _ = try await text("(document.body.innerHTML = '')", in: webView)
        webView.navigationDelegate?.webViewWebContentProcessDidTerminate?(webView)

        let restored = await waitUntil {
            (try? await text("document.body.innerText.includes('42')", in: webView)) == "true"
        }
        #expect(restored)
    }
}
