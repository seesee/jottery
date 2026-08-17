import CryptoKit
import Foundation
import Testing

@testable import Jottery

/// Covers `AppState.verifyVaultPassword(_:)` — the seam `SyncSetupView` uses
/// to confirm the "Notes password" field is correct before it's used to wrap
/// the envelope master key. Wrapping with the wrong secret silently breaks
/// onboarding for every other device (jottery-md6b), so this must reject a
/// wrong password and accept the right one, without mutating app state.
@MainActor
struct VaultPasswordVerifyTests {

    private func makeState() throws -> AppState {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-vault-verify-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let state = AppState()
        try state.initialise(database: DatabaseManager(path: dir.appendingPathComponent("test.db").path))
        return state
    }

    @Test func acceptsCorrectVaultPassword() async throws {
        let state = try makeState()
        try await state.createVault(password: "correct-horse-battery-staple")

        let result = await state.verifyVaultPassword("correct-horse-battery-staple")
        #expect(result == true)
    }

    @Test func rejectsWrongVaultPassword() async throws {
        let state = try makeState()
        try await state.createVault(password: "correct-horse-battery-staple")

        let result = await state.verifyVaultPassword("definitely-not-it")
        #expect(result == false)
    }

    @Test func rejectsEmptyStringAgainstRealVault() async throws {
        let state = try makeState()
        try await state.createVault(password: "correct-horse-battery-staple")

        let result = await state.verifyVaultPassword("")
        #expect(result == false)
    }

    @Test func returnsFalseWhenNoVaultExists() async throws {
        let state = try makeState()

        let result = await state.verifyVaultPassword("anything")
        #expect(result == false)
    }
}
