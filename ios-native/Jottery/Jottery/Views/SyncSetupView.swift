import SwiftUI

/// Sync setup flow — register device or import credentials.
struct SyncSetupView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss

    /// Called with the API key after successful registration or import.
    var onComplete: ((_ apiKey: String) -> Void)?

    @State private var selectedTab = 0
    @State private var endpoint = ""
    @State private var email = ""
    @State private var password = ""
    @State private var notesPassword = ""
    @State private var deviceName = ""
    @State private var importData = ""
    @State private var error: String?
    @State private var isWorking = false
    @State private var success = false
    @State private var registeredApiKey: String?
    @State private var vaultIsEnvelope = false

    private var importNeedsNotesPassword: Bool {
        Self.importNeedsNotesPassword(importData, vaultIsEnvelope: vaultIsEnvelope)
    }

    var body: some View {
        NavigationStack {
            Form {
                Picker(L.syncSetupMethod, selection: $selectedTab) {
                    Text(L.syncSetupRegister).tag(0)
                    Text(L.syncSetupImport).tag(1)
                }
                .pickerStyle(.segmented)
                .listRowBackground(Color.clear)

                if selectedTab == 0 {
                    registerSection
                } else {
                    importSection
                }

                if let error {
                    Section {
                        Text(error)
                            .foregroundStyle(.red)
                    }
                }

                if success {
                    Section {
                        Label(L.syncSetupSuccess, systemImage: "checkmark.circle.fill")
                            .foregroundStyle(.green)

                        Button(L.syncSetupDone) {
                            if let key = registeredApiKey {
                                onComplete?(key)
                            }
                            dismiss()
                        }
                        .buttonStyle(.borderedProminent)
                    }
                }
            }
            .navigationTitle(L.syncSetupTitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L.syncSetupCancel) { dismiss() }
                }
            }
            .task { vaultIsEnvelope = currentVaultIsEnvelope() }
        }
    }

    private var registerSection: some View {
        Section(L.syncSetupRegisterDevice) {
            TextField(L.syncSetupServerUrl, text: $endpoint)
                .textContentType(.URL)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)

            TextField(L.syncSetupEmail, text: $email)
                .textContentType(.emailAddress)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)

            SecureField(L.syncSetupServerPassword, text: $password)

            TextField(L.syncSetupDeviceName, text: $deviceName)

            Text(L.syncSetupNotesPasswordHint)
                .font(.caption)
                .foregroundStyle(.secondary)

            SecureField(L.syncSetupNotesPassword, text: $notesPassword)

            Button(action: registerDevice) {
                if isWorking {
                    ProgressView()
                } else {
                    Text(L.syncSetupRegisterAction)
                }
            }
            .disabled(endpoint.isEmpty || email.isEmpty || password.isEmpty || deviceName.isEmpty || notesPassword.isEmpty || isWorking || success)
        }
    }

    private var credentialsContainEndpoint: Bool {
        let trimmed = importData.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.hasPrefix("jottery:v1:") { return true }
        if let data = Data(base64Encoded: trimmed),
           let _ = try? JSONDecoder().decode(ImportedCredentials.self, from: data) {
            return true
        }
        return false
    }

    private var importSection: some View {
        Section(L.syncSetupImportCredentials) {
            Text(L.syncSetupImportHint)
                .font(.caption)
                .foregroundStyle(.secondary)

            TextField(L.syncSetupCredentials, text: $importData, axis: .vertical)
                .lineLimit(3...6)
                .font(.system(.body, design: .monospaced))

            if !credentialsContainEndpoint {
                TextField(L.syncSetupServerUrl, text: $endpoint)
                    .textContentType(.URL)
                    .autocorrectionDisabled()
                    .textInputAutocapitalization(.never)
            }

            TextField(L.syncSetupDeviceName, text: $deviceName)

            if importNeedsNotesPassword {
                Text(L.syncSetupNotesPasswordHint)
                    .font(.caption)
                    .foregroundStyle(.secondary)

                SecureField(L.syncSetupNotesPassword, text: $notesPassword)
            }

            Button(action: importCredentials) {
                if isWorking {
                    ProgressView()
                } else {
                    Text(L.syncSetupImportAction)
                }
            }
            .disabled(importData.isEmpty || (importNeedsNotesPassword && notesPassword.isEmpty) || isWorking || success)
        }
    }

    private func registerDevice() {
        error = nil
        isWorking = true

        Task {
            do {
                // Verify the notes (vault) password against the local vault
                // BEFORE registering — this is the password that ends up
                // wrapping the envelope master key, and getting it wrong
                // would silently upload a key other devices can't unwrap
                // (jottery-md6b). Registration must not proceed on failure.
                guard await appState.verifyVaultPassword(notesPassword) else {
                    self.error = SyncSetupError.wrongVaultPassword.localizedDescription
                    isWorking = false
                    return
                }

                let normalised = try SyncEndpoint.normalise(endpoint)
                let client = SyncClient(endpoint: normalised)
                let response = try await client.registerDevice(
                    email: email,
                    password: password,
                    deviceName: deviceName
                )
                let deviceClient = SyncClient(endpoint: normalised, apiKey: response.apiKey)

                try await checkAccountKeyBeforeJoining(
                    deviceClient: deviceClient,
                    userId: response.userId,
                    notesPassword: notesPassword,
                    serverPassword: password
                )

                try KeychainService.storeAPIKey(response.apiKey)
                try KeychainService.storeClientId(response.clientId)
                try appState.settingsRepo?.updateSync(enabled: true, endpoint: normalised)

                // Store userId for envelope encryption
                if var syncMeta = try? appState.syncRepo?.getMetadata() {
                    syncMeta.userId = response.userId
                    try? appState.syncRepo?.saveMetadata(syncMeta)
                }

                appState.settings.syncEnabled = true
                appState.settings.syncEndpoint = normalised

                registeredApiKey = response.apiKey

                await publishAccountKey(deviceClient: deviceClient, notesPassword: notesPassword)

                success = true
                isWorking = false
            } catch {
                self.error = error.localizedDescription
                isWorking = false
            }
        }
    }

    private func importCredentials() {
        error = nil
        isWorking = true

        Task {
            do {
                let trimmed = importData.trimmingCharacters(in: .whitespacesAndNewlines)

                // Only `jottery:v1:` credentials prove anything about the key
                // (see handleEncryptedCredentials). For the other formats an
                // envelope vault checks the account's key, which needs the notes
                // password: verify it before anything is sent to the server.
                var checkPassword: String?
                if Self.importNeedsNotesPassword(trimmed, vaultIsEnvelope: currentVaultIsEnvelope()) {
                    guard await appState.verifyVaultPassword(notesPassword) else {
                        self.error = SyncSetupError.wrongVaultPassword.localizedDescription
                        isWorking = false
                        return
                    }
                    checkPassword = notesPassword
                }

                if trimmed.hasPrefix("jottery:v1:") {
                    // Modern encrypted format — decrypt using current master key
                    try await handleEncryptedCredentials(trimmed)
                } else if let data = Data(base64Encoded: trimmed),
                          let creds = try? JSONDecoder().decode(LegacyCredentials.self, from: data) {
                    // Legacy format with salt
                    try await handleCredentials(
                        apiKey: creds.apiKey, endpoint: creds.endpoint, notesPassword: checkPassword
                    )
                } else if let data = Data(base64Encoded: trimmed),
                          let creds = try? JSONDecoder().decode(ImportedCredentials.self, from: data) {
                    // Legacy format without salt
                    try await handleCredentials(
                        apiKey: creds.apiKey, endpoint: creds.endpoint, notesPassword: checkPassword
                    )
                } else {
                    // Raw API key. Cloned like the other formats rather than
                    // reused as is: that gives this device its own key and
                    // client ID, and the account's user ID, without which the
                    // encryption key can be neither checked nor published.
                    guard !endpoint.isEmpty else {
                        throw SyncSetupError.needsEndpoint
                    }
                    try await handleCredentials(
                        apiKey: trimmed, endpoint: endpoint, notesPassword: checkPassword
                    )
                }

                success = true
                isWorking = false
            } catch {
                self.error = error.localizedDescription
                isWorking = false
            }
        }
    }

    /// Decrypt `jottery:v1:` credentials using the current master key.
    ///
    /// The account's key is not checked here: these credentials are encrypted
    /// with the exporting device's master key, so decrypting them at all shows
    /// that this vault holds the same key.
    private func handleEncryptedCredentials(_ input: String) async throws {
        guard let masterKey = appState.keyManager.masterKey else {
            throw SyncSetupError.invalidCredentials
        }

        let payload = String(input.dropFirst("jottery:v1:".count))
        guard let dotIndex = payload.firstIndex(of: ".") else {
            throw SyncSetupError.invalidCredentials
        }

        let encryptedB64 = String(payload[payload.index(after: dotIndex)...])

        guard let payloadData = Data(base64Encoded: encryptedB64),
              let payloadJSON = String(data: payloadData, encoding: .utf8) else {
            throw SyncSetupError.invalidCredentials
        }

        let encrypted = try CryptoService.parseEncryptedJSON(payloadJSON)
        let decryptedJSON = try CryptoService.decryptText(encrypted, key: masterKey)

        guard let credsData = decryptedJSON.data(using: .utf8) else {
            throw SyncSetupError.invalidCredentials
        }
        let creds = try JSONDecoder().decode(ImportedCredentials.self, from: credsData)

        try await handleCredentials(apiKey: creds.apiKey, endpoint: creds.endpoint)
    }

    /// Clone device and store credentials.
    ///
    /// With `notesPassword` (already verified), the account's key is checked
    /// before anything is saved, and this vault's key is published if the
    /// account has none yet, as when registering.
    private func handleCredentials(apiKey: String, endpoint: String, notesPassword: String? = nil) async throws {
        let normalised = try SyncEndpoint.normalise(endpoint)
        let client = SyncClient(endpoint: normalised)
        let response = try await client.cloneDevice(
            apiKey: apiKey,
            deviceName: deviceName.isEmpty ? "iOS" : deviceName
        )
        let deviceClient = SyncClient(endpoint: normalised, apiKey: response.apiKey)

        if let notesPassword {
            try await checkAccountKeyBeforeJoining(
                deviceClient: deviceClient,
                userId: response.userId,
                notesPassword: notesPassword,
                serverPassword: nil
            )
        }

        try KeychainService.storeAPIKey(response.apiKey)
        try KeychainService.storeClientId(response.clientId)
        try appState.settingsRepo?.updateSync(enabled: true, endpoint: normalised)

        // Store userId for envelope encryption
        if var syncMeta = try? appState.syncRepo?.getMetadata() {
            syncMeta.userId = response.userId
            try? appState.syncRepo?.saveMetadata(syncMeta)
        }

        appState.settings.syncEnabled = true
        appState.settings.syncEndpoint = normalised
        registeredApiKey = response.apiKey

        if let notesPassword {
            await publishAccountKey(deviceClient: deviceClient, notesPassword: notesPassword)
        }
    }

    // MARK: - Account key

    private func currentVaultIsEnvelope() -> Bool {
        (try? appState.encryptionRepo?.get())?.envelopeVersion != nil
    }

    /// Whether importing `credentials` needs the notes password: an envelope
    /// vault must check the account's key, and only `jottery:v1:` credentials
    /// already prove that it matches. Legacy vaults are not checked: they
    /// migrate to the account's key during envelope setup on unlock.
    static func importNeedsNotesPassword(_ credentials: String, vaultIsEnvelope: Bool) -> Bool {
        let trimmed = credentials.trimmingCharacters(in: .whitespacesAndNewlines)
        return vaultIsEnvelope && !trimmed.isEmpty && !trimmed.hasPrefix("jottery:v1:")
    }

    /// A device that already has a vault must share the account's master key,
    /// or syncing would leave notes unreadable in both directions (and undoing
    /// that means re-encrypting the whole vault). Call straight after the
    /// device is registered or cloned, BEFORE anything is saved locally: a
    /// refusal revokes that device again, leaving nothing behind. Legacy vaults
    /// are not checked: they migrate to the account's key during envelope setup.
    private func checkAccountKeyBeforeJoining(
        deviceClient: SyncClient,
        userId: String,
        notesPassword: String,
        serverPassword: String?
    ) async throws {
        guard currentVaultIsEnvelope(), let masterKey = appState.keyManager.masterKey else { return }
        do {
            _ = try await EnvelopeService.checkAccountKeyForNewDevice(
                syncClient: deviceClient,
                userId: userId,
                password: notesPassword,
                alternatePassword: serverPassword,
                localMasterKeyData: masterKey.withUnsafeBytes { Data($0) }
            )
        } catch EnvelopeError.accountKeyDiffers {
            throw SyncSetupError.accountKeyDiffers
        } catch EnvelopeError.cannotUnwrapServerKey {
            throw SyncSetupError.accountKeyUnreadable
        }
    }

    /// Envelope setup before declaring success: publish this vault's key if the
    /// account has none (legacy vaults onboard from or migrate to the account's
    /// key instead). Wrapped with the notes password, which the caller has
    /// verified, and never with the server password.
    ///
    /// Uses the new device's own client: the app's sync client only starts
    /// once this sheet completes, so `appState.syncClient` is still nil here,
    /// and setup used to skip silently. The account was then left without a
    /// key until the next password unlock, which Face ID users may never do.
    private func publishAccountKey(deviceClient: SyncClient, notesPassword: String) async {
        guard let masterKey = appState.keyManager.masterKey else { return }
        let envelopeOK = await EnvelopeService.tryEnvelopeSetup(
            appState: appState,
            password: notesPassword,
            masterKey: masterKey,
            syncClient: deviceClient
        )
        if !envelopeOK {
            // Non-fatal: sync itself is set up, but other devices can't onboard
            // until the key upload succeeds. It retries on the next unlock.
            self.error = L.syncSetupEnvelopeUploadFailed
        }
    }
}

private struct ImportedCredentials: Codable {
    let endpoint: String
    let apiKey: String
    let clientId: String?
}

private struct LegacyCredentials: Codable {
    let endpoint: String
    let apiKey: String
    let clientId: String?
    let salt: String
}

enum SyncSetupError: LocalizedError {
    case invalidCredentials
    case needsEndpoint
    case wrongPassword
    case wrongVaultPassword
    case encryptionPasswordMismatch
    case accountKeyDiffers
    case accountKeyUnreadable

    var errorDescription: String? {
        switch self {
        case .invalidCredentials: return L.syncSetupErrorInvalidCredentials
        case .needsEndpoint: return L.syncSetupErrorNeedsEndpoint
        case .wrongPassword: return L.syncSetupErrorWrongPassword
        case .wrongVaultPassword: return L.syncSetupErrorWrongVaultPassword
        case .encryptionPasswordMismatch: return L.syncSetupErrorEncryptionPasswordMismatch
        case .accountKeyDiffers: return L.syncSetupErrorAccountKeyDiffers
        case .accountKeyUnreadable: return L.syncSetupErrorAccountKeyUnreadable
        }
    }
}
