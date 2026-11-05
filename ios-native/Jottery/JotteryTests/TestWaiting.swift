import Foundation

// How tests wait. A CI runner can stall the main actor and the CPU for a minute,
// so a test must never need one thing to finish before a fixed delay runs out.
// Waiting for something that will happen goes through `waitUntil`. Keeping
// something from happening until the test has looked goes through a gate.

/// Waits until `condition` is true, and returns whether it ever was.
///
/// Counted in attempts, never by the clock: a stall costs one attempt, however
/// long it lasts. There are enough that even a run of stalls can't use them up,
/// and only a test that's going to fail ever comes near the end.
@discardableResult
func waitUntil(
    isolation: isolated (any Actor)? = #isolation,
    _ condition: () async throws -> Bool
) async rethrows -> Bool {
    for _ in 0..<15_000 {
        if try await condition() { return true }
        try? await Task.sleep(for: .milliseconds(20))
    }
    return try await condition()
}

/// Holds whoever calls `wait()` until the test calls `open()`, so that something
/// slow stays in progress for exactly as long as the test needs, on any machine.
actor AsyncGate {
    private var isOpen = false
    private var waiting: [CheckedContinuation<Void, Never>] = []

    /// How many callers are being held right now.
    var waitingCount: Int { waiting.count }

    func wait() async {
        guard !isOpen else { return }
        await withCheckedContinuation { waiting.append($0) }
    }

    func open() {
        isOpen = true
        waiting.forEach { $0.resume() }
        waiting = []
    }
}
