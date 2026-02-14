import SwiftUI

/// Sync setup flow — register device or import credentials.
struct SyncSetupView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss

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
            .disabled(endpoint.isEmpty || email.isEmpty || password.isEmpty || deviceName.isEmpty || isWorking)
        }
    }

    private var importSection: some View {
        Section("Import Credentials") {
            Text("Paste the base64 credentials blob from another device.")
                .font(.caption)
                .foregroundStyle(.secondary)

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
            .disabled(importData.isEmpty || isWorking)
        }
    }

    private func registerDevice() {
        error = nil
        isWorking = true

        Task {
            do {
                let client = SyncClient(endpoint: endpoint)
                let response = try await client.registerDevice(
                    email: email,
                    password: password,
                    deviceName: deviceName
                )

                try KeychainService.storeAPIKey(response.apiKey)
                try KeychainService.storeClientId(response.clientId)
                try appState.settingsRepo?.updateSync(enabled: true, endpoint: endpoint)

                success = true
                isWorking = false

                // Trigger initial sync
                await appState.triggerSync()
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
                // Parse base64 credentials blob
                guard let data = Data(base64Encoded: importData.trimmingCharacters(in: .whitespacesAndNewlines)),
                      let json = try? JSONDecoder().decode(ImportedCredentials.self, from: data) else {
                    throw SyncSetupError.invalidCredentials
                }

                // Clone device with the imported API key
                let client = SyncClient(endpoint: json.endpoint)
                let response = try await client.cloneDevice(
                    apiKey: json.apiKey,
                    deviceName: deviceName.isEmpty ? "iOS" : deviceName
                )

                try KeychainService.storeAPIKey(response.apiKey)
                try KeychainService.storeClientId(response.clientId)
                try appState.settingsRepo?.updateSync(enabled: true, endpoint: json.endpoint)

                success = true
                isWorking = false

                await appState.triggerSync()
            } catch {
                self.error = error.localizedDescription
                isWorking = false
            }
        }
    }
}

private struct ImportedCredentials: Codable {
    let endpoint: String
    let apiKey: String
    let clientId: String?
}

enum SyncSetupError: LocalizedError {
    case invalidCredentials

    var errorDescription: String? {
        "Invalid credentials data"
    }
}
