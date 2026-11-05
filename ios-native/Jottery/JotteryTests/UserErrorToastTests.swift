import Foundation
import Testing

@testable import Jottery

/// Covers the auto-clearing user-error toast surface (jottery-jqx8):
/// `AppState.reportError(_:duration:)` publishes `userErrorMessage`
/// immediately and clears it again once `duration` elapses, unless a
/// newer message replaced it first. A test that needs a timer not to have
/// fired decides when it fires (`waitOutErrorMessage`), because no delay is
/// certain to outlast a stalled runner.
@MainActor
struct UserErrorToastTests {

    private func waitForMessageToClear(in state: AppState) async {
        await waitUntil { state.userErrorMessage == nil }
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
        let first = AsyncGate()
        let second = AsyncGate()
        state.waitOutErrorMessage = { duration in
            await (duration == .milliseconds(100) ? first : second).wait()
        }

        state.reportError("first failure", duration: .milliseconds(100))
        state.reportError("second failure", duration: .milliseconds(400))
        await waitUntil {
            let firstIsWaiting = await first.waitingCount == 1
            let secondIsWaiting = await second.waitingCount == 1
            return firstIsWaiting && secondIsWaiting
        }

        // The first message's clear-timer fires, and must not wipe the second
        // message out from under it.
        await first.open()
        for _ in 0..<10 { await Task.yield() }
        #expect(state.userErrorMessage == "second failure")

        await second.open()
        await waitForMessageToClear(in: state)
        #expect(state.userErrorMessage == nil)
    }
}
