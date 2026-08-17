import SwiftUI

/// Opaque cover shown over `MainView` whenever the scene is not `.active`, so
/// the iOS app-switcher snapshot never contains decrypted note content.
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
