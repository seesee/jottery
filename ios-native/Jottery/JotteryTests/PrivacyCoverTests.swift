import SwiftUI
import Testing

@testable import Jottery

/// Covers the pure decision rule behind the app-switcher privacy cover
/// (jottery-jm02). The view itself can't be exercised without a live scene,
/// but the rule that decides whether it's shown is a plain function and
/// fully testable.
struct PrivacyCoverTests {

    @Test func hiddenWhenActive() {
        #expect(!PrivacyCover.isVisible(scenePhase: .active, isLocked: false, isFirstLaunch: false))
    }

    @Test func visibleWhenBackgroundedAndUnlocked() {
        #expect(PrivacyCover.isVisible(scenePhase: .background, isLocked: false, isFirstLaunch: false))
    }

    @Test func visibleWhenInactiveAndUnlocked() {
        #expect(PrivacyCover.isVisible(scenePhase: .inactive, isLocked: false, isFirstLaunch: false))
    }

    @Test func hiddenWhenLockedRegardlessOfScenePhase() {
        // Guards against a flash over UnlockScreen while the Face ID prompt
        // (which drives the scene to .inactive) is on screen.
        #expect(!PrivacyCover.isVisible(scenePhase: .inactive, isLocked: true, isFirstLaunch: false))
        #expect(!PrivacyCover.isVisible(scenePhase: .background, isLocked: true, isFirstLaunch: false))
    }

    @Test func hiddenDuringFirstLaunchRegardlessOfScenePhase() {
        // Guards against a flash over SetupScreen.
        #expect(!PrivacyCover.isVisible(scenePhase: .inactive, isLocked: false, isFirstLaunch: true))
        #expect(!PrivacyCover.isVisible(scenePhase: .background, isLocked: false, isFirstLaunch: true))
    }

    @Test func hiddenWhenActiveEvenIfLockedOrFirstLaunch() {
        #expect(!PrivacyCover.isVisible(scenePhase: .active, isLocked: true, isFirstLaunch: false))
        #expect(!PrivacyCover.isVisible(scenePhase: .active, isLocked: false, isFirstLaunch: true))
    }
}
