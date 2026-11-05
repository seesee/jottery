import Foundation
import LocalAuthentication
import Testing

@testable import Jottery

/// Stands in for `LAContext`, which needs a person at the device.
private struct ScriptedContext: DeviceOwnerContext {
    /// Why the policy can't run, or nil if it can.
    var unavailable: LAError.Code?
    /// What the prompt ends in, or nil if it succeeds.
    var promptFailure: LAError.Code?

    func canEvaluatePolicy(_ policy: LAPolicy, error: NSErrorPointer) -> Bool {
        guard let unavailable else { return true }
        error?.pointee = NSError(domain: LAErrorDomain, code: unavailable.rawValue)
        return false
    }

    func evaluatePolicy(_ policy: LAPolicy, localizedReason: String) async throws -> Bool {
        if let promptFailure { throw LAError(promptFailure) }
        return true
    }
}

/// This is all that stands between a stolen, unlocked phone and wiping the vault
/// from the lock screen, so everything short of proof has to count as no.
struct DeviceOwnerTests {

    private func authenticate(_ context: ScriptedContext) async -> Bool {
        await DeviceOwner.authenticate(reason: "wipe", with: context)
    }

    @Test func passingThePromptIsProof() async {
        #expect(await authenticate(ScriptedContext()))
    }

    /// There's nothing to check against, and its holder could do anything anyway.
    @Test func aDeviceWithNoPasscodeHasNothingToProve() async {
        #expect(await authenticate(ScriptedContext(unavailable: .passcodeNotSet)))
    }

    @Test(arguments: [LAError.Code.biometryLockout, .biometryNotAvailable, .notInteractive])
    func aPolicyThatCantRunForAnyOtherReasonIsNoProof(reason: LAError.Code) async {
        #expect(await !authenticate(ScriptedContext(unavailable: reason)))
    }

    @Test(arguments: [LAError.Code.userCancel, .systemCancel, .appCancel, .authenticationFailed])
    func aPromptThatDoesntSucceedIsNoProof(failure: LAError.Code) async {
        #expect(await !authenticate(ScriptedContext(promptFailure: failure)))
    }
}
