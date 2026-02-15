import SwiftUI

struct SetupScreen: View {
    @Environment(AppState.self) private var appState
    @State private var selectedTab = 0

    var body: some View {
        VStack(spacing: 32) {
            Spacer()

            Image(systemName: "doc.text.fill")
                .font(.system(size: 48))
                .foregroundStyle(.accent)

            Text(L.setupWelcome)
                .font(.largeTitle.bold())

            Text(L.setupSubtitle)
                .foregroundStyle(.secondary)

            Picker(L.setupModePicker, selection: $selectedTab) {
                Text(L.setupNewVault).tag(0)
                Text(L.setupConnectToServer).tag(1)
            }
            .pickerStyle(.segmented)
            .frame(maxWidth: 400)

            if selectedTab == 0 {
                NewVaultView()
            } else {
                ConnectToServerView()
            }

            Spacer()
        }
        .padding()
    }
}

// MARK: - New Vault

private struct NewVaultView: View {
    @Environment(AppState.self) private var appState
    @State private var password = ""
    @State private var confirmPassword = ""
    @State private var error: String?
    @State private var isCreating = false

    var passwordsMatch: Bool {
        !password.isEmpty && password == confirmPassword
    }

    var body: some View {
        VStack(spacing: 16) {
            SecureField(L.setupPassword, text: $password)
                .textFieldStyle(.roundedBorder)
                .frame(maxWidth: 320)

            SecureField(L.setupConfirmPassword, text: $confirmPassword)
                .textFieldStyle(.roundedBorder)
                .frame(maxWidth: 320)

            if !confirmPassword.isEmpty && !passwordsMatch {
                Text(L.setupPasswordsDoNotMatch)
                    .foregroundStyle(.red)
                    .font(.callout)
            }

            if let error {
                Text(error)
                    .foregroundStyle(.red)
                    .font(.callout)
            }

            Text(L.setupPasswordWarning)
                .font(.caption)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 320)

            Button(action: createVault) {
                if isCreating {
                    ProgressView()
                        .controlSize(.small)
                } else {
                    Text(L.setupCreateVault)
                }
            }
            .buttonStyle(.borderedProminent)
            .disabled(!passwordsMatch || isCreating)
        }
    }

    private func createVault() {
        guard passwordsMatch else { return }
        error = nil
        isCreating = true

        Task {
            do {
                try appState.createVault(password: password)
                isCreating = false
            } catch {
                self.error = error.localizedDescription
                self.isCreating = false
            }
        }
    }
}

// MARK: - Connect to Server

private struct ConnectToServerView: View {
    @Environment(AppState.self) private var appState

    @State private var method = 0  // 0 = Register, 1 = Import
    @State private var endpoint = ""
    @State private var email = ""
    @State private var password = ""
    @State private var deviceName = ""
    @State private var importData = ""
    @State private var error: String?
    @State private var isWorking = false

    // After successful registration/import
    @State private var isRegistered = false
    @State private var registeredApiKey: String?
    @State private var registeredEndpoint: String?
    @State private var encryptionPassword = ""
    @State private var syncProgress: String?

    // Imported credential data (set during import, used during unlock)
    @State private var importedSalt: Data?
    @State private var importedIterations: UInt32?
    @State private var encryptedCredentialPayload: String?  // For jottery:v1: format

    var canRegister: Bool {
        !endpoint.isEmpty && !email.isEmpty && !password.isEmpty && !deviceName.isEmpty
    }

    var canImport: Bool {
        !importData.isEmpty
    }

