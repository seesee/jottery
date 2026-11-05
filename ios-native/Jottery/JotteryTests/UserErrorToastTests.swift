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

    /// Waits for the message to clear. `reportError` starts its timer from a
    /// main-actor task, so while another test holds the main actor that timer
    /// hasn't begun, and no fixed sleep or deadline is long enough. Counting
    /// attempts works however long the stall: it costs only one of them.
    private func waitForMessageToClear(in state: AppState) async {
        for _ in 0..<1500 where state.userErrorMessage != nil {
            try? await Task.sleep(for: .milliseconds(20))
        }
    }

    @Test func reportErrorPublishesMessage() {
        let state = AppState()
        #expect(state.userErrorMessage == nil)

        state.reportError("Couldn't delete note", duration: .seconds(4))

        #expect(state.userErrorMessage == "Couldn't delete note")
    }

    @Test func reportErrorAutoClearsAfterDuration() async {
        let state = AppState()

        state.reportError("Couldn't delete note", duration: .milliseconds(100))
        #expect(state.userErrorMessage == "Couldn't delete note")

        await waitForMessageToClear(in: state)

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

        await waitForMessageToClear(in: state)
        #expect(state.userErrorMessage == nil)
    }
}
