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

    /// The alpha of each point, from a copy in a layout chosen here. The renderer's
    /// own bitmap is little-endian, so its first byte in memory is blue, not alpha,
    /// and reading that passes or fails by the colour of the cover.
    private func alpha(of image: CGImage, at points: [(x: Int, y: Int)]) throws -> [UInt8] {
        let colorSpace = try #require(CGColorSpace(name: CGColorSpace.sRGB))
        let context = try #require(CGContext(
            data: nil, width: image.width, height: image.height, bitsPerComponent: 8,
            bytesPerRow: image.width * 4, space: colorSpace,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue
        ))
        context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        let bytes = try #require(context.data).assumingMemoryBound(to: UInt8.self)
        return points.map { bytes[$0.y * context.bytesPerRow + $0.x * 4 + 3] }
    }

    /// It's no use if what's beneath shows through, or it hasn't drawn yet by
    /// the time the system takes its snapshot. Its background follows the
    /// appearance, so both are checked whatever the simulator happens to be set to.
    @Test(arguments: [UIUserInterfaceStyle.light, .dark])
    func isOpaqueAsSoonAsItsUp(style: UIUserInterfaceStyle) async throws {
        let state = try await makeUnlockedState()
        let scene = try connectedScene()
        let controller = PrivacyCoverController(appState: state)

        controller.coverIfNeeded(scene)
        defer { controller.lift(from: scene) }

        let window = try #require(controller.coverWindow(for: scene))
        window.overrideUserInterfaceStyle = style
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        format.preferredRange = .standard
        var drew = false
        let image = UIGraphicsImageRenderer(bounds: window.bounds, format: format).image { _ in
            drew = window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
        }
        #expect(drew, "nothing was drawn, so the pixels below say nothing about the cover")

        let cgImage = try #require(image.cgImage)
        let (width, height) = (cgImage.width, cgImage.height)
        let corners = [(x: 2, y: 2), (x: width - 3, y: 2), (x: 2, y: height - 3), (x: width / 2, y: height - 3)]
        #expect(try alpha(of: cgImage, at: corners) == [255, 255, 255, 255])
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
