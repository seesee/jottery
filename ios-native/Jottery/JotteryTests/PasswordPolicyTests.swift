import Foundation
import Testing

@testable import Jottery

/// The notes password protects the key the server holds, so a new one has to
/// be long enough and can't be the password the server is given.
@MainActor
struct PasswordPolicyTests {

    @Test func aPasswordOfTheMinimumLengthIsAccepted() {
        #expect(PasswordPolicy.violation(newNotesPassword: "12345678") == nil)
    }

    @Test func aShorterPasswordIsRefused() {
        #expect(PasswordPolicy.violation(newNotesPassword: "1234567") == .tooShort)
        #expect(PasswordPolicy.violation(newNotesPassword: "") == .tooShort)
    }

    @Test func aPasswordMatchingTheAccountPasswordIsRefused() {
        #expect(PasswordPolicy.violation(
            newNotesPassword: "same-password", accountPassword: "same-password"
        ) == .matchesAccountPassword)
    }

    @Test func aDifferentAccountPasswordIsFine() {
        #expect(PasswordPolicy.violation(
            newNotesPassword: "notes-password", accountPassword: "account-password"
        ) == nil)
    }

    /// The fields are compared as the user sees them, so two spellings of one
    /// character still count as a match, though their bytes differ.
    @Test func canonicallyEquivalentPasswordsCountAsAMatch() {
        #expect(PasswordPolicy.matchesAccountPassword("caf\u{e9}-password", accountPassword: "cafe\u{301}-password"))
    }

    @Test func anEmptyAccountPasswordMatchesNothing() {
        #expect(!PasswordPolicy.matchesAccountPassword("", accountPassword: ""))
    }

    @Test func changingToAShortPasswordIsRefused() async throws {
        let state = try TestVaults.makeState()
        try await state.createVault(password: "original-password")

        await #expect(throws: AppStateError.passwordTooShort) {
            try await state.changePassword(currentPassword: "original-password", newPassword: "short")
        }

        #expect(await state.verifyVaultPassword("original-password"))
    }

    /// Existing short passwords have to keep working: only new ones are checked.
    @Test func aShortExistingPasswordStillUnlocks() async throws {
        let state = try TestVaults.makeState()
        try await state.createVault(password: "short")
        state.lock()

        #expect(try await state.unlock(password: "short"))
    }

    @Test func aShortExistingPasswordCanBeChangedToALongerOne() async throws {
        let state = try TestVaults.makeState()
        try await state.createVault(password: "short")

        try await state.changePassword(currentPassword: "short", newPassword: "a-longer-password")

        #expect(await state.verifyVaultPassword("a-longer-password"))
    }
}
