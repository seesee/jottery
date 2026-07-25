import Foundation

/// Debug-only diagnostics.
///
/// Jottery ships no telemetry and no crash reporting, so release logging has
/// nowhere useful to go — it only risks leaking note metadata and the user's
/// private sync server address into the device log. Every call here compiles
/// away in release builds, and the `@autoclosure` means the message is never
/// even constructed.
///
/// Messages carry their own `[Subsystem]` prefix, matching the convention the
/// call sites already used.
///
/// If release diagnostics are ever wanted, add a separate `Log.error` backed by
/// `os.Logger` with `privacy: .private` on the interpolated values.
enum Log {

    /// Log a diagnostic message. Compiles to nothing in release builds.
    static func debug(_ message: @autoclosure () -> String) {
        #if DEBUG
        print(message())
        #endif
    }
}
