import Foundation
import LocalAuthentication

/// Proof that whoever is holding the device knows its passcode, or passes
/// Face ID or Touch ID.
enum DeviceOwner {

    /// True once they've proved it. A device with no passcode has nothing to
    /// check against, and its holder could do anything anyway.
    static func authenticate(reason: String) async -> Bool {
        await authenticate(reason: reason, with: LAContext())
    }

    /// The decision itself, apart from the system's prompt.
    static func authenticate(reason: String, with context: some DeviceOwnerContext) async -> Bool {
        var error: NSError?
        guard context.canEvaluatePolicy(.deviceOwnerAuthentication, error: &error) else {
            return (error as? LAError)?.code == .passcodeNotSet
        }
        return (try? await context.evaluatePolicy(.deviceOwnerAuthentication, localizedReason: reason)) ?? false
    }
}

/// What `DeviceOwner` needs from `LAContext`, which tests can't drive.
protocol DeviceOwnerContext {
    func canEvaluatePolicy(_ policy: LAPolicy, error: NSErrorPointer) -> Bool
    func evaluatePolicy(_ policy: LAPolicy, localizedReason: String) async throws -> Bool
}

extension LAContext: DeviceOwnerContext {}
