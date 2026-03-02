import CryptoKit
import Foundation

/// Envelope encryption operations — migration, onboarding, and password change.
///
/// Envelope encryption decouples the master key from the password:
/// a random master key encrypts data, and the password only wraps/unwraps
/// the master key. This makes password changes O(1) instead of O(n).
enum EnvelopeService {

    // MARK: - Migrate Legacy → Envelope

    /// Attempt to migrate from legacy (direct PBKDF2) to envelope encryption.
    /// Non-fatal: failures are logged and the legacy path continues to work.
    @MainActor
    static func tryMigrateToEnvelope(
        appState: AppState,
        password: String,
        masterKey: SymmetricKey
    ) async {
        guard let syncRepo = appState.syncRepo,
              let encryptionRepo = appState.encryptionRepo,
              let syncClient = appState.syncClient else {
            print("[Envelope] Migration skipped — sync not configured")
            return
        }

        do {
            guard let syncMeta = try syncRepo.getMetadata(),
                  syncMeta.syncEnabled,
                  let userId = syncMeta.userId,
                  !userId.isEmpty else {
                print("[Envelope] Migration skipped — no userId in sync metadata")
                return
            }

            // Check if already migrated
            if let metadata = try encryptionRepo.get(), metadata.envelopeVersion != nil {
                print("[Envelope] Already migrated — skipping")
                return
            }

            let masterKeyData = masterKey.withUnsafeBytes { Data($0) }

            // Derive wrapping key from password + userId (1M iterations)
            let wrappingKey = CryptoService.deriveWrappingKey(password: password, userId: userId)

            // Wrap master key for server
            let serverWrapped = try CryptoService.wrapMasterKey(masterKeyData, with: wrappingKey)
            let serverBlob = try CryptoService.serializeEncryptedJSON(serverWrapped)

            // Upload to server
            let putRequest = PutWrappedKeyRequest(
                blob: serverBlob,
                kdfVersion: CryptoService.wrappingKdfVersion,
                kdfIterations: Int(CryptoService.wrappingIterations)
            )
            try await syncClient.putWrappedKey(putRequest)

            // Generate device salt and derive device key for local wrapping
            let deviceSalt = CryptoService.generateSalt()
            let deviceKey = CryptoService.deriveKey(
                password: password,
                salt: deviceSalt,
                iterations: CryptoService.defaultIterations
            )

            // Wrap master key locally with device key
            let localWrapped = try CryptoService.wrapMasterKey(masterKeyData, with: deviceKey)
            let localBlob = try CryptoService.serializeEncryptedJSON(localWrapped)

            // Create verification token with the master key
            let verificationEncrypted = try CryptoService.encryptText(
                EncryptionMetadata.verificationPlaintext, key: masterKey
            )
            let verificationJSON = try CryptoService.serializeEncryptedJSON(verificationEncrypted)

            // Save envelope metadata (clears legacy salt/iterations)
            try encryptionRepo.saveEnvelope(
                envelopeVersion: 1,
                deviceSalt: deviceSalt.base64EncodedString(),
                localWrappedMaster: localBlob,
                wrappingKdfVersion: CryptoService.wrappingKdfVersion
            )

            // Update verification token
            if var metadata = try encryptionRepo.get() {
                metadata.verification = verificationJSON
                try encryptionRepo.store(metadata)
            }

            print("[Envelope] Migration complete")
        } catch {
            print("[Envelope] Migration failed (non-fatal): \(error)")
        }
    }

    // MARK: - Onboard From Server

