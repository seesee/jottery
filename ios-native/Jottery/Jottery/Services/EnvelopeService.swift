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
        userId: String,
        localPassword: String? = nil
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
            password: localPassword ?? password,
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

    // MARK: - Envelope Setup (Register / Unlock)

    /// Set up envelope encryption after device registration or on unlock.
    /// Checks the server for an existing wrapped key first (another device may
    /// have already uploaded one). If found, onboards from the server key and
    /// re-encrypts local data if the master key differs. If no server key exists,
    /// falls through to `tryMigrateToEnvelope` to upload ours.
    @MainActor
    static func tryEnvelopeSetup(
        appState: AppState,
        password: String,
        masterKey: SymmetricKey
    ) async {
        guard let syncRepo = appState.syncRepo,
              let encryptionRepo = appState.encryptionRepo,
              let syncClient = appState.syncClient else {
            print("[Envelope] Setup skipped — sync not configured")
            return
        }

        do {
            guard let syncMeta = try syncRepo.getMetadata(),
                  syncMeta.syncEnabled,
                  let userId = syncMeta.userId,
                  !userId.isEmpty else {
                print("[Envelope] Setup skipped — no userId in sync metadata")
                return
            }

            // Already migrated — nothing to do
            if let metadata = try encryptionRepo.get(), metadata.envelopeVersion != nil {
                print("[Envelope] Already migrated — skipping setup")
                return
            }

            // Check server for an existing wrapped key from another device
            let serverResponse = try? await syncClient.getWrappedKey()

            if serverResponse != nil {
                // Another device already uploaded — onboard from server
                print("[Envelope] Server has wrapped key — onboarding from server")
                let serverKey = try await onboardFromServer(
                    syncClient: syncClient,
                    encryptionRepo: encryptionRepo,
                    password: password,
                    userId: userId
                )

                // If the server key differs from our local key, re-encrypt all data
                let localKeyData = masterKey.withUnsafeBytes { Data($0) }
                let serverKeyData = serverKey.withUnsafeBytes { Data($0) }
                if localKeyData != serverKeyData {
                    print("[Envelope] Server key differs from local — re-encrypting data")
                    try appState.reEncryptAllData(from: masterKey, to: serverKey)
                    appState.keyManager.unlockWithKeyData(serverKeyData)
                    try appState.loadNotes()
                }
            } else {
                // No server key — we're the first device, upload ours
                print("[Envelope] No server key — migrating local key to envelope")
                await tryMigrateToEnvelope(
                    appState: appState,
                    password: password,
                    masterKey: masterKey
                )
            }
        } catch {
            print("[Envelope] Setup failed (non-fatal): \(error)")
        }
    }

    // MARK: - Fast Password Change

    /// Local re-wrap only — new device salt, re-wrap master key, save to DB.
    /// Synchronous (throws on failure) so the caller knows immediately if it failed.
    static func changePasswordLocal(
        encryptionRepo: EncryptionRepository,
        newPassword: String,
        masterKeyData: Data
    ) throws {
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

        print("[Envelope] Local password re-wrap complete")
    }

    /// Server re-wrap — upload new wrapped key. Non-fatal; server will be
    /// updated on next unlock/sync if this fails.
    static func changePasswordServer(
        syncRepo: SyncRepository,
        syncClient: SyncClient,
        newPassword: String,
        masterKeyData: Data
    ) async {
        do {
            guard let syncMeta = try syncRepo.getMetadata(),
                  syncMeta.syncEnabled,
                  let userId = syncMeta.userId, !userId.isEmpty else {
                print("[Envelope] Server re-wrap skipped — no userId")
                return
            }

            let newWrappingKey = CryptoService.deriveWrappingKey(password: newPassword, userId: userId)
            let serverWrapped = try CryptoService.wrapMasterKey(masterKeyData, with: newWrappingKey)
            let serverBlob = try CryptoService.serializeEncryptedJSON(serverWrapped)

            let putRequest = PutWrappedKeyRequest(
                blob: serverBlob,
                kdfVersion: CryptoService.wrappingKdfVersion,
                kdfIterations: Int(CryptoService.wrappingIterations)
            )
            try await syncClient.putWrappedKey(putRequest)
            print("[Envelope] Server re-wrap complete")
        } catch {
            print("[Envelope] Server re-wrap failed (non-fatal): \(error)")
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
