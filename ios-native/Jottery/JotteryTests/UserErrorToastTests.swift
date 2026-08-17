import Foundation
import Testing

@testable import Jottery

/// Covers the auto-clearing user-error toast surface (jottery-jqx8):
/// `AppState.reportError(_:duration:)` publishes `userErrorMessage`
/// immediately and clears it again once `duration` elapses, unless a
/// newer message replaced it first. Uses a short, explicit `duration`
/// instead of the real 4s default so the test runs fast without a fake
/// clock — mirrors the existing `syncStatusMessage` auto-clear pattern.
@MainActor
struct UserErrorToastTests {

    @Test func reportErrorPublishesMessage() {
        let state = AppState()
        #expect(state.userErrorMessage == nil)

        state.reportError("Couldn't delete note", duration: .seconds(4))

        #expect(state.userErrorMessage == "Couldn't delete note")
    }

    @Test func reportErrorAutoClearsAfterDuration() async {
        let state = AppState()

        // Generous margin (10x the clear duration) — this suite runs
        // alongside ~130 other tests under Swift Testing's parallel
        // executor, so wall-clock delays of a few hundred ms under load are
        // expected; only the relative ordering (clear fires before the
        // assertion, not exact timing) matters here.
        state.reportError("Couldn't delete note", duration: .milliseconds(100))
        #expect(state.userErrorMessage == "Couldn't delete note")

        try? await Task.sleep(for: .seconds(1))

        #expect(state.userErrorMessage == nil)
    }

    @Test func newerMessageIsNotClobberedByAnOlderMessagesTimer() async {
        let state = AppState()

        state.reportError("first failure", duration: .milliseconds(100))
        state.reportError("second failure", duration: .milliseconds(400))

        // The first message's clear-timer must not fire and wipe the second
        // message out from under it. Checked well before either timer's
        // due, so scheduler contention from the wider parallel test run
        // can't flip this into a false failure.
        try? await Task.sleep(for: .milliseconds(200))
        #expect(state.userErrorMessage == "second failure")

        // Generous margin past the second timer's 400ms — see the
        // reportErrorAutoClearsAfterDuration comment above.
        try? await Task.sleep(for: .seconds(2))
        #expect(state.userErrorMessage == nil)
    }
}