    /// Onboard a new device by downloading the wrapped master key from the server.
    /// Returns the unwrapped master key as a SymmetricKey.
    static func onboardFromServer(
        syncClient: SyncClient,
        encryptionRepo: EncryptionRepository,
        password: String,
        userId: String
    ) async throws -> SymmetricKey {
        // Fetch wrapped key from server
        guard let response = try await syncClient.getWrappedKey() else {
            throw EnvelopeError.noWrappedKeyOnServer
        }

        // Derive wrapping key from password + userId
        let wrappingKey = CryptoService.deriveWrappingKey(password: password, userId: userId)

        // Unwrap master key
        let serverWrapped = try CryptoService.parseEncryptedJSON(response.blob)
        let masterKeyData = try CryptoService.unwrapMasterKey(serverWrapped, with: wrappingKey)

        guard masterKeyData.count == CryptoService.keyLength else {
            throw EnvelopeError.invalidMasterKeyLength
        }

        let masterKey = SymmetricKey(data: masterKeyData)

        // Generate device salt for local storage
        let deviceSalt = CryptoService.generateSalt()
        let deviceKey = CryptoService.deriveKey(
            password: password,
            salt: deviceSalt,
            iterations: CryptoService.defaultIterations
        )

        // Wrap master key locally
        let localWrapped = try CryptoService.wrapMasterKey(masterKeyData, with: deviceKey)
        let localBlob = try CryptoService.serializeEncryptedJSON(localWrapped)

        // Create verification token
        let verificationEncrypted = try CryptoService.encryptText(
            EncryptionMetadata.verificationPlaintext, key: masterKey
        )
        let verificationJSON = try CryptoService.serializeEncryptedJSON(verificationEncrypted)

        // Save envelope metadata
        try encryptionRepo.saveEnvelope(
            envelopeVersion: 1,
            deviceSalt: deviceSalt.base64EncodedString(),
            localWrappedMaster: localBlob,
            wrappingKdfVersion: CryptoService.wrappingKdfVersion
        )

        // Update verification token
        if var metadata = try encryptionRepo.get() {
            metadata.verification = verificationJSON
            try encryptionRepo.store(metadata)
        }

        return masterKey
    }

    // MARK: - Fast Password Change

    /// Change password with envelope encryption — re-wraps only, no note re-encryption.
    @MainActor
    static func changePasswordEnvelope(
        appState: AppState,
        newPassword: String,
        masterKeyData: Data
    ) async throws {
        guard let encryptionRepo = appState.encryptionRepo else {
            throw EnvelopeError.notInitialised
        }

        // Generate new device salt and derive new device key
        let newDeviceSalt = CryptoService.generateSalt()
        let newDeviceKey = CryptoService.deriveKey(
            password: newPassword,
            salt: newDeviceSalt,
            iterations: CryptoService.defaultIterations
        )

        // Re-wrap master key locally
        let localWrapped = try CryptoService.wrapMasterKey(masterKeyData, with: newDeviceKey)
        let localBlob = try CryptoService.serializeEncryptedJSON(localWrapped)

        // Update verification token with the master key
        let masterKey = SymmetricKey(data: masterKeyData)
        let verificationEncrypted = try CryptoService.encryptText(
            EncryptionMetadata.verificationPlaintext, key: masterKey
        )
        let verificationJSON = try CryptoService.serializeEncryptedJSON(verificationEncrypted)

        // Save updated envelope metadata
        try encryptionRepo.saveEnvelope(
            envelopeVersion: 1,
            deviceSalt: newDeviceSalt.base64EncodedString(),
            localWrappedMaster: localBlob,
            wrappingKdfVersion: CryptoService.wrappingKdfVersion
        )

        // Update verification token
        if var metadata = try encryptionRepo.get() {
            metadata.verification = verificationJSON
            try encryptionRepo.store(metadata)
        }

        // If sync configured, re-wrap for server too
        if let syncRepo = appState.syncRepo,
           let syncClient = appState.syncClient,
           let syncMeta = try? syncRepo.getMetadata(),
           syncMeta.syncEnabled,
           let userId = syncMeta.userId, !userId.isEmpty {

            let newWrappingKey = CryptoService.deriveWrappingKey(password: newPassword, userId: userId)
            let serverWrapped = try CryptoService.wrapMasterKey(masterKeyData, with: newWrappingKey)
            let serverBlob = try CryptoService.serializeEncryptedJSON(serverWrapped)

            let putRequest = PutWrappedKeyRequest(
                blob: serverBlob,
                kdfVersion: CryptoService.wrappingKdfVersion,
                kdfIterations: Int(CryptoService.wrappingIterations)
            )
            try await syncClient.putWrappedKey(putRequest)
        }
    }
}

// MARK: - Errors

enum EnvelopeError: LocalizedError {
    case noWrappedKeyOnServer
    case invalidMasterKeyLength
    case notInitialised

    var errorDescription: String? {
        switch self {
        case .noWrappedKeyOnServer:
            return "No wrapped key found on server — the source device may not have migrated yet"
        case .invalidMasterKeyLength:
            return "Invalid master key length after unwrapping"
        case .notInitialised:
            return "Encryption repository not initialised"
        }
    }
}
