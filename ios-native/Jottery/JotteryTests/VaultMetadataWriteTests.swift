import CryptoKit
import Foundation
import GRDB
import Testing

@testable import Jottery

/// A vault's wrapped key and its verification token have to be written together.
/// A kill between the two leaves a key the token doesn't match, and an unlock that
/// says wrong password for ever, with no way to tell (jottery-chtn).
@MainActor
struct VaultMetadataWriteTests {

    private let password = "notes-password-correct-horse"

    /// Stands in for being killed after the first write: nothing can follow it.
    private func stopAfterTheFirstWrite(_ state: AppState) async throws {
        let db = try #require(state.encryptionRepo).db
        try await db.dbPool.write { db in
            try db.execute(sql: """
                CREATE TRIGGER killed BEFORE UPDATE ON encryption_metadata
                BEGIN SELECT RAISE(ABORT, 'killed'); END
            """)
        }
    }

    @Test func aNewVaultIsWrittenInOneGo() async throws {
        let state = try TestVaults.makeState()
        try await stopAfterTheFirstWrite(state)

        try await state.createVault(password: password)

        let metadata = try #require(try state.encryptionRepo?.get())
        #expect(metadata.envelopeVersion == 1)
        #expect(metadata.localWrappedMaster != nil)
        #expect(metadata.verification != nil)
    }

    @Test func aNewVaultUnlocksAfterBeingWrittenInOneGo() async throws {
        let state = try TestVaults.makeState()
        try await state.createVault(password: password)
        state.lock()

        try await state.unlock(password: password)

        #expect(!state.isLocked)
    }

    @Test func savingAnEnvelopeSavesItsTokenWithIt() async throws {
        let state = try TestVaults.makeState()
        try await state.createVault(password: password)
        let repo = try #require(state.encryptionRepo)

        try repo.saveEnvelope(
            envelopeVersion: 1, deviceSalt: "salt", localWrappedMaster: "wrapped",
            wrappingKdfVersion: 1, verification: "the-new-token"
        )

        let metadata = try #require(try repo.get())
        #expect(metadata.localWrappedMaster == "wrapped")
        #expect(metadata.verification == "the-new-token")
    }
}
