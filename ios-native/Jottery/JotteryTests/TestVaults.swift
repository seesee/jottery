import CryptoKit
import Foundation
import GRDB
import Testing

@testable import Jottery

/// Shared helpers for tests that inspect or tamper with a vault's raw tables.
@MainActor
enum TestVaults {

    /// A fresh, uninitialised-vault AppState on a temporary database.
    static func makeState() throws -> AppState {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-test-vault-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let state = AppState()
        try state.initialise(database: DatabaseManager(path: dir.appendingPathComponent("test.db").path))
        return state
    }

    /// An unlocked vault in the pre-envelope ("legacy") format: the master key
    /// is derived straight from the password and salt. A low iteration count
    /// keeps the tests fast; legacy unlock uses whatever count is stored.
    static func makeLegacyVault(password: String, iterations: UInt32 = 1_000) async throws -> AppState {
        let state = try makeState()
        let salt = CryptoService.generateSalt()
        let key = try CryptoService.deriveKey(password: password, salt: salt, iterations: iterations)
        let verification = try CryptoService.serializeEncryptedJSON(
            CryptoService.encryptText(EncryptionMetadata.verificationPlaintext, key: key)
        )
        try #require(state.encryptionRepo).store(EncryptionMetadata(
            salt: salt.base64EncodedString(),
            iterations: Int(iterations),
            createdAt: ISO8601DateFormatter().string(from: Date()),
            algorithm: "AES-256-GCM",
            verification: verification
        ))
        try await state.unlock(password: password)
        #expect(try state.encryptionRepo?.get()?.envelopeVersion == nil)
        return state
    }

    /// Ciphertext under a random key no vault holds.
    static func foreignCiphertext() throws -> String {
        try CryptoService.serializeEncryptedJSON(
            CryptoService.encryptText("under another key", key: SymmetricKey(size: .bits256))
        )
    }

    /// A raw SQL write, bypassing the repositories. Synchronous, so GRDB's
    /// blocking `write` is used rather than its async overload.
    static func execute(_ state: AppState, _ sql: String, _ arguments: StatementArguments) throws {
        try #require(state.db).dbPool.write { db in
            try db.execute(sql: sql, arguments: arguments)
        }
    }

    /// Every row of every table a re-key touches, for before/after comparison.
    static func snapshot(_ state: AppState) throws -> [String: [Row]] {
        try #require(state.db).dbPool.read { db in
            var tables: [String: [Row]] = [:]
            for table in ["notes", "note_versions", "attachments", "saved_searches", "encryption_metadata"] {
                tables[table] = try Row.fetchAll(db, sql: "SELECT * FROM \(table) ORDER BY rowid")
            }
            return tables
        }
    }

    static func keyData(_ state: AppState) -> Data? {
        state.keyManager.masterKey?.withUnsafeBytes { Data($0) }
    }
}
