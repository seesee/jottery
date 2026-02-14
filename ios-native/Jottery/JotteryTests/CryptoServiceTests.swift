import CryptoKit
import Foundation
import Testing

@testable import Jottery

struct CryptoServiceTests {

    // MARK: - Key Derivation

    @Test func keyDerivationDeterministic() {
        let salt = CryptoService.generateSalt()
        let key1 = CryptoService.deriveKey(password: "test-password", salt: salt, iterations: 100_000)
        let key2 = CryptoService.deriveKey(password: "test-password", salt: salt, iterations: 100_000)

        // Same password + salt → same key
        let data1 = key1.withUnsafeBytes { Data($0) }
        let data2 = key2.withUnsafeBytes { Data($0) }
        #expect(data1 == data2)
    }

    @Test func keyDerivationDifferentPasswords() {
        let salt = CryptoService.generateSalt()
        let key1 = CryptoService.deriveKey(password: "password-a", salt: salt, iterations: 100_000)
        let key2 = CryptoService.deriveKey(password: "password-b", salt: salt, iterations: 100_000)

        let data1 = key1.withUnsafeBytes { Data($0) }
        let data2 = key2.withUnsafeBytes { Data($0) }
        #expect(data1 != data2)
    }

    @Test func keyDerivationDifferentSalts() {
        let salt1 = CryptoService.generateSalt()
        let salt2 = CryptoService.generateSalt()
        let key1 = CryptoService.deriveKey(password: "same-password", salt: salt1, iterations: 100_000)
        let key2 = CryptoService.deriveKey(password: "same-password", salt: salt2, iterations: 100_000)

        let data1 = key1.withUnsafeBytes { Data($0) }
        let data2 = key2.withUnsafeBytes { Data($0) }
        #expect(data1 != data2)
    }

    // MARK: - Encrypt / Decrypt Text

    @Test func textRoundTrip() throws {
        let key = testKey()
        let plaintext = "Hello, World! This is a test message."

        let encrypted = try CryptoService.encryptText(plaintext, key: key)
        let decrypted = try CryptoService.decryptText(encrypted, key: key)

        #expect(decrypted == plaintext)
    }

    @Test func emptyStringRoundTrip() throws {
        let key = testKey()
        let encrypted = try CryptoService.encryptText("", key: key)
        let decrypted = try CryptoService.decryptText(encrypted, key: key)
        #expect(decrypted == "")
    }

    @Test func unicodeRoundTrip() throws {
        let key = testKey()
        let plaintext = "日本語テスト 🎉 Ñoño"
        let encrypted = try CryptoService.encryptText(plaintext, key: key)
        let decrypted = try CryptoService.decryptText(encrypted, key: key)
        #expect(decrypted == plaintext)
    }

    @Test func decryptWithWrongKeyFails() throws {
        let key1 = CryptoService.deriveKey(password: "key-one", salt: CryptoService.generateSalt(), iterations: 100_000)
        let key2 = CryptoService.deriveKey(password: "key-two", salt: CryptoService.generateSalt(), iterations: 100_000)

        let encrypted = try CryptoService.encryptText("secret", key: key1)

        #expect(throws: (any Error).self) {
            try CryptoService.decryptText(encrypted, key: key2)
        }
    }

    // MARK: - Wire Format

    @Test func encryptedDataFormat() throws {
        let key = testKey()
        let encrypted = try CryptoService.encryptText("test", key: key)

        // Ciphertext should be valid base64
        #expect(Data(base64Encoded: encrypted.ciphertext) != nil)
        // IV should be valid base64 and decode to 12 bytes
        let ivData = Data(base64Encoded: encrypted.iv)
        #expect(ivData != nil)
        #expect(ivData?.count == 12)

        // Ciphertext (decoded) should be: plaintext_len + 16 (tag)
        let ciphertextData = Data(base64Encoded: encrypted.ciphertext)!
        #expect(ciphertextData.count == 4 + 16)  // "test" is 4 bytes + 16 byte tag
    }

    @Test func jsonSerialisation() throws {
        let key = testKey()
        let encrypted = try CryptoService.encryptText("test", key: key)

        let json = try CryptoService.serializeEncryptedJSON(encrypted)
        #expect(json.contains("ciphertext"))
        #expect(json.contains("iv"))

        let parsed = try CryptoService.parseEncryptedJSON(json)
        #expect(parsed == encrypted)

        // Full round-trip through JSON
        let decrypted = try CryptoService.decryptText(parsed, key: key)
        #expect(decrypted == "test")
    }

    // MARK: - JSON Helpers

    @Test func jsonRoundTrip() throws {
        let key = testKey()
        let data = ["tag1", "tag2", "tag3"]

        let encrypted = try CryptoService.encryptStringArray(data, key: key)
        let decrypted = try CryptoService.decryptStringArray(encrypted, key: key)

        #expect(decrypted == data)
    }

    // MARK: - Hashing

    @Test func hashDeterministic() {
        let hash1 = CryptoService.hash("test data")
        let hash2 = CryptoService.hash("test data")
        #expect(hash1 == hash2)

        let hash3 = CryptoService.hash("different")
        #expect(hash1 != hash3)
    }

    @Test func hashIsBase64() {
        let hash = CryptoService.hash("test")
        #expect(Data(base64Encoded: hash) != nil)
        // SHA-256 produces 32 bytes, which is 44 chars in base64
        #expect(Data(base64Encoded: hash)!.count == 32)
    }

    // MARK: - Hash Chain

    @Test func hashChainEmptyParent() {
        let chain = CryptoService.updateHashChain(currentHash: "abc", parentChain: [])
        #expect(chain == ["abc"])
    }

    @Test func hashChainPrepends() {
        let chain = CryptoService.updateHashChain(
            currentHash: "new",
            parentChain: ["old1", "old2"]
        )
        #expect(chain == ["new", "old1", "old2"])
    }

    @Test func hashChainTruncates() {
        let parent = (0..<50).map { "hash_\($0)" }
        let chain = CryptoService.updateHashChain(currentHash: "new", parentChain: parent)
        #expect(chain.count == 50)
        #expect(chain[0] == "new")
        #expect(chain[49] == "hash_48")
    }

    @Test func findCommonAncestor() {
        let chainA = ["a1", "common", "old"]
        let chainB = ["b1", "b2", "common"]
        #expect(CryptoService.findCommonAncestor(chainA: chainA, chainB: chainB) == "common")
    }

    @Test func findCommonAncestorNone() {
        let chainA = ["a1", "a2"]
        let chainB = ["b1", "b2"]
        #expect(CryptoService.findCommonAncestor(chainA: chainA, chainB: chainB) == nil)
    }

    // MARK: - Salt Generation

    @Test func saltLength() {
        let salt = CryptoService.generateSalt()
        #expect(salt.count == 32)
    }

    @Test func saltsUnique() {
        let salt1 = CryptoService.generateSalt()
        let salt2 = CryptoService.generateSalt()
        #expect(salt1 != salt2)
    }

    // MARK: - UUID

    @Test func uuidFormat() {
        let uuid = CryptoService.generateUUID()
        #expect(uuid.count == 36) // Standard UUID format with hyphens
        #expect(uuid == uuid.lowercased()) // Should be lowercase
    }

    // MARK: - Helpers

    private func testKey() -> SymmetricKey {
        let salt = Data(repeating: 0xAB, count: 32)
        return CryptoService.deriveKey(password: "test-password", salt: salt, iterations: 100_000)
    }
}
