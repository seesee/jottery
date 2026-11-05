import Foundation
import Testing

@testable import Jottery

/// iOS kills an app that takes over 10 seconds to start, and opening the database
/// can take that long on a real vault. Killed partway, the same work starts again
/// at the next launch, so the app can stop opening altogether.
@MainActor
struct LaunchTests {

    private func temporaryDatabasePath() throws -> String {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-launch-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir.appendingPathComponent("test.db").path
    }

    private func waitFor(_ semaphore: DispatchSemaphore) async {
        await withCheckedContinuation { continuation in
            DispatchQueue.global().async {
                semaphore.wait()
                continuation.resume()
            }
        }
    }

    private func makeState() -> AppState {
        let state = AppState()
        state.forgetEarlierInstall = {}
        return state
    }

    /// The open is held at a gate for as long as this test likes, so nothing here
    /// depends on timing. If it ran on the main thread, the test could never reach
    /// the line that opens the gate.
    @Test func theMainThreadStaysFreeWhileTheDatabaseOpens() async throws {
        let state = makeState()
        let path = try temporaryDatabasePath()
        let started = DispatchSemaphore(value: 0)
        let gate = DispatchSemaphore(value: 0)
        state.openDatabase = {
            started.signal()
            gate.wait()
            return try DatabaseManager(path: path)
        }

        let launch = Task { await state.initialise() }
        await waitFor(started)

        #expect(!state.isDatabaseReady)
        #expect(state.db == nil)

        gate.signal()
        await launch.value

        #expect(state.isDatabaseReady)
        #expect(state.db != nil)
    }

    @Test func aDatabaseThatWontOpenStillLeavesTheLoadingScreen() async {
        struct OpenFailed: Error {}
        let state = makeState()
        state.openDatabase = { throw OpenFailed() }

        await state.initialise()

        #expect(state.isDatabaseReady)
        #expect(state.isFirstLaunch)
        #expect(state.db == nil)
    }

    /// The first scene and becoming active both ask for it.
    @Test func askingTwiceOpensItOnce() async throws {
        let state = makeState()
        let path = try temporaryDatabasePath()
        let opens = OpenCounter()
        let started = DispatchSemaphore(value: 0)
        let gate = DispatchSemaphore(value: 0)
        state.openDatabase = {
            opens.record()
            started.signal()
            gate.wait()
            return try DatabaseManager(path: path)
        }

        let first = Task { await state.initialise() }
        await waitFor(started)
        await state.initialise()

        gate.signal()
        await first.value

        #expect(opens.count == 1)
    }
}

private final class OpenCounter: @unchecked Sendable {
    private let lock = NSLock()
    private var value = 0
    var count: Int { lock.withLock { value } }
    func record() { lock.withLock { value += 1 } }
}
