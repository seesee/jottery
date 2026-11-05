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
        masterKeyData: Data,
        as write: WrappedKeyWrite
    ) async throws {
        try await upload(
            blob: wrappedKeyBlob(password: password, userId: userId, masterKeyData: masterKeyData),
            syncClient: syncClient,
            as: write
        )
    }

    /// `masterKeyData`, wrapped as it is on the server.
    private static func wrappedKeyBlob(password: String, userId: String, masterKeyData: Data) throws -> String {
        // Derive wrapping key from password + userId (1M iterations)
        let wrappingKey = try CryptoService.deriveWrappingKey(password: password, userId: userId)
        let serverWrapped = try CryptoService.wrapMasterKey(masterKeyData, with: wrappingKey)
        return try CryptoService.serializeEncryptedJSON(serverWrapped)
    }

    private static func upload(blob: String, syncClient: SyncClient, as write: WrappedKeyWrite) async throws {
        try await syncClient.putWrappedKey(PutWrappedKeyRequest(
            blob: blob,
            kdfVersion: CryptoService.wrappingKdfVersion,
            kdfIterations: Int(CryptoService.wrappingIterations)
        ), as: write)
    }

    /// What `reconcileWrappedKey` found on the server, and did about it.
    enum WrappedKeyOutcome: Equatable {
        /// The server held no key: this device's was uploaded.
        case published
        /// The notes password opens the server's copy, so it was left alone.
        case leftAlone
        /// The notes password can't open the server's copy, and this device
        /// has synced with the account: its key was uploaded over it.
        case repaired
        /// The notes password can't open the server's copy, and this device
        /// has never synced, so it can't know it holds the account's key.
        case unreadable
        /// The notes password can't open the server's copy, though this device
        /// has opened or uploaded it before. That rules out a blob from an
        /// older build, so the password was changed on another device: the
        /// server's copy is the newer one and was left alone.
        case passwordChangedElsewhere
        /// An upload was refused because the key changed, and reading it again
        /// failed or found it changing still. Whatever it is now, it's no blob
        /// from an older build, so this device must never repair it.
        case changedUnderThisDevice
    }

    /// Make sure the account's wrapped key can be opened with the notes
    /// password. Throws if the server can't be reached or rejects an upload.
    ///
    /// New vaults are in envelope format from the start, so nothing else ever
    /// publishes their key: without this a first device leaves the server with
    /// no key and every other device fails with "no wrapped key found".
    ///
    /// Older builds could wrap the key with the account password, which the
    /// server knows, and nothing may adopt a key opened that way. Those
    /// accounts are repaired here, by a device that has synced with the account
    /// and so holds its real key (`hasSyncedWithAccount`), but only one that
    /// has never opened or uploaded the blob (`hasOpenedServerKey`). Any other
    /// device has proof the blob isn't an old one, so failing to open it means
    /// another device changed the password, and overwriting it would undo that
    /// change, with the two devices taking turns to undo each other's.
    ///
    /// A blob the notes password does open is never overwritten, even when it
    /// holds a different key: that means this device has diverged from the
    /// account, and replacing the account's key would damage it.
    ///
    /// Every upload is conditional on what was read, which is what closes the
    /// gap between reading and writing: two devices can't both act on one read.
    /// A refusal means the key just changed, so it's read again and judged
    /// afresh, except that a blob that has appeared or changed under this device
    /// is no old blob, whoever wrote it, so it's never repaired.
    @discardableResult
    static func reconcileWrappedKey(
        syncClient: SyncClient,
        userId: String,
        password: String,
        masterKeyData: Data,
        hasSyncedWithAccount: Bool,
        hasOpenedServerKey: Bool
    ) async throws -> WrappedKeyOutcome {
        var keyChangedUnderUs = false
        for _ in 0..<2 {
            let write: WrappedKeyWrite
            let outcome: WrappedKeyOutcome
            let found: ServerKeyRead
            do {
                found = try await readServerKey(syncClient: syncClient, userId: userId, password: password)
            } catch _ where keyChangedUnderUs {
                // The refusal was the proof. Losing it to a failed read would
                // let the next unlock repair over another device's new password.
                return .changedUnderThisDevice
            }
            switch found {
            case .opened:
                return .leftAlone
            case .absent:
                write = .firstUpload
                outcome = .published
            case .unreadable(let updatedAt):
                guard !hasOpenedServerKey, !keyChangedUnderUs else { return .passwordChangedElsewhere }
                guard hasSyncedWithAccount else { return .unreadable }
                write = .repair(updatedAt: updatedAt)
                outcome = .repaired
            }
            do {
                try await uploadWrappedKey(
                    syncClient: syncClient,
                    userId: userId,
                    password: password,
                    masterKeyData: masterKeyData,
                    as: write
                )
                return outcome
            } catch SyncClientError.wrappedKeyConflict {
                keyChangedUnderUs = true
            }
        }
        // It changed twice in a row. The next unlock looks again, as a device
        // that knows better than to repair it.
        return .changedUnderThisDevice
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
    /// `write` says what the upload is for, going by what the caller read.
    @discardableResult
    @MainActor
    static func tryMigrateToEnvelope(
        appState: AppState,
        password: String,
        masterKey: SymmetricKey,
        syncClient explicitClient: SyncClient? = nil,
        as write: WrappedKeyWrite = .firstUpload
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
                masterKeyData: masterKeyData,
                as: write
            )
            try syncRepo.markServerKeyOpened()

            // Generate device salt and derive device key for local wrapping
            let deviceSalt = CryptoService.generateSalt()
            let deviceKey = try CryptoService.deriveKey(
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
                wrappingKdfVersion: CryptoService.wrappingKdfVersion,
                verification: verificationJSON
            )

            Log.debug("[Envelope] Migration complete")
            return true
        } catch SyncClientError.wrappedKeyConflict {
            // Nothing has changed here, so the next unlock reads the key again.
            // A repair that lost means another device just wrote it, and this
            // device must never try to repair it again.
            if case .repair = write {
                try? syncRepo.markServerKeyOpened()
                appState.accountKeyNotice = .passwordChangedElsewhere
            }
            return false
        } catch {
            Log.debug("[Envelope] Migration failed (non-fatal): \(error)")
            return false
        }
    }

    // MARK: - Onboard From Server

    /// Download the account's wrapped master key and unwrap it, without
    /// changing anything locally. Returns `nil` if the server holds no key.
    ///
    /// `password` must be the notes password, and it is the only one ever
    /// tried. The server sees the account password in plaintext and chooses
    /// the user ID, so it could wrap a key of its own with them: a key opened
    /// that way proves nothing, and adopting it would hand the server every
    /// note. Blobs older builds wrapped with the account password are therefore
    /// refused here, until a connected device repairs them
    /// (`reconcileWrappedKey`).
    ///
    /// Throws `EnvelopeError.cannotUnwrapServerKey` when the password doesn't
    /// open the key. Any other error (the server being unreachable, say) is
    /// passed through unchanged, so a caller can tell "no key", "wrong
    /// password" and "could not ask" apart. Splitting this from
    /// `adoptServerKey` lets the connect flow check the key BEFORE it creates
    /// a vault, so a failure never leaves a device on a key of its own.
    static func fetchServerKey(
        syncClient: SyncClient,
        userId: String,
        password: String
    ) async throws -> Data? {
        switch try await readServerKey(syncClient: syncClient, userId: userId, password: password) {
        case .absent: return nil
        case .opened(let key): return key
        case .unreadable: throw EnvelopeError.cannotUnwrapServerKey
        }
    }

    /// What's on the server, as far as `password` can tell.
    private enum ServerKeyRead {
        case absent
        case opened(Data)
        /// `updatedAt` identifies this read, for a repair of what it found.
        case unreadable(updatedAt: String?)
    }

    private static func readServerKey(
        syncClient: SyncClient,
        userId: String,
        password: String
    ) async throws -> ServerKeyRead {
        guard let response = try await syncClient.getWrappedKey() else {
            return .absent
        }

        guard let serverWrapped = try? CryptoService.parseEncryptedJSON(response.blob) else {
            return .unreadable(updatedAt: response.updatedAt)
        }
        let wrappingKey = try CryptoService.deriveWrappingKey(password: password, userId: userId)
        guard let unwrapped = try? CryptoService.unwrapMasterKey(serverWrapped, with: wrappingKey) else {
            return .unreadable(updatedAt: response.updatedAt)
        }
        guard unwrapped.count == CryptoService.keyLength else {
            throw EnvelopeError.invalidMasterKeyLength
        }
        return .opened(unwrapped)
    }

    /// For a device with no vault yet, connecting with `notesPassword`: the
    /// account's key to adopt, or nil when the account has none and this
    /// password is about to protect a brand-new vault. Nothing is changed.
    ///
    /// Throws a `SyncSetupError` when the password can't be used, and passes
    /// server failures through. The password rules only apply to that new
    /// vault: an account's existing password has to keep working, however short
    /// and even if it matches the account password, or those users could never
    /// add a device. Only the notes password opening the key proves it's an
    /// existing one. The caller warns about a match (`PasswordPolicy`).
    static func accountKeyForNewVault(
        syncClient: SyncClient,
        userId: String,
        notesPassword: String,
        accountPassword: String
    ) async throws -> Data? {
        do {
            if let serverKey = try await fetchServerKey(
                syncClient: syncClient, userId: userId, password: notesPassword
            ) {
                return serverKey
            }
        } catch EnvelopeError.cannotUnwrapServerKey {
            throw SyncSetupError.encryptionPasswordMismatch
        }
        switch PasswordPolicy.violation(newNotesPassword: notesPassword, accountPassword: accountPassword) {
        case .matchesAccountPassword: throw SyncSetupError.notesPasswordMatchesAccountPassword
        case .tooShort: throw SyncSetupError.notesPasswordTooShort
        case nil: return nil
        }
    }

    /// How the account's server key relates to this device's own vault key.
    enum AccountKeyStatus: Equatable {
        /// The server holds no key yet: this device can publish its own.
        case noServerKey
        /// The server key is the same as this device's key: safe to sync.
        case matchesLocalKey
        /// The account already has a different key, opened here with the notes
        /// password. Syncing as things are would leave notes unreadable in both
        /// directions, so the caller either moves the vault onto this key
        /// (`AppState.switchVaultToMasterKey`) or refuses to join.
        ///
        /// Only the notes password can produce this, and that is the whole
        /// basis for trusting the key: the server doesn't know that password,
        /// so it can't have wrapped this. Moving a vault re-encrypts everything
        /// and uploads it, so it must never happen without the user agreeing.
        case differs(accountKey: Data)
    }

    /// Before a device that already has a vault joins an existing account,
    /// compare the account's key with the vault's own. Nothing is changed.
    ///
    /// Throws `cannotUnwrapServerKey` when the notes password can't open the
    /// account's key, and passes server failures through.
    static func checkAccountKey(
        syncClient: SyncClient,
        userId: String,
        password: String,
        localMasterKeyData: Data
    ) async throws -> AccountKeyStatus {
        guard let serverKey = try await fetchServerKey(
            syncClient: syncClient,
            userId: userId,
            password: password
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
        localMasterKeyData: Data
    ) async throws -> AccountKeyStatus {
        do {
            return try await checkAccountKey(
                syncClient: syncClient,
                userId: userId,
                password: password,
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
            wrappingKdfVersion: CryptoService.wrappingKdfVersion,
            verification: local.verification
        )

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
        let deviceKey = try CryptoService.deriveKey(
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
        localPassword: String? = nil
    ) async throws -> SymmetricKey {
        guard let masterKeyData = try await fetchServerKey(
            syncClient: syncClient,
            userId: userId,
            password: password
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
    /// have already uploaded one). If found, a legacy vault onboards from it,
    /// asking the user first if that means re-encrypting local data
    /// (`AppState.pendingAccountKeyMerge`). If no server key exists, falls
    /// through to `tryMigrateToEnvelope` to upload ours. An envelope vault
    /// publishes a missing key and repairs one the notes password can't open
    /// (`reconcileWrappedKey`).
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

            // A password change that hasn't reached the server yet comes first,
            // or this device would take its own old blob for someone else's.
            try await flushPendingWrappedKey(syncClient: syncClient, syncRepo: syncRepo)

            let hasSyncedWithAccount = syncMeta.lastSyncAt != nil

            // Already in envelope format (every vault created by this build is):
            // nothing to migrate locally, but the server may hold no key, which
            // leaves every other device unable to connect, or one that older
            // builds wrapped with the account password.
            if let metadata = try encryptionRepo.get(), metadata.envelopeVersion != nil {
                let outcome = try await reconcileWrappedKey(
                    syncClient: syncClient,
                    userId: userId,
                    password: password,
                    masterKeyData: masterKey.withUnsafeBytes { Data($0) },
                    hasSyncedWithAccount: hasSyncedWithAccount,
                    hasOpenedServerKey: syncMeta.serverKeyOpened
                )
                Log.debug("[Envelope] Server key check: \(outcome)")
                switch outcome {
                case .published, .leftAlone, .repaired, .changedUnderThisDevice:
                    try syncRepo.markServerKeyOpened()
                case .passwordChangedElsewhere:
                    // Set here too for a key that changed under a repair: that
                    // blob is no old one, so this device must leave it for good.
                    try syncRepo.markServerKeyOpened()
                    appState.accountKeyNotice = .passwordChangedElsewhere
                case .unreadable:
                    break
                }
                return outcome != .unreadable
            }

            // Legacy vault: check for a key another device has already put on
            // the account. A server failure throws (caught below) rather than
            // being taken for "no key", which would migrate this vault onto
            // its own key and leave it diverged from the account's.
            let serverKeyData: Data?
            var upload = WrappedKeyWrite.firstUpload
            switch try await readServerKey(syncClient: syncClient, userId: userId, password: password) {
            case .opened(let key):
                serverKeyData = key
            case .absent:
                serverKeyData = nil
            case .unreadable where syncMeta.serverKeyOpened:
                // As in `reconcileWrappedKey`: this device has opened this blob
                // before, so it isn't an old one to repair.
                appState.accountKeyNotice = .passwordChangedElsewhere
                return true
            case .unreadable(let updatedAt) where hasSyncedWithAccount:
                // As in `reconcileWrappedKey`: this device has synced, so it
                // holds the account's key. Migrating publishes it.
                Log.debug("[Envelope] Notes password can't open the server key — repairing it")
                serverKeyData = nil
                upload = .repair(updatedAt: updatedAt)
            case .unreadable:
                throw EnvelopeError.cannotUnwrapServerKey
            }

            if let serverKeyData {
                Log.debug("[Envelope] Server has wrapped key — onboarding from server")
                try syncRepo.markServerKeyOpened()
                let localKeyData = masterKey.withUnsafeBytes { Data($0) }
                if serverKeyData == localKeyData {
                    _ = try adoptServerKey(
                        serverKeyData, encryptionRepo: encryptionRepo, localPassword: password
                    )
                } else {
                    // Moving the vault uploads everything under the account's
                    // key, so the user decides. Nothing changes until they do.
                    Log.debug("[Envelope] Server key differs from local — asking before re-encrypting")
                    appState.pendingAccountKeyMerge = PendingAccountKeyMerge(
                        accountKey: serverKeyData, notesPassword: password
                    )
                }
                return true
            } else {
                // No server key — we're the first device, upload ours
                Log.debug("[Envelope] No server key — migrating local key to envelope")
                return await tryMigrateToEnvelope(
                    appState: appState,
                    password: password,
                    masterKey: masterKey,
                    syncClient: syncClient,
                    as: upload
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
        let newDeviceKey = try CryptoService.deriveKey(
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
            wrappingKdfVersion: CryptoService.wrappingKdfVersion,
            verification: verificationJSON
        )

        Log.debug("[Envelope] Local password re-wrap complete")
    }

    /// Server re-wrap: queue the new wrapped key and try to upload it. Returns
    /// false if it's still waiting, and it then goes up with the next unlock
    /// or sync (`flushPendingWrappedKey`). The password has already changed on
    /// this device by now, so a failure here doesn't undo it, but until the key
    /// is up a new device still needs the old password. `syncClient` is nil when
    /// sync is configured but hasn't started.
    @discardableResult
    static func changePasswordServer(
        syncRepo: SyncRepository,
        syncClient: SyncClient?,
        newPassword: String,
        masterKeyData: Data
    ) async -> Bool {
        do {
            guard let syncMeta = try syncRepo.getMetadata(),
                  let userId = syncMeta.userId, !userId.isEmpty else {
                Log.debug("[Envelope] Server re-wrap skipped — no userId")
                return true
            }

            // Saved before the upload, so that neither a failure nor the app
            // being killed leaves the server on the old password with nothing
            // on this device knowing.
            try syncRepo.savePendingWrappedKey(wrappedKeyBlob(
                password: newPassword, userId: userId, masterKeyData: masterKeyData
            ))
            guard let syncClient else { return false }
            try await flushPendingWrappedKey(syncClient: syncClient, syncRepo: syncRepo)
            Log.debug("[Envelope] Server re-wrap complete")
            return true
        } catch {
            Log.debug("[Envelope] Server re-wrap failed (will retry): \(error)")
            return false
        }
    }

    /// Upload a wrapped key left waiting by a password change, if there is one.
    /// Needs no password, so a Face ID unlock can finish the job. Throws if it
    /// couldn't be uploaded, and the key stays queued.
    static func flushPendingWrappedKey(syncClient: SyncClient, syncRepo: SyncRepository) async throws {
        guard let blob = try syncRepo.getMetadata()?.pendingWrappedKey else { return }
        try await upload(blob: blob, syncClient: syncClient, as: .passwordChange)
        try syncRepo.markServerKeyOpened()
        try syncRepo.savePendingWrappedKey(nil)
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
            return L.envelopeErrorNoServerKey
        case .cannotUnwrapServerKey:
            return L.envelopeErrorCantOpenServerKey
        case .invalidMasterKeyLength:
            return L.envelopeErrorInvalidKeyLength
        case .notInitialised:
            return L.envelopeErrorNotInitialised
        }
    }
}
