import SwiftUI
import Testing
import UIKit
import WebKit

@testable import Jottery

/// Words typed into a note can reach the system keyboard's dictionary, outside the
/// vault, and turn up as suggestions in other apps. So nothing a note holds may be
/// typed into a field that learns (R2-108).
@MainActor
@Suite(.serialized)
struct KeyboardPrivacyTests {

    private func makeWindow(_ view: some View) throws -> UIWindow {
        let scene = try #require(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
        let window = UIWindow(windowScene: scene)
        window.rootViewController = UIHostingController(rootView: view)
        window.makeKeyAndVisible()
        return window
    }

    private func all<T: UIView>(_ type: T.Type, in view: UIView) -> [T] {
        ((view as? T).map { [$0] } ?? []) + view.subviews.flatMap { all(type, in: $0) }
    }

    @Test func theTagFieldLearnsNothing() async throws {
        let state = try TestVaults.makeState()
        let window = try makeWindow(TagInputView(tags: .constant([])).environment(state))
        defer { window.isHidden = true }

        await waitUntil { !all(UITextField.self, in: window).isEmpty }

        let field = try #require(all(UITextField.self, in: window).first)
        #expect(field.autocorrectionType == .no)
        #expect(field.autocapitalizationType == .none)
    }

    @Test func theSearchFieldLearnsNothing() async throws {
        let state = try TestVaults.makeState()
        try await state.createVault(password: "notes-password-correct-horse")
        let window = try makeWindow(NavigationStack { NoteListView(showsTagsInList: false) }.environment(state))
        defer { window.isHidden = true }

        await waitUntil { !all(UISearchTextField.self, in: window).isEmpty }

        let field = try #require(all(UISearchTextField.self, in: window).first)
        #expect(field.autocorrectionType == .no)
        #expect(field.autocapitalizationType == .none)
    }

    @Test func outlinerRowsLearnNothing() async throws {
        let window = try makeWindow(WebOutlinerEditorView(content: .constant("- a private name")))
        defer { window.isHidden = true }

        await waitUntil { !all(WKWebView.self, in: window).isEmpty }
        let webView = try #require(all(WKWebView.self, in: window).first)

        func attribute(_ name: String) async throws -> String {
            let script = "String(document.querySelector('.outliner-content')?.getAttribute('\(name)'))"
            return try await withCheckedThrowingContinuation { continuation in
                webView.evaluateJavaScript(script) { result, error in
                    if let error {
                        continuation.resume(throwing: error)
                    } else {
                        continuation.resume(returning: result as? String ?? "")
                    }
                }
            }
        }

        await waitUntil { (try? await attribute("autocorrect")) == "off" }

        #expect(try await attribute("autocorrect") == "off")
        #expect(try await attribute("autocapitalize") == "off")
        #expect(try await attribute("spellcheck") == "false")
    }
}
