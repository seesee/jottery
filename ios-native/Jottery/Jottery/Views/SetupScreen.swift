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
        .onAppear {
            appState.initialise()
        }
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
    @State private var endpoint = ""
    @State private var email = ""
    @State private var password = ""
    @State private var deviceName = ""
    @State private var error: String?
    @State private var isConnecting = false
    @State private var isRegistered = false
    @State private var encryptionPassword = ""

    var canConnect: Bool {
        !endpoint.isEmpty && !email.isEmpty && !password.isEmpty && !deviceName.isEmpty
    }

    var body: some View {
        VStack(spacing: 16) {
            if !isRegistered {
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

                if let error {
                    Text(error)
                        .foregroundStyle(.red)
                        .font(.callout)
                }

                Button(action: registerDevice) {
                    if isConnecting {
                        ProgressView()
                            .controlSize(.small)
                    } else {
                        Text("Register Device")
                    }
                }
                .buttonStyle(.borderedProminent)
                .disabled(!canConnect || isConnecting)
            } else {
                Text("Device registered successfully!")
                    .foregroundStyle(.green)

                SecureField("Encryption Password", text: $encryptionPassword)
                    .textFieldStyle(.roundedBorder)
                    .frame(maxWidth: 320)

                Text("Enter the same encryption password used on your other devices.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: 320)

                Button("Unlock & Sync") {
                    unlockAndSync()
                }
                .buttonStyle(.borderedProminent)
                .disabled(encryptionPassword.isEmpty)
            }
        }
    }

    private func registerDevice() {
        error = nil
        isConnecting = true

        Task {
            do {
                let client = SyncClient(endpoint: endpoint)
                let response = try await client.registerDevice(
                    email: email,
                    password: password,
                    deviceName: deviceName
                )

                // Store API key and client ID
                try KeychainService.storeAPIKey(response.apiKey)
                try KeychainService.storeClientId(response.clientId)

                // Update settings
                try appState.settingsRepo?.updateSync(enabled: true, endpoint: endpoint)

                isRegistered = true
                isConnecting = false
            } catch {
                self.error = error.localizedDescription
                isConnecting = false
            }
        }
    }

    private func unlockAndSync() {
        // For connecting to an existing server, we need to create the vault
        // with the encryption password, then do an initial pull.
        Task {
            do {
                try appState.createVault(password: encryptionPassword)
                await appState.triggerSync()
            } catch {
                self.error = error.localizedDescription
            }
        }
    }
}