    var body: some View {
        VStack(spacing: 16) {
            if !isRegistered {
                Picker(L.setupMethod, selection: $method) {
                    Text(L.setupRegister).tag(0)
                    Text(L.setupImport).tag(1)
                }
                .pickerStyle(.segmented)
                .frame(maxWidth: 320)

                if method == 0 {
                    registerFields
                } else {
                    importFields
                }

                if let error {
                    Text(error)
                        .foregroundStyle(.red)
                        .font(.callout)
                        .frame(maxWidth: 320)
                }

                if method == 0 {
                    Button(action: registerDevice) {
                        if isWorking {
                            ProgressView().controlSize(.small)
                        } else {
                            Text(L.setupRegisterDevice)
                        }
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(!canRegister || isWorking)
                } else {
                    Button(action: importCredentials) {
                        if isWorking {
                            ProgressView().controlSize(.small)
                        } else {
                            Text(L.setupImport)
                        }
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(!canImport || isWorking)
                }
            } else {
                Label(L.setupConnected, systemImage: "checkmark.circle.fill")
                    .foregroundStyle(.green)

                SecureField(L.setupEncryptionPassword, text: $encryptionPassword)
                    .textFieldStyle(.roundedBorder)
                    .frame(maxWidth: 320)
                    .disabled(isWorking)

                Text(L.setupEncryptionPasswordHint)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: 320)

                if let error {
                    Text(error)
                        .foregroundStyle(.red)
                        .font(.callout)
                }

                if let syncProgress {
                    HStack(spacing: 8) {
                        ProgressView()
                            .controlSize(.small)
                        Text(syncProgress)
                            .font(.callout)
                            .foregroundStyle(.secondary)
                    }
                }

                Button(action: unlockAndSync) {
                    if isWorking {
                        ProgressView().controlSize(.small)
                    } else {
                        Text(L.setupUnlockAndSync)
                    }
                }
                .buttonStyle(.borderedProminent)
                .disabled(encryptionPassword.isEmpty || isWorking)
            }
        }
    }

    // MARK: - Register Fields

    private var registerFields: some View {
        Group {
            TextField(L.setupServerUrl, text: $endpoint)
                .textFieldStyle(.roundedBorder)
                .textContentType(.URL)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
                .frame(maxWidth: 320)

            TextField(L.setupEmail, text: $email)
                .textFieldStyle(.roundedBorder)
                .textContentType(.emailAddress)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
                .frame(maxWidth: 320)

            SecureField(L.setupServerPassword, text: $password)
                .textFieldStyle(.roundedBorder)
                .frame(maxWidth: 320)

            TextField(L.setupDeviceName, text: $deviceName)
                .textFieldStyle(.roundedBorder)
                .frame(maxWidth: 320)
        }
    }

    // MARK: - Import Fields

    /// Whether the credentials field looks like a format that already contains the endpoint.
    private var credentialsContainEndpoint: Bool {
        let trimmed = importData.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.hasPrefix("jottery:v1:") { return true }
        if let data = Data(base64Encoded: trimmed),
           let _ = try? JSONDecoder().decode(ImportedCredentials.self, from: data) {
            return true
        }
        return false
    }

    private var importFields: some View {
        Group {
            Text(L.setupImportCredentials)
                .font(.caption)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 320)

            TextField(L.setupCredentials, text: $importData, axis: .vertical)
                .textFieldStyle(.roundedBorder)
                .lineLimit(3...6)
                .font(.system(.body, design: .monospaced))
                .frame(maxWidth: 320)

            if !credentialsContainEndpoint {
                TextField(L.setupServerUrl, text: $endpoint)
                    .textFieldStyle(.roundedBorder)
                    .textContentType(.URL)
                    .autocorrectionDisabled()
                    .textInputAutocapitalization(.never)
                    .frame(maxWidth: 320)
            }

            TextField(L.setupDeviceName, text: $deviceName)
                .textFieldStyle(.roundedBorder)
                .frame(maxWidth: 320)
        }
    }

    // MARK: - Actions

    private func registerDevice() {
        error = nil
        isWorking = true

        Task {
            do {
                let normalised = normaliseEndpoint(endpoint)
                let client = SyncClient(endpoint: normalised)
                let response = try await client.registerDevice(
                    email: email,
                    password: password,
                    deviceName: deviceName
                )

                try KeychainService.storeAPIKey(response.apiKey)
                try KeychainService.storeClientId(response.clientId)
                try appState.settingsRepo?.updateSync(enabled: true, endpoint: normalised)

                appState.settings.syncEnabled = true
                appState.settings.syncEndpoint = normalised

                registeredApiKey = response.apiKey
                registeredEndpoint = normalised
                isRegistered = true
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

                if trimmed.hasPrefix("jottery:v1:") {
                    // Modern encrypted format: jottery:v1:<salt_b64>.<encrypted_payload_b64>
                    try handleEncryptedCredentials(trimmed)
                } else if let data = Data(base64Encoded: trimmed),
                          let creds = try? JSONDecoder().decode(LegacyCredentials.self, from: data) {
                    // Legacy format: base64(JSON) with endpoint, apiKey, clientId, salt
                    try await handleLegacyCredentials(creds)
                } else if let data = Data(base64Encoded: trimmed),
                          let creds = try? JSONDecoder().decode(ImportedCredentials.self, from: data) {
                    // Legacy format without salt
                    try await handlePlainCredentials(creds)
                } else {
                    // Raw API key — needs endpoint
                    guard !endpoint.isEmpty else {
                        throw SyncSetupError.needsEndpoint
                    }
                    try handleRawApiKey(trimmed)
                }

                isRegistered = true
                isWorking = false
            } catch {
                self.error = error.localizedDescription
                isWorking = false
            }
        }
    }

