import CryptoKit
import Foundation
import Testing

@testable import Jottery

/// Covers `AppState.createVault(password:existingSalt:existingIterations:)`
/// (jottery-d9se): key derivation now runs off the main thread via
/// `Task.detached`, mirroring `unlock(password:)`. This pins the
/// create → lock → unlock round trip so that refactor can't silently change
/// what password unlocks a freshly created vault.
@MainActor
struct CreateVaultRoundTripTests {

    private func makeState() throws -> AppState {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-create-vault-roundtrip-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let state = AppState()
        try state.initialise(database: DatabaseManager(path: dir.appendingPathComponent("test.db").path))
        return state
    }

    /// A freshly created vault must be unlockable with the same password
    /// after a lock — the new-vault (envelope) path.
    @Test func createLockUnlockRoundTripSucceedsWithSamePassword() async throws {
        let state = try makeState()
        let password = "correct horse battery staple"

        try await state.createVault(password: password)
        #expect(state.isFirstLaunch == false)
        #expect(state.isLocked == false)

        // A note created before lock should still be readable after unlock,
        // proving the same master key round-trips through create → lock →
        // unlock rather than just the derived KDF output matching.
        let note = try #require(try state.createNote(content: "round trip note"))

        state.lock()
        #expect(state.isLocked == true)

        let unlocked = try await state.unlock(password: password)
        #expect(unlocked == true)
        #expect(state.isLocked == false)
        #expect(state.notes.first(where: { $0.id == note.id })?.content == "round trip note")
    }

    /// The wrong password must still be rejected after the derive moved
    /// off-main — the detached task must not swallow the mismatch.
    @Test func createLockUnlockRoundTripRejectsWrongPassword() async throws {
        let state = try makeState()
        try await state.createVault(password: "correct horse battery staple")
        state.lock()

        await #expect(throws: (any Error).self) {
            try await state.unlock(password: "definitely not it")
        }
        #expect(state.isLocked == true)
    }

    /// Legacy import path (`existingSalt`/`existingIterations` supplied)
    /// must also round-trip — this exercises the other branch of
    /// `createVault`, which derives the key directly rather than via the
    /// envelope wrap.
    @Test func createLockUnlockRoundTripSucceedsWithImportedSalt() async throws {
        let state = try makeState()
        let password = "imported vault password"
        let salt = CryptoService.generateSalt()

        try await state.createVault(
            password: password, existingSalt: salt, existingIterations: CryptoService.minIterations
        )
        #expect(state.isLocked == false)

        state.lock()
        let unlocked = try await state.unlock(password: password)
        #expect(unlocked == true)
        #expect(state.isLocked == false)
    }
}
