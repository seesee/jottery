import CryptoKit
import Foundation
import LocalAuthentication

/// Manages the in-memory master key, auto-lock timer, and biometric unlock.
///
/// The `SymmetricKey` is kept in memory only — never persisted to disk.
/// Biometric unlock stores the raw key bytes in Keychain with
/// `SecAccessControl` flag `.biometryCurrentSet`.
///
/// CryptoKit zeroes a `SymmetricKey` when the last reference goes, so locking
/// clears the key provided nothing else holds it, which is why `lock()` also
/// ends sync. Key bytes that have to pass through `Data` are wiped once used
/// (`Data.wipe()`). That's as far as it can go: Swift gives no control over
/// copies the runtime makes, so this narrows the window rather than closing it.
@MainActor @Observable
final class KeyManager {

    // MARK: - State

    /// The in-memory master key. Internal setter to allow password change flow.
    var masterKey: SymmetricKey?
    private(set) var isUnlocked: Bool = false

    /// Seconds since last user interaction before auto-locking. Zero or less
    /// means never. Takes effect at once, whether or not a timer is running.
    var autoLockTimeout: TimeInterval = 15 * 60 {  // 15 minutes default
        didSet {
            if isUnlocked { startAutoLockTimer() }
        }
    }

    /// Called when the auto-lock timer fires. AppState should wire this
    /// to its own `lock()` so the UI transitions to the unlock screen.
    var onAutoLock: (() -> Void)?

    /// Measured on a clock that counts through sleep and can't be moved. On the
    /// wall clock, setting the date back in Settings would stop auto-lock.
    private var lastActivity = ContinuousClock.Instant.now
    private var autoLockTimer: Timer?

    // MARK: - Unlock

    /// Restore the master key from raw bytes (biometric unlock path).
    func unlockWithKeyData(_ keyData: Data) {
        masterKey = SymmetricKey(data: keyData)
        isUnlocked = true
        lastActivity = .now
        startAutoLockTimer()
    }

    /// Lock the application — wipe the in-memory key.
    func lock() {
        masterKey = nil
        isUnlocked = false
        stopAutoLockTimer()
    }

    /// Record user activity to reset the auto-lock countdown.
    func recordActivity(at instant: ContinuousClock.Instant = .now) {
        lastActivity = instant
    }

    // MARK: - Biometric Key Storage

    /// Store the current master key in Keychain protected by biometrics.
    func enableBiometricUnlock() throws {
        guard let key = masterKey else {
            throw KeyManagerError.noKeyLoaded
        }
        var keyData = key.withUnsafeBytes { Data($0) }
        defer { keyData.wipe() }
        try KeychainService.storeBiometricKey(keyData)
    }

    /// Guards against overlapping biometric prompts — the keychain read
    /// below implicitly presents Face ID, so two concurrent calls would
    /// queue two prompts (seen when UnlockScreen's appear lifecycle
    /// runs twice on cold launch).
    private var isBiometricUnlockInFlight = false

    /// Attempt to retrieve the master key via biometric authentication.
    /// Returns `true` if successful. `retrieve` is injectable for tests.
    func attemptBiometricUnlock(
        retrieve: @Sendable () async throws -> Data = { try await KeychainService.retrieveBiometricKey() }
    ) async -> Bool {
        guard !isBiometricUnlockInFlight else { return false }
        isBiometricUnlockInFlight = true
        defer { isBiometricUnlockInFlight = false }
        do {
            var keyData = try await retrieve()
            defer { keyData.wipe() }
            unlockWithKeyData(keyData)
            return true
        } catch {
            return false
        }
    }

    /// Remove biometric key from Keychain.
    func disableBiometricUnlock() {
        KeychainService.deleteBiometricKey()
    }

    /// Check if biometric unlock is available and enrolled.
    var isBiometricAvailable: Bool {
        let context = LAContext()
        var error: NSError?
        return context.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &error)
    }

    /// Check if a biometric key is stored.
    var isBiometricEnabled: Bool {
        KeychainService.hasBiometricKey()
    }

    // MARK: - Auto-Lock

    private func startAutoLockTimer() {
        stopAutoLockTimer()
        guard autoLockTimeout > 0 else { return }

        autoLockTimer = Timer.scheduledTimer(withTimeInterval: 30, repeats: true) { [weak self] _ in
            Task { @MainActor in
                self?.checkAutoLock()
            }
        }
    }

    private func stopAutoLockTimer() {
        autoLockTimer?.invalidate()
        autoLockTimer = nil
    }

    /// Whether the clock is running: there's something to lock, and a timeout.
    var isAutoLockTimerRunning: Bool { autoLockTimer != nil }

    func checkAutoLock() {
        guard isUnlocked, autoLockTimeout > 0 else { return }
        if lastActivity.duration(to: .now) >= .seconds(autoLockTimeout) {
            // Use the callback so AppState can lock the full app (show unlock screen,
            // clear notes, stop SSE). The callback calls appState.lock() which in turn
            // calls keyManager.lock().
            if let onAutoLock {
                onAutoLock()
            } else {
                lock()
            }
        }
    }

    // No deinit needed — KeyManager lives for the app's lifetime.
    // Timer invalidation happens in lock() / stopAutoLockTimer().
}

extension Data {
    /// Overwrite these bytes, once they've done their job.
    mutating func wipe() {
        resetBytes(in: startIndex..<endIndex)
    }
}

enum KeyManagerError: LocalizedError {
    case noKeyLoaded

    var errorDescription: String? {
        switch self {
        case .noKeyLoaded: return L.keyErrorNoKeyLoaded
        }
    }
}
