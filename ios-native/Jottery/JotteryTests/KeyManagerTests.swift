import Foundation
import Testing

@testable import Jottery

@MainActor
struct KeyManagerTests {

    @Test func concurrentBiometricUnlockAttemptsOnlyTriggerOnePrompt() async {
        let manager = KeyManager()
        // Each retriever invocation stands in for one Face ID prompt.
        let counter = PromptCounter()
        let slowRetrieve: @Sendable () async throws -> Data = {
            await counter.increment()
            try await Task.sleep(for: .milliseconds(200))
            return Data(repeating: 7, count: 32)
        }

        // Two overlapping attempts — mirrors UnlockScreen.onAppear firing twice.
        async let first = manager.attemptBiometricUnlock(retrieve: slowRetrieve)
        async let second = manager.attemptBiometricUnlock(retrieve: slowRetrieve)
        let results = await [first, second]

        #expect(await counter.count == 1)          // one prompt, not two
        #expect(results.contains(true))            // the real attempt succeeded
        #expect(manager.isUnlocked)
    }

    @Test func failedAttemptClearsInFlightFlag() async {
        let manager = KeyManager()
        let failing: @Sendable () async throws -> Data = {
            throw KeyManagerError.noKeyLoaded
        }
        let result = await manager.attemptBiometricUnlock(retrieve: failing)
        #expect(result == false)

        // A later attempt must not be blocked by a stuck flag.
        let succeeding: @Sendable () async throws -> Data = {
            Data(repeating: 7, count: 32)
        }
        let retry = await manager.attemptBiometricUnlock(retrieve: succeeding)
        #expect(retry == true)
    }
}

private actor PromptCounter {
    var count = 0
    func increment() { count += 1 }
}
