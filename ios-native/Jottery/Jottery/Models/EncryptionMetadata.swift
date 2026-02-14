import Foundation
import GRDB

/// Encryption setup metadata — stored once in the `encryption_metadata` table.
struct EncryptionMetadata: Codable, FetchableRecord, PersistableRecord {
    static let databaseTableName = "encryption_metadata"

    var id: Int = 1
    var salt: String           // Base64-encoded 32-byte salt
    var iterations: Int
    var createdAt: String      // ISO 8601
    var algorithm: String      // "AES-256-GCM"
    var verification: String?  // Encrypted known plaintext for password verification

    enum CodingKeys: String, CodingKey {
        case id
        case salt
        case iterations
        case createdAt = "created_at"
        case algorithm
        case verification
    }

    /// The known plaintext used for password verification.
    static let verificationPlaintext = "jottery-vault-ok"

    /// Salt as raw bytes.
    var saltData: Data? {
        Data(base64Encoded: salt)
    }

    /// Create metadata for a new vault.
    static func new(salt: Data, iterations: UInt32, verification: String) -> EncryptionMetadata {
        EncryptionMetadata(
            salt: salt.base64EncodedString(),
            iterations: Int(iterations),
            createdAt: ISO8601DateFormatter.jottery.string(from: Date()),
            algorithm: "AES-256-GCM",
            verification: verification
        )
    }
}
