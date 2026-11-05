import SwiftUI
import UIKit

/// Opaque cover shown whenever the scene is not `.active`, so the iOS
/// app-switcher snapshot never contains decrypted note content. Shown by
/// `PrivacyCoverController`.
///
/// Deliberately never shown over `UnlockScreen`/`SetupScreen` — those have
/// nothing sensitive on screen, and the Face ID prompt itself drives the
/// scene to `.inactive` while `UnlockScreen` is visible, so gating on
/// `isLocked`/`isFirstLaunch` (rather than scene phase alone) avoids a
/// cover flash during biometric unlock.
struct PrivacyCoverView: View {
    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: "lock.fill")
                .font(.system(size: 48))
                .foregroundStyle(.accent)

            Text(L.unlockTitle)
                .font(.title2.bold())
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(.background)
        .ignoresSafeArea()
    }
}

/// Pure decision logic, factored out so it can be unit-tested without
/// standing up a `ScenePhase` in a live scene.
enum PrivacyCover {
    static func isVisible(scenePhase: ScenePhase, isLocked: Bool, isFirstLaunch: Bool) -> Bool {
        guard scenePhase != .active else { return false }
        guard !isFirstLaunch else { return false }
        guard !isLocked else { return false }
        return true
    }
}

/// Puts `PrivacyCoverView` in a window of its own above everything else.
///
/// An overlay in the SwiftUI hierarchy isn't enough: sheets, QuickLook and the
/// share sheet present above it, so a note or attachment open in one of those
/// reached the app switcher's snapshot. SwiftUI's `scenePhase` is also too late
/// to rely on, because it's delivered in an update some time after the
/// transition. The cover goes up, with no animation, from the scene's own
/// notifications, which are posted synchronously.
@MainActor
final class PrivacyCoverController: NSObject {

    /// How long the cover stays after a return that locks the app, so what was
    /// on screen has gone before it lifts. Sheets take a moment to dismiss.
    private static let lockedLiftDelay: Duration = .milliseconds(500)

    private let appState: AppState
    private var windows: [ObjectIdentifier: UIWindow] = [:]
    private var pendingLifts: [ObjectIdentifier: Task<Void, Never>] = [:]

    init(appState: AppState) {
        self.appState = appState
        super.init()
        let center = NotificationCenter.default
        center.addObserver(self, selector: #selector(sceneWillDeactivate), name: UIScene.willDeactivateNotification, object: nil)
        center.addObserver(self, selector: #selector(sceneDidEnterBackground), name: UIScene.didEnterBackgroundNotification, object: nil)
        center.addObserver(self, selector: #selector(sceneDidActivate), name: UIScene.didActivateNotification, object: nil)
        center.addObserver(self, selector: #selector(sceneDidDisconnect), name: UIScene.didDisconnectNotification, object: nil)
    }

    @objc private func sceneWillDeactivate(_ notification: Notification) {
        guard let scene = notification.object as? UIWindowScene else { return }
        coverIfNeeded(scene)
    }

    /// Also here, because an unlock can finish after the scene went inactive.
    @objc private func sceneDidEnterBackground(_ notification: Notification) {
        guard let scene = notification.object as? UIWindowScene else { return }
        appState.noteEnteredBackground()
        coverIfNeeded(scene)
    }

    @objc private func sceneDidActivate(_ notification: Notification) {
        guard let scene = notification.object as? UIWindowScene else { return }
        lift(from: scene)
    }

    @objc private func sceneDidDisconnect(_ notification: Notification) {
        guard let scene = notification.object as? UIWindowScene else { return }
        let id = ObjectIdentifier(scene)
        pendingLifts.removeValue(forKey: id)?.cancel()
        windows[id] = nil
    }

    /// The window currently covering `scene`, if any.
    func coverWindow(for scene: UIWindowScene) -> UIWindow? {
        windows[ObjectIdentifier(scene)].flatMap { $0.isHidden ? nil : $0 }
    }

    func coverIfNeeded(_ scene: UIWindowScene) {
        guard PrivacyCover.isVisible(
            scenePhase: .inactive,
            isLocked: appState.isLocked,
            isFirstLaunch: appState.isFirstLaunch
        ) else { return }

        let id = ObjectIdentifier(scene)
        pendingLifts.removeValue(forKey: id)?.cancel()
        let window = windows[id] ?? makeWindow(for: scene)
        windows[id] = window
        window.isHidden = false
        // The snapshot can be taken before the next run loop turn.
        window.rootViewController?.view.layoutIfNeeded()
        CATransaction.flush()
    }

    /// The auto-lock check runs first, so a return that locks the app never
    /// shows what was on screen.
    func lift(from scene: UIWindowScene) {
        let id = ObjectIdentifier(scene)
        guard let window = windows[id], !window.isHidden else {
            appState.lockIfAwayTooLong()
            return
        }
        guard !appState.lockIfAwayTooLong(), !appState.isLocked else {
            pendingLifts[id]?.cancel()
            pendingLifts[id] = Task { [weak window] in
                try? await Task.sleep(for: Self.lockedLiftDelay)
                guard !Task.isCancelled else { return }
                window?.isHidden = true
            }
            return
        }
        window.isHidden = true
    }

    private func makeWindow(for scene: UIWindowScene) -> UIWindow {
        let window = UIWindow(windowScene: scene)
        window.windowLevel = .alert + 1
        window.rootViewController = UIHostingController(rootView: PrivacyCoverView())
        window.accessibilityViewIsModal = true
        return window
    }
}
