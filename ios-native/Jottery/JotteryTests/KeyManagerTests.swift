import Foundation
import Testing

@testable import Jottery

@MainActor
struct KeyManagerTests {

    @Test func concurrentBiometricUnlockAttemptsOnlyTriggerOnePrompt() async {
        let manager = KeyManager()
        // Each retriever invocation stands in for one Face ID prompt, which
        // stays up until the test ends it.
        let counter = PromptCounter()
        let prompt = AsyncGate()
        let retrieve: @Sendable () async throws -> Data = {
            await counter.increment()
            await prompt.wait()
            return Data(repeating: 7, count: 32)
        }

        // Two overlapping attempts — mirrors UnlockScreen.onAppear firing twice.
        // The second starts while the first is provably still waiting on Face ID.
        let first = Task { await manager.attemptBiometricUnlock(retrieve: retrieve) }
        await waitUntil { await prompt.waitingCount == 1 }
        let second = await manager.attemptBiometricUnlock(retrieve: retrieve)
        await prompt.open()

        #expect(second == false)                   // refused while one is in flight
        #expect(await first.value == true)         // the real attempt succeeded
        #expect(await counter.count == 1)          // one prompt, not two
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

/// "Never" used to lock within 30 seconds (jottery-7wi1).
@MainActor
struct KeyManagerAutoLockTests {

    @Test func neverMeansNever() {
        let manager = KeyManager()
        manager.unlockWithKeyData(Data(repeating: 7, count: 32))

        manager.autoLockTimeout = 0
        manager.checkAutoLock()

        #expect(manager.isUnlocked)
    }

    @Test func aTimeoutThatHasPassedLocks() {
        let manager = KeyManager()
        manager.unlockWithKeyData(Data(repeating: 7, count: 32))
        manager.autoLockTimeout = 60

        manager.recordActivity(at: .now - .seconds(61))
        manager.checkAutoLock()

        #expect(!manager.isUnlocked)
    }

    @Test func aTimeoutThatHasntPassedDoesntLock() {
        let manager = KeyManager()
        manager.unlockWithKeyData(Data(repeating: 7, count: 32))
        manager.autoLockTimeout = 60

        manager.recordActivity(at: .now - .seconds(30))
        manager.checkAutoLock()

        #expect(manager.isUnlocked)
    }

    @Test func choosingNeverStopsTheTimer() {
        let manager = KeyManager()
        manager.unlockWithKeyData(Data(repeating: 7, count: 32))
        #expect(manager.isAutoLockTimerRunning)

        manager.autoLockTimeout = 0

        #expect(!manager.isAutoLockTimerRunning)
    }

    @Test func choosingATimeoutAfterNeverStartsTheTimer() {
        let manager = KeyManager()
        manager.autoLockTimeout = 0
        manager.unlockWithKeyData(Data(repeating: 7, count: 32))
        #expect(!manager.isAutoLockTimerRunning)

        manager.autoLockTimeout = 60

        #expect(manager.isAutoLockTimerRunning)
    }
}
