import CryptoKit
import CommonCrypto
import Foundation

/// Encrypted data wire format — JSON: {"ciphertext":"<base64>","iv":"<base64>"}
/// Base64 of ciphertext includes the GCM auth tag (last 16 bytes).
struct EncryptedData: Codable, Equatable {
    let ciphertext: String  // base64(ciphertext + tag)
    let iv: String          // base64(12-byte nonce)
}

/// AES-256-GCM encryption service, byte-for-byte compatible with the web and TUI clients.
///
/// Wire format matches `src/lib/services/crypto.ts` and `tui/src/crypto/service.rs`:
/// - Key derivation: PBKDF2-HMAC-SHA256
/// - Cipher: AES-256-GCM (CryptoKit)
/// - Nonce: 12 random bytes
/// - Ciphertext encoding: base64(ciphertext || 16-byte tag)
/// - JSON: `{"ciphertext":"...","iv":"..."}`
enum CryptoService {

    // MARK: - Constants

    static let keyLength = 32         // 256 bits
    static let nonceLength = 12       // 96 bits — GCM recommendation
    static let saltLength = 32        // 256 bits
    static let defaultIterations: UInt32 = 600_000
    static let minIterations: UInt32 = 100_000
    static let maxHashChainLength = 50

    // MARK: - Key Derivation

    /// Derive a 256-bit AES key from a password using PBKDF2-HMAC-SHA256.
    /// Uses CommonCrypto (`CCKeyDerivationPBKDF`) for compatibility.
    static func deriveKey(password: String, salt: Data, iterations: UInt32) -> SymmetricKey {
        let effectiveIterations = max(iterations, minIterations)
        let passwordData = Array(password.utf8)

        var derivedKey = Data(count: keyLength)
        _ = derivedKey.withUnsafeMutableBytes { derivedKeyPtr in
            salt.withUnsafeBytes { saltPtr in
                CCKeyDerivationPBKDF(
                    CCPBKDFAlgorithm(kCCPBKDF2),
                    passwordData,
                    passwordData.count,
                    saltPtr.bindMemory(to: UInt8.self).baseAddress!,
                    salt.count,
                    CCPseudoRandomAlgorithm(kCCPRFHmacAlgSHA256),
                    effectiveIterations,
                    derivedKeyPtr.bindMemory(to: UInt8.self).baseAddress!,
                    keyLength
                )
            }
        }
        return SymmetricKey(data: derivedKey)
    }

    // MARK: - Encrypt / Decrypt

    /// Encrypt plaintext to the standard wire format.
    static func encryptText(_ plaintext: String, key: SymmetricKey) throws -> EncryptedData {
        let data = Data(plaintext.utf8)
        return try encrypt(data, key: key)
    }

    /// Decrypt the standard wire format back to plaintext.
    static func decryptText(_ encrypted: EncryptedData, key: SymmetricKey) throws -> String {
        let data = try decrypt(encrypted, key: key)
        guard let text = String(data: data, encoding: .utf8) else {
            throw CryptoError.invalidUTF8
        }
        return text
    }

    /// Encrypt raw data.
    static func encrypt(_ data: Data, key: SymmetricKey) throws -> EncryptedData {
        let nonce = AES.GCM.Nonce()
        let sealed = try AES.GCM.seal(data, using: key, nonce: nonce)

        // CryptoKit puts ciphertext and tag in separate properties.
        // Wire format expects ciphertext || tag concatenated then base64-encoded.
        var combined = sealed.ciphertext
        combined.append(sealed.tag)

        return EncryptedData(
            ciphertext: combined.base64EncodedString(),
            iv: Data(nonce).base64EncodedString()
        )
    }

    /// Decrypt raw data.
    static func decrypt(_ encrypted: EncryptedData, key: SymmetricKey) throws -> Data {
        guard let combinedData = Data(base64Encoded: encrypted.ciphertext),
              let nonceData = Data(base64Encoded: encrypted.iv) else {
            throw CryptoError.invalidBase64
        }

        guard combinedData.count >= 16 else {
            throw CryptoError.ciphertextTooShort
        }

        let ciphertext = combinedData.prefix(combinedData.count - 16)
        let tag = combinedData.suffix(16)

        let nonce = try AES.GCM.Nonce(data: nonceData)
        let sealedBox = try AES.GCM.SealedBox(nonce: nonce, ciphertext: ciphertext, tag: tag)
        return try AES.GCM.open(sealedBox, using: key)
    }

    // MARK: - JSON Helpers

    /// Encrypt a Codable value as JSON.
    static func encryptJSON<T: Encodable>(_ value: T, key: SymmetricKey) throws -> EncryptedData {
        let data = try JSONEncoder().encode(value)
        guard let json = String(data: data, encoding: .utf8) else {
            throw CryptoError.encodingFailed
        }
        return try encryptText(json, key: key)
    }

    /// Decrypt JSON back to a Codable value.
    static func decryptJSON<T: Decodable>(_ encrypted: EncryptedData, key: SymmetricKey) throws -> T {
        let json = try decryptText(encrypted, key: key)
        guard let data = json.data(using: .utf8) else {
            throw CryptoError.invalidUTF8
        }
        return try JSONDecoder().decode(T.self, from: data)
    }

    /// Encrypt a string array (e.g. tags) — JSON-stringify then encrypt.
    static func encryptStringArray(_ strings: [String], key: SymmetricKey) throws -> EncryptedData {
        return try encryptJSON(strings, key: key)
    }

    /// Decrypt a string array.
    static func decryptStringArray(_ encrypted: EncryptedData, key: SymmetricKey) throws -> [String] {
        return try decryptJSON(encrypted, key: key)
    }

