import SwiftUI

/// Sync setup flow — register device or import credentials.
struct SyncSetupView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss

    var onComplete: (() -> Void)?

    @State private var selectedTab = 0
    @State private var endpoint = ""
    @State private var email = ""
    @State private var password = ""
    @State private var deviceName = ""
    @State private var importData = ""
    @State private var error: String?
    @State private var isWorking = false
    @State private var success = false

    var body: some View {
        NavigationStack {
            Form {
                Picker("Method", selection: $selectedTab) {
                    Text("Register").tag(0)
                    Text("Import").tag(1)
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
                        Label("Device registered successfully", systemImage: "checkmark.circle.fill")
                            .foregroundStyle(.green)

                        Button("Done") {
                            onComplete?()
                            dismiss()
                        }
                        .buttonStyle(.borderedProminent)
                    }
                }
            }
            .navigationTitle("Sync Setup")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
            }
        }
    }

    private var registerSection: some View {
        Section("Register Device") {
            TextField("Server URL", text: $endpoint)
                .textContentType(.URL)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)

            TextField("Email", text: $email)
                .textContentType(.emailAddress)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)

            SecureField("Server Password", text: $password)

            TextField("Device Name", text: $deviceName)

            Button(action: registerDevice) {
                if isWorking {
                    ProgressView()
                } else {
                    Text("Register")
                }
            }
            .disabled(endpoint.isEmpty || email.isEmpty || password.isEmpty || deviceName.isEmpty || isWorking || success)
        }
    }

    private var importSection: some View {
        Section("Import Credentials") {
            Text("Paste the base64 credentials blob from another device.")
                .font(.caption)
                .foregroundStyle(.secondary)

            TextField("Server URL", text: $endpoint)
                .textContentType(.URL)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)

            TextField("Device Name", text: $deviceName)

            TextField("Credentials", text: $importData, axis: .vertical)
                .lineLimit(3...6)
                .font(.system(.body, design: .monospaced))

            Button(action: importCredentials) {
                if isWorking {
                    ProgressView()
                } else {
                    Text("Import")
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

                // Update in-memory settings
                appState.settings.syncEnabled = true
                appState.settings.syncEndpoint = normalised

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
                // Try parsing as a base64 JSON credentials blob
                let trimmed = importData.trimmingCharacters(in: .whitespacesAndNewlines)

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
                } else {
                    // Try as a raw API key — needs endpoint
                    guard !endpoint.isEmpty else {
                        throw SyncSetupError.needsEndpoint
                    }
                    let normalised = normaliseEndpoint(endpoint)

                    try KeychainService.storeAPIKey(trimmed)
                    try appState.settingsRepo?.updateSync(enabled: true, endpoint: normalised)

                    appState.settings.syncEnabled = true
                    appState.settings.syncEndpoint = normalised
                }

                success = true
                isWorking = false
            } catch {
                self.error = error.localizedDescription
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

private struct ImportedCredentials: Codable {
    let endpoint: String
    let apiKey: String
    let clientId: String?
}

enum SyncSetupError: LocalizedError {
    case invalidCredentials
    case needsEndpoint

    var errorDescription: String? {
        switch self {
        case .invalidCredentials: return "Invalid credentials data"
        case .needsEndpoint: return "Server URL is required when importing a raw API key"
        }
    }
}
