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

            Text("Welcome to Jottery")
                .font(.largeTitle.bold())

            Text("Private, encrypted notes")
                .foregroundStyle(.secondary)

            Picker("Setup Mode", selection: $selectedTab) {
                Text("New Vault").tag(0)
                Text("Connect to Server").tag(1)
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
            SecureField("Password", text: $password)
                .textFieldStyle(.roundedBorder)
                .frame(maxWidth: 320)

            SecureField("Confirm Password", text: $confirmPassword)
                .textFieldStyle(.roundedBorder)
                .frame(maxWidth: 320)

            if !confirmPassword.isEmpty && !passwordsMatch {
                Text("Passwords do not match")
                    .foregroundStyle(.red)
                    .font(.callout)
            }

            if let error {
                Text(error)
                    .foregroundStyle(.red)
                    .font(.callout)
            }

            Text("This password encrypts your notes. If lost, data cannot be recovered.")
                .font(.caption)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 320)

            Button(action: createVault) {
                if isCreating {
                    ProgressView()
                        .controlSize(.small)
                } else {
                    Text("Create Vault")
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

    var canRegister: Bool {
        !endpoint.isEmpty && !email.isEmpty && !password.isEmpty && !deviceName.isEmpty
    }

    var canImport: Bool {
        !importData.isEmpty
    }

    var body: some View {
        VStack(spacing: 16) {
            if !isRegistered {
                Picker("Method", selection: $method) {
                    Text("Register").tag(0)
                    Text("Import").tag(1)
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
                            Text("Register Device")
                        }
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(!canRegister || isWorking)
                } else {
                    Button(action: importCredentials) {
                        if isWorking {
                            ProgressView().controlSize(.small)
                        } else {
                            Text("Import")
                        }
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(!canImport || isWorking)
                }
            } else {
                Label("Connected", systemImage: "checkmark.circle.fill")
                    .foregroundStyle(.green)

                SecureField("Encryption Password", text: $encryptionPassword)
                    .textFieldStyle(.roundedBorder)
                    .frame(maxWidth: 320)
                    .disabled(isWorking)

                Text("Enter the same encryption password used on your other devices.")
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
                        Text("Unlock & Sync")
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
            TextField("Server URL", text: $endpoint)
                .textFieldStyle(.roundedBorder)
                .textContentType(.URL)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
                .frame(maxWidth: 320)

            TextField("Email", text: $email)
                .textFieldStyle(.roundedBorder)
                .textContentType(.emailAddress)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
                .frame(maxWidth: 320)

            SecureField("Server Password", text: $password)
                .textFieldStyle(.roundedBorder)
                .frame(maxWidth: 320)

            TextField("Device Name", text: $deviceName)
                .textFieldStyle(.roundedBorder)
                .frame(maxWidth: 320)
        }
    }

    // MARK: - Import Fields

    private var importFields: some View {
        Group {
            Text("Paste the base64 credentials blob from another device, or a raw API key.")
                .font(.caption)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 320)

            TextField("Server URL", text: $endpoint)
                .textFieldStyle(.roundedBorder)
                .textContentType(.URL)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
                .frame(maxWidth: 320)

            TextField("Device Name", text: $deviceName)
                .textFieldStyle(.roundedBorder)
                .frame(maxWidth: 320)

            TextField("Credentials", text: $importData, axis: .vertical)
                .textFieldStyle(.roundedBorder)
                .lineLimit(3...6)
                .font(.system(.body, design: .monospaced))
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
                var storedKey: String = trimmed
                var resolvedEndpoint: String

                if let data = Data(base64Encoded: trimmed),
                   let creds = try? JSONDecoder().decode(ImportedCredentials.self, from: data) {
                    // Full credentials blob with endpoint
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
                    storedKey = response.apiKey
                    resolvedEndpoint = normalised
                } else {
                    // Raw API key — needs endpoint
                    guard !endpoint.isEmpty else {
                        throw SyncSetupError.needsEndpoint
                    }
                    let normalised = normaliseEndpoint(endpoint)

                    try KeychainService.storeAPIKey(trimmed)
                    try appState.settingsRepo?.updateSync(enabled: true, endpoint: normalised)

                    appState.settings.syncEnabled = true
                    appState.settings.syncEndpoint = normalised
                    resolvedEndpoint = normalised
                }

                registeredApiKey = storedKey
                registeredEndpoint = resolvedEndpoint
                isRegistered = true
                isWorking = false
            } catch {
                self.error = error.localizedDescription
                isWorking = false
            }
        }
    }

    private func unlockAndSync() {
        error = nil
        isWorking = true
        syncProgress = "Creating vault…"

        Task {
            do {
                try appState.createVault(password: encryptionPassword)

                syncProgress = "Setting up sync…"
                if let apiKey = registeredApiKey {
                    appState.setupSync(apiKey: apiKey)
                }

                guard let syncService = appState.syncService else {
                    syncProgress = nil
                    isWorking = false
                    return
                }

                syncProgress = "Pushing local data…"
                try await syncService.push()

                syncProgress = "Pulling notes from server…"
                try await syncService.pull()

                syncProgress = "Finishing…"
                try await syncService.finalise()
                try? appState.loadNotes()
                appState.lastSyncAt = Date()

                let count = appState.notes.count
                syncProgress = "Done — \(count) note\(count == 1 ? "" : "s") synced"
                isWorking = false
            } catch {
                self.error = error.localizedDescription
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
