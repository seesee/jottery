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

            Button(action: importCredentials) {
                if isWorking {
                    ProgressView()
                } else {
                    Text(L.syncSetupImportAction)
                }
            }
            .disabled(importData.isEmpty || isWorking || success)
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

                // Attempt envelope setup before declaring success.
                // If another device already uploaded a wrapped key, we onboard
                // from that key; otherwise we upload ours. Wrapped with the
                // vault password, verified above — never the server password.
                if let masterKey = appState.keyManager.masterKey {
                    let envelopeOK = await EnvelopeService.tryEnvelopeSetup(
                        appState: appState,
                        password: notesPassword,
                        masterKey: masterKey
                    )
                    if !envelopeOK {
                        // Non-fatal: sync itself is set up, but other devices
                        // can't onboard until the key upload succeeds. It
                        // retries automatically on the next unlock.
                        self.error = L.syncSetupEnvelopeUploadFailed
                    }
                }

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

                if trimmed.hasPrefix("jottery:v1:") {
                    // Modern encrypted format — decrypt using current master key
                    try await handleEncryptedCredentials(trimmed)
                } else if let data = Data(base64Encoded: trimmed),
                          let creds = try? JSONDecoder().decode(LegacyCredentials.self, from: data) {
                    // Legacy format with salt
                    try await handleCredentials(apiKey: creds.apiKey, endpoint: creds.endpoint)
                } else if let data = Data(base64Encoded: trimmed),
                          let creds = try? JSONDecoder().decode(ImportedCredentials.self, from: data) {
                    // Legacy format without salt
                    try await handleCredentials(apiKey: creds.apiKey, endpoint: creds.endpoint)
                } else {
                    // Raw API key
                    guard !endpoint.isEmpty else {
                        throw SyncSetupError.needsEndpoint
                    }
                    let normalised = try SyncEndpoint.normalise(endpoint)
                    try KeychainService.storeAPIKey(trimmed)
                    try appState.settingsRepo?.updateSync(enabled: true, endpoint: normalised)
                    appState.settings.syncEnabled = true
                    appState.settings.syncEndpoint = normalised
                    registeredApiKey = trimmed
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
    private func handleCredentials(apiKey: String, endpoint: String) async throws {
        let normalised = try SyncEndpoint.normalise(endpoint)
        let client = SyncClient(endpoint: normalised)
        let response = try await client.cloneDevice(
            apiKey: apiKey,
            deviceName: deviceName.isEmpty ? "iOS" : deviceName
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

    var errorDescription: String? {
        switch self {
        case .invalidCredentials: return L.syncSetupErrorInvalidCredentials
        case .needsEndpoint: return L.syncSetupErrorNeedsEndpoint
        case .wrongPassword: return L.syncSetupErrorWrongPassword
        case .wrongVaultPassword: return L.syncSetupErrorWrongVaultPassword
        }
    }
}
