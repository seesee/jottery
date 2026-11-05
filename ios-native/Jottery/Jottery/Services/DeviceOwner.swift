import Foundation
import LocalAuthentication

/// Proof that whoever is holding the device knows its passcode, or passes
/// Face ID or Touch ID.
enum DeviceOwner {

    /// True once they've proved it. A device with no passcode has nothing to
    /// check against, and its holder could do anything anyway.
    static func authenticate(reason: String) async -> Bool {
        let context = LAContext()
        var error: NSError?
        guard context.canEvaluatePolicy(.deviceOwnerAuthentication, error: &error) else {
            return (error as? LAError)?.code == .passcodeNotSet
        }
        return (try? await context.evaluatePolicy(.deviceOwnerAuthentication, localizedReason: reason)) ?? false
    }
}
