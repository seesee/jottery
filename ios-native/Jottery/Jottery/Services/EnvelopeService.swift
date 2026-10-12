import CryptoKit
import Foundation

/// Envelope encryption operations — migration, onboarding, and password change.
///
/// Envelope encryption decouples the master key from the password:
/// a random master key encrypts data, and the password only wraps/unwraps
/// the master key. This makes password changes O(1) instead of O(n).
enum EnvelopeService {

    // MARK: - Publish Wrapped Key

    /// Wrap `masterKeyData` with a key derived from `password` + `userId` and
    /// store it on the server so other devices can onboard.
    ///
    /// `password` must be the notes (vault) password. The server sees the
    /// account password in plaintext at registration and login, so a blob
    /// wrapped with it could be opened by the server operator; the notes
    /// password never leaves the device.
    static func uploadWrappedKey(
        syncClient: SyncClient,
        userId: String,
        password: String,
        masterKeyData: Data
    ) async throws {
        // Derive wrapping key from password + userId (1M iterations)
        let wrappingKey = CryptoService.deriveWrappingKey(password: password, userId: userId)

        let serverWrapped = try CryptoService.wrapMasterKey(masterKeyData, with: wrappingKey)
        let serverBlob = try CryptoService.serializeEncryptedJSON(serverWrapped)

        let putRequest = PutWrappedKeyRequest(
            blob: serverBlob,
            kdfVersion: CryptoService.wrappingKdfVersion,
            kdfIterations: Int(CryptoService.wrappingIterations)
        )
        try await syncClient.putWrappedKey(putRequest)
    }

    /// Upload the wrapped master key only if the server has none yet.
    /// An existing server copy is never overwritten. Returns `true` if a key
    /// was uploaded. Throws if the server cannot be reached or rejects the upload.
    ///
    /// New vaults are in envelope format from the start, so nothing else ever
    /// publishes their key: without this a first device leaves the server with
    /// no key and every other device fails with "no wrapped key found".
    @discardableResult
    static func publishWrappedKeyIfMissing(
        syncClient: SyncClient,
        userId: String,
        password: String,
        masterKeyData: Data
    ) async throws -> Bool {
        if try await syncClient.getWrappedKey() != nil {
            return false
        }
        try await uploadWrappedKey(
            syncClient: syncClient,
            userId: userId,
            password: password,
            masterKeyData: masterKeyData
        )
        return true
    }

    // MARK: - Migrate Legacy → Envelope

    /// Attempt to migrate from legacy (direct PBKDF2) to envelope encryption.
    /// Non-fatal: failures are logged and the legacy path continues to work.
    /// Returns `true` on success or when there was nothing to do, `false`
    /// only when the attempt itself failed — callers that need to surface
    /// the failure to a user (e.g. `SyncSetupView`) can check the result;
    /// background/migration callers may discard it and keep the existing
    /// silent-retry-on-next-unlock behaviour.
    @discardableResult
    @MainActor
    static func tryMigrateToEnvelope(
        appState: AppState,
        password: String,
        masterKey: SymmetricKey
    ) async -> Bool {
        // Whether sync is on lives in `appState.settings`. The `syncEnabled`
        // column of the sync metadata record is never written (it stays false),
        // so it must not be used as a gate: doing so made every envelope upload
        // skip silently and left the server without a wrapped key.
        guard appState.settings.syncEnabled,
              let syncRepo = appState.syncRepo,
              let encryptionRepo = appState.encryptionRepo,
              let syncClient = appState.syncClient else {
            Log.debug("[Envelope] Migration skipped — sync not configured")
            return true
        }

        do {
            guard let syncMeta = try syncRepo.getMetadata(),
                  let userId = syncMeta.userId,
                  !userId.isEmpty else {
                Log.debug("[Envelope] Migration skipped — no userId in sync metadata")
                return true
            }

            // Check if already migrated
            if let metadata = try encryptionRepo.get(), metadata.envelopeVersion != nil {
                Log.debug("[Envelope] Already migrated — skipping")
                return true
            }

            let masterKeyData = masterKey.withUnsafeBytes { Data($0) }

            // Wrap master key for the server with the notes password and upload it
            try await uploadWrappedKey(
                syncClient: syncClient,
                userId: userId,
                password: password,
                masterKeyData: masterKeyData
            )

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

            Log.debug("[Envelope] Migration complete")
            return true
        } catch {
            Log.debug("[Envelope] Migration failed (non-fatal): \(error)")
            return false
        }
    }

    // MARK: - Onboard From Server

