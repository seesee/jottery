import Foundation
import GRDB

/// Reads and writes encryption metadata (salt, iterations, algorithm).
struct EncryptionRepository: Sendable {

    let db: DatabaseManager

    /// Fetch the encryption metadata (returns nil if vault not set up).
    func get() throws -> EncryptionMetadata? {
        try db.dbPool.read { db in
            try EncryptionMetadata.fetchOne(db, key: 1)
        }
    }

    /// Store encryption metadata for a new vault.
    func store(_ metadata: EncryptionMetadata) throws {
        try db.dbPool.write { db in
            try metadata.save(db)
        }
    }

    /// Check if a vault has been set up.
    func isVaultSetUp() throws -> Bool {
        try get() != nil
    }
}
