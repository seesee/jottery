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
    ///
    /// `syncClient` overrides `appState.syncClient`, as for `tryEnvelopeSetup`.
    @discardableResult
    @MainActor
    static func tryMigrateToEnvelope(
        appState: AppState,
        password: String,
        masterKey: SymmetricKey,
        syncClient explicitClient: SyncClient? = nil
    ) async -> Bool {
        // Whether sync is on lives in `appState.settings`. The `syncEnabled`
        // column of the sync metadata record is never written (it stays false),
        // so it must not be used as a gate: doing so made every envelope upload
        // skip silently and left the server without a wrapped key.
        guard appState.settings.syncEnabled,
              let syncRepo = appState.syncRepo,
              let encryptionRepo = appState.encryptionRepo,
              let syncClient = explicitClient ?? appState.syncClient else {
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

    /// Download the account's wrapped master key and unwrap it, without
    /// changing anything locally. Returns `nil` if the server holds no key.
    ///
    /// The blob is wrapped with the notes password, so `password` should be
    /// that. Builds before this fix could wrap it with the account password,
    /// so `alternatePassword` (the account password) is tried if the first
    /// attempt fails to unwrap. An empty alternate is ignored.
    ///
    /// Throws `EnvelopeError.cannotUnwrapServerKey` when neither password
    /// opens the key. Any other error (the server being unreachable, say) is
    /// passed through unchanged, so a caller can tell "no key", "wrong
    /// password" and "could not ask" apart. Splitting this from
    /// `adoptServerKey` lets the connect flow check the key BEFORE it creates
    /// a vault, so a failure never leaves a device on a key of its own.
    static func fetchServerKey(
        syncClient: SyncClient,
        userId: String,
        password: String,
        alternatePassword: String? = nil
    ) async throws -> Data? {
        guard let response = try await syncClient.getWrappedKey() else {
            return nil
        }

        let serverWrapped = try CryptoService.parseEncryptedJSON(response.blob)

        // Try each candidate password; the wrapping key derives from password + userId
        var candidates = [password]
        if let alternatePassword, !alternatePassword.isEmpty, alternatePassword != password {
            candidates.append(alternatePassword)
        }

        for candidate in candidates {
            let wrappingKey = CryptoService.deriveWrappingKey(password: candidate, userId: userId)
            guard let unwrapped = try? CryptoService.unwrapMasterKey(serverWrapped, with: wrappingKey) else {
                continue
            }
            guard unwrapped.count == CryptoService.keyLength else {
                throw EnvelopeError.invalidMasterKeyLength
            }
            return unwrapped
        }

        throw EnvelopeError.cannotUnwrapServerKey
    }

    /// How the account's server key relates to this device's own vault key.
    enum AccountKeyStatus: Equatable {
        /// The server holds no key yet: this device can publish its own.
        case noServerKey
        /// The server key is the same as this device's key: safe to sync.
        case matchesLocalKey
        /// The account already has a different key, unwrapped here. Syncing as
        /// things are would leave notes unreadable in both directions, so the
        /// caller either moves the vault onto this key
        /// (`AppState.switchVaultToMasterKey`) or refuses to join.
        case differs(accountKey: Data)
    }

    /// Before a device that already has a vault joins an existing account,
    /// compare the account's key with the vault's own. Nothing is changed.
    ///
    /// Throws `cannotUnwrapServerKey` when the account's key cannot be opened
    /// with either password, and passes server failures through.
    static func checkAccountKey(
        syncClient: SyncClient,
        userId: String,
        password: String,
        alternatePassword: String? = nil,
        localMasterKeyData: Data
    ) async throws -> AccountKeyStatus {
        guard let serverKey = try await fetchServerKey(
            syncClient: syncClient,
            userId: userId,
            password: password,
            alternatePassword: alternatePassword
        ) else {
            return .noServerKey
        }

        return serverKey == localMasterKeyData ? .matchesLocalKey : .differs(accountKey: serverKey)
    }

    /// `checkAccountKey` for a device registered (or cloned) on the account
    /// moments ago, with `syncClient` holding that device's API key. The key
    /// can only be fetched once the device exists, so when the check fails the
    /// device is revoked again (best effort) before the error is rethrown: a
    /// failed join must not leave an orphaned device on the account. A
    /// `.differs` result keeps the device, because the caller may still merge;
    /// it must revoke the device itself if it does not.
    static func checkAccountKeyForNewDevice(
        syncClient: SyncClient,
        userId: String,
        password: String,
        alternatePassword: String? = nil,
        localMasterKeyData: Data
    ) async throws -> AccountKeyStatus {
        do {
            return try await checkAccountKey(
                syncClient: syncClient,
                userId: userId,
                password: password,
                alternatePassword: alternatePassword,
                localMasterKeyData: localMasterKeyData
            )
        } catch {
            try? await syncClient.revokeThisDevice()
            throw error
        }
    }

    /// Save an already-unwrapped server master key as this device's vault key,
    /// wrapped under `localPassword`. Purely local: no network, so once
    /// `fetchServerKey` has succeeded this cannot fail for a server reason.
    static func adoptServerKey(
        _ masterKeyData: Data,
        encryptionRepo: EncryptionRepository,
        localPassword: String
    ) throws -> SymmetricKey {
        let local = try localEnvelope(for: masterKeyData, localPassword: localPassword)

        // Save envelope metadata
        try encryptionRepo.saveEnvelope(
            envelopeVersion: 1,
            deviceSalt: local.deviceSalt,
            localWrappedMaster: local.localWrappedMaster,
            wrappingKdfVersion: CryptoService.wrappingKdfVersion
        )

        // Update verification token
        if var metadata = try encryptionRepo.get() {
            metadata.verification = local.verification
            try encryptionRepo.store(metadata)
        }

        return SymmetricKey(data: masterKeyData)
    }

    /// The envelope metadata that makes `masterKeyData` this device's vault key,
    /// wrapped locally under `localPassword`, built on `current` (its creation
    /// date and algorithm are kept). Nothing is saved: `VaultReKeyService`
    /// stores it in the same transaction as the re-encrypted data.
    static func envelopeMetadata(
        for masterKeyData: Data,
        localPassword: String,
        current: EncryptionMetadata
    ) throws -> EncryptionMetadata {
        let local = try localEnvelope(for: masterKeyData, localPassword: localPassword)
        var metadata = current
        metadata.salt = ""
        metadata.iterations = 0
        metadata.envelopeVersion = 1
        metadata.deviceSalt = local.deviceSalt
        metadata.localWrappedMaster = local.localWrappedMaster
        metadata.wrappingKdfVersion = CryptoService.wrappingKdfVersion
        metadata.verification = local.verification
        return metadata
    }

    /// The local half of the envelope: a fresh device salt, the master key
    /// wrapped under a key derived from it and `localPassword`, and a
    /// verification token encrypted with the master key.
    private static func localEnvelope(
        for masterKeyData: Data,
        localPassword: String
    ) throws -> (deviceSalt: String, localWrappedMaster: String, verification: String) {
        let deviceSalt = CryptoService.generateSalt()
        let deviceKey = CryptoService.deriveKey(
            password: localPassword,
            salt: deviceSalt,
            iterations: CryptoService.defaultIterations
        )
        let localWrapped = try CryptoService.serializeEncryptedJSON(
            CryptoService.wrapMasterKey(masterKeyData, with: deviceKey)
        )
        let verification = try CryptoService.serializeEncryptedJSON(
            CryptoService.encryptText(
                EncryptionMetadata.verificationPlaintext, key: SymmetricKey(data: masterKeyData)
            )
        )
        return (deviceSalt.base64EncodedString(), localWrapped, verification)
    }

    /// Onboard a new device by downloading the wrapped master key from the server.
    /// Returns the unwrapped master key as a SymmetricKey. Nothing is written
    /// locally unless the key could be opened.
    static func onboardFromServer(
        syncClient: SyncClient,
        encryptionRepo: EncryptionRepository,
        password: String,
        userId: String,
        localPassword: String? = nil,
        alternatePassword: String? = nil
    ) async throws -> SymmetricKey {
        guard let masterKeyData = try await fetchServerKey(
            syncClient: syncClient,
            userId: userId,
            password: password,
            alternatePassword: alternatePassword
        ) else {
            throw EnvelopeError.noWrappedKeyOnServer
        }

        return try adoptServerKey(
            masterKeyData,
            encryptionRepo: encryptionRepo,
            localPassword: localPassword ?? password
        )
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
    ///
    /// `syncClient` overrides `appState.syncClient`. Sync setup passes one built
    /// from the new device's API key, because it runs before the app's sync
    /// client is started: `appState.syncClient` is still nil at that point.
    @discardableResult
    @MainActor
    static func tryEnvelopeSetup(
        appState: AppState,
        password: String,
        masterKey: SymmetricKey,
        syncClient explicitClient: SyncClient? = nil
    ) async -> Bool {
        // Whether sync is on lives in `appState.settings`. The `syncEnabled`
        // column of the sync metadata record is never written (it stays false),
        // so it must not be used as a gate: doing so made every envelope upload
        // skip silently and left the server without a wrapped key.
        guard appState.settings.syncEnabled,
              let syncRepo = appState.syncRepo,
              let encryptionRepo = appState.encryptionRepo,
              let syncClient = explicitClient ?? appState.syncClient else {
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

            // Legacy vault: check for a key another device has already put on
            // the account. A server failure throws (caught below) rather than
            // being taken for "no key", which would migrate this vault onto
            // its own key and leave it diverged from the account's.
            if let serverKeyData = try await fetchServerKey(
                syncClient: syncClient,
                userId: userId,
                password: password
            ) {
                Log.debug("[Envelope] Server has wrapped key — onboarding from server")
                let localKeyData = masterKey.withUnsafeBytes { Data($0) }
                if serverKeyData == localKeyData {
                    _ = try adoptServerKey(
                        serverKeyData, encryptionRepo: encryptionRepo, localPassword: password
                    )
                } else {
                    // Re-encrypt the whole vault onto the account's key and
                    // switch to it in one transaction: a failure changes nothing.
                    // (Saving the server key first, as this used to, left notes
                    // under the old key whenever re-encryption skipped any.)
                    Log.debug("[Envelope] Server key differs from local — re-encrypting the vault onto it")
                    try await appState.switchVaultToMasterKey(serverKeyData, notesPassword: password)
                }
                return true
            } else {
                // No server key — we're the first device, upload ours
                Log.debug("[Envelope] No server key — migrating local key to envelope")
                return await tryMigrateToEnvelope(
                    appState: appState,
                    password: password,
                    masterKey: masterKey,
                    syncClient: syncClient
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
    case cannotUnwrapServerKey
    case invalidMasterKeyLength
    case notInitialised

    var errorDescription: String? {
        switch self {
        case .noWrappedKeyOnServer:
            return "No wrapped key found on server — the source device may not have migrated yet"
        case .cannotUnwrapServerKey:
            return "The account's encryption key could not be opened with the password provided"
        case .invalidMasterKeyLength:
            return "Invalid master key length after unwrapping"
        case .notInitialised:
            return "Encryption repository not initialised"
        }
    }
}
