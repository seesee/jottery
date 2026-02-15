import Foundation
import Security
import LocalAuthentication

/// Keychain wrapper for storing API keys and biometric-protected encryption keys.
///
/// Two distinct items:
/// 1. **API key** — stored as a generic password, no biometric protection.
/// 2. **Biometric key** — master key bytes, protected by `.biometryCurrentSet`.
enum KeychainService {

    // MARK: - Service Identifiers

    private static let service = "com.jottery.ios"
    private static let apiKeyAccount = "sync-api-key"
    private static let biometricKeyAccount = "biometric-master-key"
    private static let clientIdAccount = "sync-client-id"

    // MARK: - API Key

    /// Store the sync API key.
    static func storeAPIKey(_ apiKey: String) throws {
        let data = Data(apiKey.utf8)
        try store(data: data, account: apiKeyAccount, biometric: false)
    }

    /// Retrieve the sync API key.
    static func retrieveAPIKey() -> String? {
        guard let data = retrieve(account: apiKeyAccount) else { return nil }
        return String(data: data, encoding: .utf8)
    }

    /// Delete the sync API key.
    static func deleteAPIKey() {
        delete(account: apiKeyAccount)
    }

    // MARK: - Client ID

    /// Store the sync client ID.
    static func storeClientId(_ clientId: String) throws {
        let data = Data(clientId.utf8)
        try store(data: data, account: clientIdAccount, biometric: false)
    }

    /// Retrieve the sync client ID.
    static func retrieveClientId() -> String? {
        guard let data = retrieve(account: clientIdAccount) else { return nil }
        return String(data: data, encoding: .utf8)
    }

    /// Delete the sync client ID.
    static func deleteClientId() {
        delete(account: clientIdAccount)
    }

    // MARK: - Biometric Master Key

    /// Store the master key bytes protected by biometrics.
    static func storeBiometricKey(_ keyData: Data) throws {
        // Delete existing item first
        deleteBiometricKey()

        var error: Unmanaged<CFError>?
        guard let access = SecAccessControlCreateWithFlags(
            kCFAllocatorDefault,
            kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly,
            .biometryCurrentSet,
            &error
        ) else {
            throw KeychainError.accessControlCreationFailed
        }

        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: biometricKeyAccount,
            kSecValueData as String: keyData,
            kSecAttrAccessControl as String: access,
        ]

        let status = SecItemAdd(query as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw KeychainError.storeFailed(status)
        }
    }

    /// Retrieve the biometric-protected master key. Triggers Face ID / Touch ID.
    static func retrieveBiometricKey() async throws -> Data {
        return try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                let context = LAContext()
                context.localizedReason = "Unlock Jottery"

                let query: [String: Any] = [
                    kSecClass as String: kSecClassGenericPassword,
                    kSecAttrService as String: service,
                    kSecAttrAccount as String: biometricKeyAccount,
                    kSecReturnData as String: true,
                    kSecUseAuthenticationContext as String: context,
                ]

                var result: AnyObject?
                let status = SecItemCopyMatching(query as CFDictionary, &result)

                if status == errSecSuccess, let data = result as? Data {
                    continuation.resume(returning: data)
                } else {
                    continuation.resume(throwing: KeychainError.retrieveFailed(status))
                }
            }
        }
    }

    /// Check if a biometric key exists.
    static func hasBiometricKey() -> Bool {
        let context = LAContext()
        context.interactionNotAllowed = true

        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: biometricKeyAccount,
            kSecUseAuthenticationContext as String: context,
        ]

        let status = SecItemCopyMatching(query as CFDictionary, nil)
        // errSecInteractionNotAllowed means the item exists but needs biometric
        return status == errSecSuccess || status == errSecInteractionNotAllowed
    }

    /// Delete the biometric key.
    static func deleteBiometricKey() {
        delete(account: biometricKeyAccount)
    }

    // MARK: - Private Helpers

    private static func store(data: Data, account: String, biometric: Bool) throws {
        // Delete existing first
        delete(account: account)

        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
        ]

        if biometric {
            var error: Unmanaged<CFError>?
            if let access = SecAccessControlCreateWithFlags(
                kCFAllocatorDefault,
                kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly,
                .biometryCurrentSet,
                &error
            ) {
                query[kSecAttrAccessControl as String] = access
                query.removeValue(forKey: kSecAttrAccessible as String)
            }
        }

        let status = SecItemAdd(query as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw KeychainError.storeFailed(status)
        }
    }

    private static func retrieve(account: String) -> Data? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]

        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status == errSecSuccess else { return nil }
        return result as? Data
    }

    private static func delete(account: String) {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
        SecItemDelete(query as CFDictionary)
    }
}

enum KeychainError: LocalizedError {
    case accessControlCreationFailed
    case storeFailed(OSStatus)
    case retrieveFailed(OSStatus)

    var errorDescription: String? {
        switch self {
        case .accessControlCreationFailed:
            return "Failed to create access control for Keychain"
        case .storeFailed(let status):
            return "Keychain store failed: \(status)"
        case .retrieveFailed(let status):
            return "Keychain retrieve failed: \(status)"
        }
    }
}
