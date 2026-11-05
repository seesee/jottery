import SwiftUI
import Testing
import UIKit

@testable import Jottery

/// At large text sizes, or with the keyboard up in landscape, a fixed stack puts
/// the buttons off screen with no way to reach them (jottery-mo3o).
@MainActor
@Suite(.serialized)
struct ScrollableScreenTests {

    private func scrollViews(in view: UIView) -> [UIScrollView] {
        ((view as? UIScrollView).map { [$0] } ?? []) + view.subviews.flatMap { scrollViews(in: $0) }
    }

    /// A short window and the largest text, so nothing can fit.
    private func host(_ view: some View) throws -> UIWindow {
        let scene = try #require(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
        let window = UIWindow(windowScene: scene)
        window.frame = CGRect(x: 0, y: 0, width: 320, height: 320)
        window.rootViewController = UIHostingController(rootView: view.dynamicTypeSize(.accessibility5))
        window.makeKeyAndVisible()
        return window
    }

    private func expectItScrolls(_ window: UIWindow) async throws {
        let scrolls = await waitUntil {
            scrollViews(in: window).contains { $0.contentSize.height > $0.bounds.height && $0.bounds.height > 0 }
        }
        #expect(scrolls, "what doesn't fit can't be reached")
    }

    @Test func theUnlockScreenScrollsWhenItDoesntFit() async throws {
        let state = try TestVaults.makeState()
        let window = try host(UnlockScreen().environment(state))
        defer { window.isHidden = true }

        try await expectItScrolls(window)
    }

    @Test func theSetupScreenScrollsWhenItDoesntFit() async throws {
        let state = try TestVaults.makeState()
        let window = try host(SetupScreen().environment(state))
        defer { window.isHidden = true }

        try await expectItScrolls(window)
    }

    /// Where there's room it still centres, as a stack does: the scroll view's
    /// content fills the screen rather than sitting at the top of it.
    @Test func contentFillsAScreenWithRoomToSpare() async throws {
        let scene = try #require(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
        let window = UIWindow(windowScene: scene)
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 800)
        window.rootViewController = UIHostingController(rootView: ScrollableScreen { Text("Short") })
        window.makeKeyAndVisible()
        defer { window.isHidden = true }

        let filled = await waitUntil {
            // The bounds include the safe area, and the content doesn't.
            scrollViews(in: window).contains { scroll in
                let insets = scroll.adjustedContentInset.top + scroll.adjustedContentInset.bottom
                return scroll.bounds.height > 0 && scroll.contentSize.height + insets >= scroll.bounds.height - 1
            }
        }

        #expect(filled)
    }
}