    /// Handle `jottery:v1:<salt_b64>.<encrypted_payload_b64>` format.
    /// The payload is encrypted with the user's master key, so we can't
    /// decrypt it until they enter the encryption password in the next step.
    private func handleEncryptedCredentials(_ input: String) throws {
        let payload = String(input.dropFirst("jottery:v1:".count))
        guard let dotIndex = payload.firstIndex(of: ".") else {
            throw SyncSetupError.invalidCredentials
        }

        let saltB64 = String(payload[payload.startIndex..<dotIndex])
        let encryptedPayload = String(payload[payload.index(after: dotIndex)...])

        guard let saltData = Data(base64Encoded: saltB64), saltData.count >= 32 else {
            throw SyncSetupError.invalidCredentials
        }

        // Store for use during the "Unlock & Sync" step
        importedSalt = saltData
        importedIterations = CryptoService.defaultIterations
        encryptedCredentialPayload = encryptedPayload

        // We don't have the endpoint or API key yet — they're inside the
        // encrypted payload. We'll decrypt after the user enters the password.
    }

    /// Handle legacy base64 JSON with salt included.
    private func handleLegacyCredentials(_ creds: LegacyCredentials) async throws {
        let normalised = normaliseEndpoint(creds.endpoint)

        // Extract and store the salt for vault creation
        if let saltData = Data(base64Encoded: creds.salt), saltData.count >= 32 {
            importedSalt = saltData
        }

        // Clone the device to get a fresh API key for this device
        let client = SyncClient(endpoint: normalised)
        let response = try await client.cloneDevice(
            apiKey: creds.apiKey,
            deviceName: deviceName.isEmpty ? "iOS" : deviceName
        )

        try KeychainService.storeAPIKey(response.apiKey)
        try KeychainService.storeClientId(response.clientId)
        try appState.settingsRepo?.updateSync(enabled: true, endpoint: normalised)

        appState.settings.syncEnabled = true
        appState.settings.syncEndpoint = normalised
        registeredApiKey = response.apiKey
        registeredEndpoint = normalised
    }

    /// Handle base64 JSON credentials without salt.
    private func handlePlainCredentials(_ creds: ImportedCredentials) async throws {
        let normalised = normaliseEndpoint(creds.endpoint)

        let client = SyncClient(endpoint: normalised)
        let response = try await client.cloneDevice(
            apiKey: creds.apiKey,
            deviceName: deviceName.isEmpty ? "iOS" : deviceName
        )

        try KeychainService.storeAPIKey(response.apiKey)
        try KeychainService.storeClientId(response.clientId)
        try appState.settingsRepo?.updateSync(enabled: true, endpoint: normalised)

        appState.settings.syncEnabled = true
        appState.settings.syncEndpoint = normalised
        registeredApiKey = response.apiKey
        registeredEndpoint = normalised
    }

    /// Handle a raw API key pasted directly.
    private func handleRawApiKey(_ apiKey: String) throws {
        let normalised = normaliseEndpoint(endpoint)

        try KeychainService.storeAPIKey(apiKey)
        try appState.settingsRepo?.updateSync(enabled: true, endpoint: normalised)

        appState.settings.syncEnabled = true
        appState.settings.syncEndpoint = normalised
        registeredApiKey = apiKey
        registeredEndpoint = normalised
    }

