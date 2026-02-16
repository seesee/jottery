import CryptoKit
import Foundation
import LocalAuthentication

/// Manages the in-memory master key, auto-lock timer, and biometric unlock.
///
/// The `SymmetricKey` is kept in memory only — never persisted to disk.
/// Biometric unlock stores the raw key bytes in Keychain with
/// `SecAccessControl` flag `.biometryCurrentSet`.
@MainActor @Observable
final class KeyManager {

    // MARK: - State

    /// The in-memory master key. Internal setter to allow password change flow.
    internal(set) var masterKey: SymmetricKey?
    private(set) var isUnlocked: Bool = false

    /// Seconds since last user interaction before auto-locking.
    var autoLockTimeout: TimeInterval = 15 * 60  // 15 minutes default

    /// Called when the auto-lock timer fires. AppState should wire this
    /// to its own `lock()` so the UI transitions to the unlock screen.
    var onAutoLock: (() -> Void)?

    private var lastActivityDate = Date()
    private var autoLockTimer: Timer?

    // MARK: - Unlock

    /// Derive the master key from a password and store it in memory.
    /// Returns `true` if derivation succeeds.
    func unlock(password: String, salt: Data, iterations: UInt32) -> SymmetricKey {
        let key = CryptoService.deriveKey(password: password, salt: salt, iterations: iterations)
        masterKey = key
        isUnlocked = true
        lastActivityDate = Date()
        startAutoLockTimer()
        return key
    }

    /// Restore the master key from raw bytes (biometric unlock path).
    func unlockWithKeyData(_ keyData: Data) {
        masterKey = SymmetricKey(data: keyData)
        isUnlocked = true
        lastActivityDate = Date()
        startAutoLockTimer()
    }

    /// Lock the application — wipe the in-memory key.
    func lock() {
        masterKey = nil
        isUnlocked = false
        stopAutoLockTimer()
    }

    /// Record user activity to reset the auto-lock countdown.
    func recordActivity() {
        lastActivityDate = Date()
    }

    // MARK: - Biometric Key Storage

    /// Store the current master key in Keychain protected by biometrics.
    func enableBiometricUnlock() throws {
        guard let key = masterKey else {
            throw KeyManagerError.noKeyLoaded
        }
        let keyData = key.withUnsafeBytes { Data($0) }
        try KeychainService.storeBiometricKey(keyData)
    }

    /// Attempt to retrieve the master key via biometric authentication.
    /// Returns `true` if successful.
    func attemptBiometricUnlock() async -> Bool {
        do {
            let keyData = try await KeychainService.retrieveBiometricKey()
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

    private func checkAutoLock() {
        guard isUnlocked else { return }
        let elapsed = Date().timeIntervalSince(lastActivityDate)
        if elapsed >= autoLockTimeout {
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

enum KeyManagerError: LocalizedError {
    case noKeyLoaded

    var errorDescription: String? {
        switch self {
        case .noKeyLoaded: return "No encryption key loaded"
        }
    }
}
