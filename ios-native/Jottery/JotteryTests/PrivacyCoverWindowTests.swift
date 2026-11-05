import SwiftUI
import Testing
import UIKit

@testable import Jottery

/// A sheet, QuickLook or the share sheet presents above anything in the SwiftUI
/// hierarchy, so the cover needs a window of its own (jottery-5337).
@MainActor
@Suite(.serialized)
struct PrivacyCoverWindowTests {

    private func connectedScene() throws -> UIWindowScene {
        try #require(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
    }

    private func makeUnlockedState() async throws -> AppState {
        let state = try TestVaults.makeState()
        try await state.createVault(password: "notes-password-correct-horse")
        return state
    }

    @Test func coversAnUnlockedVault() async throws {
        let state = try await makeUnlockedState()
        let scene = try connectedScene()
        let controller = PrivacyCoverController(appState: state)

        controller.coverIfNeeded(scene)
        defer { controller.lift(from: scene) }

        let window = try #require(controller.coverWindow(for: scene))
        #expect(!window.isHidden)
        #expect(window.windowScene === scene)
        #expect(window.rootViewController?.view.bounds.isEmpty == false)
    }

    /// It's no use if what's beneath shows through, or it hasn't drawn yet by
    /// the time the system takes its snapshot.
    @Test func isOpaqueAsSoonAsItsUp() async throws {
        let state = try await makeUnlockedState()
        let scene = try connectedScene()
        let controller = PrivacyCoverController(appState: state)

        controller.coverIfNeeded(scene)
        defer { controller.lift(from: scene) }

        let window = try #require(controller.coverWindow(for: scene))
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        format.preferredRange = .standard
        let image = UIGraphicsImageRenderer(bounds: window.bounds, format: format).image { _ in
            window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
        }
        let cgImage = try #require(image.cgImage)
        let data = try #require(cgImage.dataProvider?.data as Data?)
        #expect(cgImage.bitsPerComponent == 8)
        let bytesPerPixel = cgImage.bitsPerPixel / 8
        let alphaOffset = cgImage.alphaInfo == .premultipliedFirst || cgImage.alphaInfo == .first ? 0 : bytesPerPixel - 1
        for (x, y) in [(2, 2), (cgImage.width - 3, 2), (2, cgImage.height - 3), (cgImage.width / 2, cgImage.height - 3)] {
            #expect(data[y * cgImage.bytesPerRow + x * bytesPerPixel + alphaOffset] == 255)
        }
    }

    /// The point of the window: a modal presented in the app's own window is
    /// still underneath it.
    @Test func sitsAboveAPresentedSheet() async throws {
        let state = try await makeUnlockedState()
        let scene = try connectedScene()
        let controller = PrivacyCoverController(appState: state)
        let app = UIWindow(windowScene: scene)
        app.windowLevel = .normal
        app.rootViewController = UIViewController()
        app.isHidden = false
        defer { app.isHidden = true }
        let sheet = UIViewController()
        app.rootViewController?.present(sheet, animated: false)
        defer { sheet.dismiss(animated: false) }

        controller.coverIfNeeded(scene)
        defer { controller.lift(from: scene) }

        let window = try #require(controller.coverWindow(for: scene))
        #expect(window.windowLevel > app.windowLevel)
        #expect(window.windowLevel > .alert)
        #expect(sheet.view.window !== window)
        #expect(scene.windows.max { $0.windowLevel < $1.windowLevel && !$1.isHidden } === window)
    }

    @Test func noCoverOverTheUnlockScreen() async throws {
        let state = try await makeUnlockedState()
        state.lock()
        let scene = try connectedScene()
        let controller = PrivacyCoverController(appState: state)

        controller.coverIfNeeded(scene)

        #expect(controller.coverWindow(for: scene) == nil)
    }

    @Test func noCoverDuringSetup() throws {
        let state = try TestVaults.makeState()
        #expect(state.isFirstLaunch)
        let scene = try connectedScene()
        let controller = PrivacyCoverController(appState: state)

        controller.coverIfNeeded(scene)

        #expect(controller.coverWindow(for: scene) == nil)
    }

    @Test func liftsAtOnceOnAnOrdinaryReturn() async throws {
        let state = try await makeUnlockedState()
        let scene = try connectedScene()
        let controller = PrivacyCoverController(appState: state)
        controller.coverIfNeeded(scene)

        controller.lift(from: scene)

        #expect(controller.coverWindow(for: scene) == nil)
        #expect(!state.isLocked)
    }

    /// Lifting first would show the note for a moment before the lock landed.
    @Test func aReturnPastTheTimeoutLocksBeforeTheCoverLifts() async throws {
        let state = try await makeUnlockedState()
        state.keyManager.autoLockTimeout = 60
        state.backgroundedAt = .now - .seconds(120)
        let scene = try connectedScene()
        let controller = PrivacyCoverController(appState: state)
        controller.coverIfNeeded(scene)

        controller.lift(from: scene)

        #expect(state.isLocked)
        #expect(controller.coverWindow(for: scene) != nil)

        await waitUntil { controller.coverWindow(for: scene) == nil }
        #expect(controller.coverWindow(for: scene) == nil)
    }

    @Test func aReturnWithinTheTimeoutDoesntLock() async throws {
        let state = try await makeUnlockedState()
        state.keyManager.autoLockTimeout = 600
        state.backgroundedAt = .now - .seconds(5)

        #expect(!state.lockIfAwayTooLong())
        #expect(!state.isLocked)
    }

    @Test func aTimeoutOfZeroMeansNever() async throws {
        let state = try await makeUnlockedState()
        state.keyManager.autoLockTimeout = 0
        state.backgroundedAt = .now - .seconds(86_400)

        #expect(!state.lockIfAwayTooLong())
    }

    /// The scene phase and the scene notification both arrive on a return.
    @Test func checkingTwiceLocksOnce() async throws {
        let state = try await makeUnlockedState()
        state.keyManager.autoLockTimeout = 60
        state.backgroundedAt = .now - .seconds(120)

        #expect(state.lockIfAwayTooLong())
        #expect(!state.lockIfAwayTooLong())
        state.handleScenePhaseChange(.active)
        #expect(state.isLocked)
    }

    @Test func anUnfinishedLiftIsCancelledIfTheAppLeavesAgain() async throws {
        let state = try await makeUnlockedState()
        state.keyManager.autoLockTimeout = 60
        state.backgroundedAt = .now - .seconds(120)
        let scene = try connectedScene()
        let controller = PrivacyCoverController(appState: state)
        controller.coverIfNeeded(scene)
        controller.lift(from: scene)
        let lockedWindow = try #require(controller.coverWindow(for: scene))

        // Unlocked again and gone before the delayed lift ran.
        try await state.unlock(password: "notes-password-correct-horse")
        controller.coverIfNeeded(scene)
        defer { controller.lift(from: scene) }

        for _ in 0..<40 {
            try? await Task.sleep(for: .milliseconds(20))
        }
        #expect(!lockedWindow.isHidden)
    }
}