    private func unlockAndSync() {
        error = nil
        isWorking = true
        syncProgress = L.setupProgressCreatingVault

        Task {
            do {
                // If we have an encrypted credential payload (jottery:v1: format),
                // we need to decrypt it first using the password + imported salt.
                if let encryptedPayload = encryptedCredentialPayload, let salt = importedSalt {
                    syncProgress = L.setupProgressDecrypting

                    // The payload is base64(JSON of {"ciphertext":"...","iv":"..."})
                    guard let payloadData = Data(base64Encoded: encryptedPayload),
                          let payloadJSON = String(data: payloadData, encoding: .utf8) else {
                        throw SyncSetupError.invalidCredentials
                    }
                    let encrypted = try CryptoService.parseEncryptedJSON(payloadJSON)

                    // The jottery:v1: format doesn't include the iteration count.
                    // Try common values — vault creation uses 600k, but the web
                    // app's credential import path hardcodes 100k for imported vaults.
                    let iterationCandidates: [UInt32] = [600_000, 100_000]
                    var decryptedJSON: String?
                    var matchedIterations: UInt32 = CryptoService.defaultIterations

                    for candidate in iterationCandidates {
                        let key = CryptoService.deriveKey(
                            password: encryptionPassword,
                            salt: salt,
                            iterations: candidate
                        )
                        if let json = try? CryptoService.decryptText(encrypted, key: key) {
                            decryptedJSON = json
                            matchedIterations = candidate
                            break
                        }
                    }

                    guard let json = decryptedJSON else {
                        throw SyncSetupError.wrongPassword
                    }

                    importedIterations = matchedIterations

                    guard let credsData = json.data(using: .utf8) else {
                        throw SyncSetupError.invalidCredentials
                    }
                    let creds = try JSONDecoder().decode(ImportedCredentials.self, from: credsData)

                    // Now clone the device to get a fresh API key
                    syncProgress = L.setupProgressRegistering
                    let normalised = normaliseEndpoint(creds.endpoint)
                    let client = SyncClient(endpoint: normalised)
                    let response = try await client.cloneDevice(
                        apiKey: creds.apiKey,
                        deviceName: deviceName.isEmpty ? "iOS" : deviceName
                    )

                    try KeychainService.storeAPIKey(response.apiKey)
                    try KeychainService.storeClientId(response.clientId)
                    try appState.settingsRepo?.updateSync(enabled: true, endpoint: normalised)

                    appState.settings.syncEnabled = true
                    appState.settings.syncEndpoint = normalised
                    registeredApiKey = response.apiKey
                    registeredEndpoint = normalised
                }

                syncProgress = L.setupProgressCreatingVault
                try appState.createVault(
                    password: encryptionPassword,
                    existingSalt: importedSalt,
                    existingIterations: importedIterations
                )

                // createVault() sets isFirstLaunch=false, which transitions
                // the UI to NoteListView. From here on, update appState
                // properties so progress is visible on the NoteListView
                // status bar (this view's @State is about to become invisible).
                syncProgress = L.setupProgressSettingUp
                appState.isSyncing = true
                appState.syncStatusMessage = L.setupProgressSettingUp

                if let apiKey = registeredApiKey {
                    appState.setupSync(apiKey: apiKey)
                }

                guard let syncService = appState.syncService else {
                    syncProgress = nil
                    appState.isSyncing = false
                    appState.syncStatusMessage = nil
                    isWorking = false
                    return
                }

                appState.syncStatusMessage = L.setupProgressPushing
                try await syncService.push()

                appState.syncStatusMessage = L.setupProgressPulling
                try await syncService.pull()

                appState.syncStatusMessage = L.setupProgressFinishing
                try await syncService.finalise()
                try appState.loadNotes()
                appState.lastSyncAt = Date()
                appState.isSyncing = false

                let count = appState.notes.count
                appState.syncStatusMessage = "Synced — \(count) note\(count == 1 ? "" : "s")"
                isWorking = false

                // Clear the status message after a few seconds
                try? await Task.sleep(for: .seconds(4))
                appState.syncStatusMessage = nil
            } catch {
                self.error = error.localizedDescription
                appState.syncError = error.localizedDescription
                appState.syncStatusMessage = nil
                appState.isSyncing = false
                syncProgress = nil
                isWorking = false
            }
        }
    }

    private func normaliseEndpoint(_ raw: String) -> String {
        var url = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if !url.hasPrefix("http://") && !url.hasPrefix("https://") {
            url = "https://\(url)"
        }
        if url.hasSuffix("/") {
            url = String(url.dropLast())
        }
        return url
    }
}

/// Decoded from a base64 credentials blob shared between devices.
private struct ImportedCredentials: Codable {
    let endpoint: String
    let apiKey: String
    let clientId: String?
}

/// Legacy format includes the encryption salt for key derivation compatibility.
private struct LegacyCredentials: Codable {
    let endpoint: String
    let apiKey: String
    let clientId: String?
    let salt: String
}
