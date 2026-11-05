import Foundation

/// The rules a notes password has to meet.
///
/// The notes password wraps the master key, and the server holds that wrapped
/// key. The account password reaches the server in plaintext at registration
/// and login, so if the two matched the server could unwrap the key and read
/// every note. The comparison is made here, on the device.
enum PasswordPolicy {

    static let minimumNotesPasswordLength = 8

    enum Violation: Equatable {
        case tooShort
        case matchesAccountPassword
    }

    /// True when `notesPassword` is one the server has been given.
    static func matchesAccountPassword(_ notesPassword: String, accountPassword: String) -> Bool {
        !accountPassword.isEmpty && notesPassword == accountPassword
    }

    /// The first rule a password being chosen now breaks, if any. `accountPassword`
    /// is nil where the app doesn't know it (it is never stored).
    ///
    /// Only for a new password: a new vault or a password change. Never use it
    /// at unlock, or when joining an account that already has a key, because
    /// an existing password has to keep working, even a short one. One that
    /// matches the account password is warned about instead: refusing it would
    /// stop those users adding a device.
    static func violation(newNotesPassword: String, accountPassword: String? = nil) -> Violation? {
        if let accountPassword, matchesAccountPassword(newNotesPassword, accountPassword: accountPassword) {
            return .matchesAccountPassword
        }
        return newNotesPassword.count < minimumNotesPasswordLength ? .tooShort : nil
    }
}
