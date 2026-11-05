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

    /// Save envelope encryption metadata, clearing legacy salt/iterations.
    ///
    /// The wrapped key and its verification token go together. A wrapped key
    /// saved without its token, by being killed between two writes, leaves a
    /// vault that says wrong password for ever.
    func saveEnvelope(envelopeVersion: Int, deviceSalt: String,
                      localWrappedMaster: String, wrappingKdfVersion: Int, verification: String) throws {
        try db.dbPool.write { db in
            try db.execute(sql: """
                UPDATE encryption_metadata
                SET envelope_version = ?, device_salt = ?,
                    local_wrapped_master = ?, wrapping_kdf_version = ?,
                    salt = '', iterations = 0, verification = ?
                WHERE id = 1
            """, arguments: [envelopeVersion, deviceSalt, localWrappedMaster, wrappingKdfVersion, verification])
        }
    }
}