    /// Onboard a new device by downloading the wrapped master key from the server.
    /// Returns the unwrapped master key as a SymmetricKey.
    ///
    /// The blob is wrapped with the notes password, so `password` should be
    /// that. Builds before this fix could wrap it with the account password,
    /// so `alternatePassword` (the account password) is tried if the first
    /// attempt fails to unwrap. Nothing is written locally unless one works.
    static func onboardFromServer(
        syncClient: SyncClient,
        encryptionRepo: EncryptionRepository,
        password: String,
        userId: String,
        localPassword: String? = nil,
        alternatePassword: String? = nil
    ) async throws -> SymmetricKey {
        // Fetch wrapped key from server
        guard let response = try await syncClient.getWrappedKey() else {
            throw EnvelopeError.noWrappedKeyOnServer
        }

        let serverWrapped = try CryptoService.parseEncryptedJSON(response.blob)

        // Try each candidate password; the wrapping key derives from password + userId
        var candidates = [password]
        if let alternatePassword, alternatePassword != password {
            candidates.append(alternatePassword)
        }

        var unwrapped: Data?
        var unwrapError: Error = EnvelopeError.noWrappedKeyOnServer
        for candidate in candidates {
            let wrappingKey = CryptoService.deriveWrappingKey(password: candidate, userId: userId)
            do {
                unwrapped = try CryptoService.unwrapMasterKey(serverWrapped, with: wrappingKey)
                break
            } catch {
                unwrapError = error
            }
        }
        guard let masterKeyData = unwrapped else {
            throw unwrapError
        }

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
    ///
    /// Non-fatal, same as `tryMigrateToEnvelope`: returns `true` on success
    /// or when nothing was needed, `false` only when the attempt failed.
    /// A `false` result is safe to ignore — the envelope key stays
    /// unmigrated locally, so the next successful `unlock(password:)`
    /// retries automatically (see the `metadata.envelopeVersion == nil`
    /// check there). Callers that want to tell the user surface it instead.
    @discardableResult
    @MainActor
    static func tryEnvelopeSetup(
        appState: AppState,
        password: String,
        masterKey: SymmetricKey
    ) async -> Bool {
        // Whether sync is on lives in `appState.settings`. The `syncEnabled`
        // column of the sync metadata record is never written (it stays false),
        // so it must not be used as a gate: doing so made every envelope upload
        // skip silently and left the server without a wrapped key.
        guard appState.settings.syncEnabled,
              let syncRepo = appState.syncRepo,
              let encryptionRepo = appState.encryptionRepo,
              let syncClient = appState.syncClient else {
            Log.debug("[Envelope] Setup skipped — sync not configured")
            return true
        }

        do {
            guard let syncMeta = try syncRepo.getMetadata(),
                  let userId = syncMeta.userId,
                  !userId.isEmpty else {
                Log.debug("[Envelope] Setup skipped — no userId in sync metadata")
                return true
            }

            // Already in envelope format (every vault created by this build is):
            // nothing to migrate locally, but the server may still hold no key,
            // which leaves every other device unable to connect. Publish ours
            // if it is missing; an existing server key is never overwritten.
            if let metadata = try encryptionRepo.get(), metadata.envelopeVersion != nil {
                let uploaded = try await publishWrappedKeyIfMissing(
                    syncClient: syncClient,
                    userId: userId,
                    password: password,
                    masterKeyData: masterKey.withUnsafeBytes { Data($0) }
                )
                Log.debug(uploaded
                    ? "[Envelope] Server had no wrapped key — uploaded ours"
                    : "[Envelope] Already migrated, server key present — nothing to do")
                return true
            }

            // Check server for an existing wrapped key from another device
            let serverResponse = try? await syncClient.getWrappedKey()

            if serverResponse != nil {
                // Another device already uploaded — onboard from server
                Log.debug("[Envelope] Server has wrapped key — onboarding from server")
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
                    Log.debug("[Envelope] Server key differs from local — re-encrypting data")
                    try appState.reEncryptAllData(from: masterKey, to: serverKey)
                    appState.keyManager.unlockWithKeyData(serverKeyData)
                    try await appState.loadNotes()
                }
                return true
            } else {
                // No server key — we're the first device, upload ours
                Log.debug("[Envelope] No server key — migrating local key to envelope")
                return await tryMigrateToEnvelope(
                    appState: appState,
                    password: password,
                    masterKey: masterKey
                )
            }
        } catch {
            Log.debug("[Envelope] Setup failed (non-fatal): \(error)")
            return false
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

        Log.debug("[Envelope] Local password re-wrap complete")
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
            // The caller only has a sync client when sync is configured, so a
            // user ID is all that is needed (see the note on `syncEnabled` above).
            guard let syncMeta = try syncRepo.getMetadata(),
                  let userId = syncMeta.userId, !userId.isEmpty else {
                Log.debug("[Envelope] Server re-wrap skipped — no userId")
                return
            }

            try await uploadWrappedKey(
                syncClient: syncClient,
                userId: userId,
                password: newPassword,
                masterKeyData: masterKeyData
            )
            Log.debug("[Envelope] Server re-wrap complete")
        } catch {
            Log.debug("[Envelope] Server re-wrap failed (non-fatal): \(error)")
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