    // MARK: - Hashing

    /// SHA-256 hash, returned as standard base64.
    static func hash(_ data: String) -> String {
        let digest = SHA256.hash(data: Data(data.utf8))
        return Data(digest).base64EncodedString()
    }

    /// Compute content hash for conflict detection.
    /// Must match the web client's `computeContentHash` exactly.
    static func computeContentHash(for note: SyncableNoteFields) -> String {
        // Build deterministic JSON matching web client structure.
        // Use ordered keys via manual string building to match JSON.stringify output.
        var attachmentIds = note.attachmentIds
        attachmentIds.sort()

        let jsonParts: [(String, Any)] = [
            ("archived", note.archived),
            ("attachments", attachmentIds),
            ("content", note.content),
            ("locked", note.locked),
            ("pinned", note.pinned),
            ("showPreview", note.showPreview),
            ("syntaxLanguage", note.syntaxLanguage as Any),
            ("tags", note.tags),
            ("wordWrap", note.wordWrap),
        ]

        // JSON.stringify produces alphabetically ordered keys for simple objects.
        // We need to match that exactly.
        let json = buildDeterministicJSON(jsonParts)
        return hash(json)
    }

    // MARK: - Hash Chain

    /// Update hash chain by prepending new hash and trimming.
    static func updateHashChain(currentHash: String, parentChain: [String] = []) -> [String] {
        var newChain = [currentHash] + parentChain
        if newChain.count > maxHashChainLength {
            newChain = Array(newChain.prefix(maxHashChainLength))
        }
        return newChain
    }

    /// Find common ancestor between two hash chains.
    static func findCommonAncestor(chainA: [String], chainB: [String]) -> String? {
        let chainBSet = Set(chainB)
        for hash in chainA {
            if chainBSet.contains(hash) {
                return hash
            }
        }
        return nil
    }

    // MARK: - Random Generation

    /// Generate random salt (32 bytes).
    static func generateSalt() -> Data {
        var salt = Data(count: saltLength)
        salt.withUnsafeMutableBytes { ptr in
            _ = SecRandomCopyBytes(kSecRandomDefault, saltLength, ptr.baseAddress!)
        }
        return salt
    }

    /// Generate UUID v4 string.
    static func generateUUID() -> String {
        UUID().uuidString.lowercased()
    }

    // MARK: - Wire Format Helpers

    /// Parse encrypted data from the JSON string stored in the database.
    /// Format: `{"ciphertext":"...","iv":"..."}`
    static func parseEncryptedJSON(_ jsonString: String) throws -> EncryptedData {
        guard let data = jsonString.data(using: .utf8) else {
            throw CryptoError.invalidUTF8
        }
        return try JSONDecoder().decode(EncryptedData.self, from: data)
    }

    /// Serialize encrypted data to JSON string for storage.
    static func serializeEncryptedJSON(_ encrypted: EncryptedData) throws -> String {
        let data = try JSONEncoder().encode(encrypted)
        guard let json = String(data: data, encoding: .utf8) else {
            throw CryptoError.encodingFailed
        }
        return json
    }
}

// MARK: - Supporting Types

/// Fields needed for content hash computation.
struct SyncableNoteFields {
    let content: String         // Encrypted content string
    let tags: [String]          // Encrypted tags array
    let attachmentIds: [String]
    let pinned: Bool
    let archived: Bool
    let locked: Bool
    let syntaxLanguage: String?
    let wordWrap: Bool
    let showPreview: Bool
    let color: String?
}

enum CryptoError: LocalizedError {
    case invalidBase64
    case ciphertextTooShort
    case invalidUTF8
    case encodingFailed
    case decryptionFailed

    var errorDescription: String? {
        switch self {
        case .invalidBase64: return "Invalid base64 data"
        case .ciphertextTooShort: return "Ciphertext too short"
        case .invalidUTF8: return "Invalid UTF-8 data"
        case .encodingFailed: return "Encoding failed"
        case .decryptionFailed: return "Decryption failed. Invalid key or corrupted data."
        }
    }
}

// MARK: - Deterministic JSON Builder

/// Build a JSON string with keys in the exact order provided.
/// Matches `JSON.stringify` output for the content hash computation.
private func buildDeterministicJSON(_ pairs: [(String, Any)]) -> String {
    var parts: [String] = []
    for (key, value) in pairs {
        let encodedKey = jsonEncode(key)
        let encodedValue = jsonEncodeValue(value)
        parts.append("\(encodedKey):\(encodedValue)")
    }
    return "{\(parts.joined(separator: ","))}"
}

private func jsonEncode(_ string: String) -> String {
    let escaped = string
        .replacingOccurrences(of: "\\", with: "\\\\")
        .replacingOccurrences(of: "\"", with: "\\\"")
        .replacingOccurrences(of: "\n", with: "\\n")
        .replacingOccurrences(of: "\r", with: "\\r")
        .replacingOccurrences(of: "\t", with: "\\t")
    return "\"\(escaped)\""
}

private func jsonEncodeValue(_ value: Any) -> String {
    switch value {
    case let b as Bool:
        return b ? "true" : "false"
    case let s as String:
        return jsonEncode(s)
    case let arr as [String]:
        let items = arr.map { jsonEncode($0) }.joined(separator: ",")
        return "[\(items)]"
    case is NSNull:
        return "null"
    case let opt as String?:
        if let s = opt {
            return jsonEncode(s)
        }
        return "null"
    default:
        // For optional types that are nil
        if case Optional<Any>.none = value {
            return "null"
        }
        return jsonEncode(String(describing: value))
    }
}
