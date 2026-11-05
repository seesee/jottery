import SwiftUI

struct SetupScreen: View {
    @Environment(AppState.self) private var appState
    @State private var selectedTab = 0

    var body: some View {
        ScrollableScreen { form }
    }

    private var form: some View {
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
                RestoreFromBackupButton()
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

    var passwordIsTooShort: Bool {
        PasswordPolicy.violation(newNotesPassword: password) == .tooShort
    }

    var body: some View {
        VStack(spacing: 16) {
            PasswordField(L.setupPassword, text: $password)
                .textFieldStyle(.roundedBorder)
                .frame(maxWidth: 320)

            PasswordField(L.setupConfirmPassword, text: $confirmPassword)
                .textFieldStyle(.roundedBorder)
                .frame(maxWidth: 320)

            if !password.isEmpty && passwordIsTooShort {
                Text(L.notesPasswordTooShort(PasswordPolicy.minimumNotesPasswordLength))
                    .foregroundStyle(.red)
                    .font(.callout)
            }

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
            .disabled(!passwordsMatch || passwordIsTooShort || isCreating)
        }
    }

    private func createVault() {
        guard passwordsMatch, !passwordIsTooShort else { return }
        error = nil
        isCreating = true

        Task {
            do {
                try await appState.createVault(password: password)
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
    @State private var isWorking = false

    // After successful registration
    @State private var isRegistered = false
    @State private var registeredApiKey: String?
    @State private var encryptionPassword = ""
    @State private var syncProgress: String?
    @State private var failedAttempts = 0
    @State private var showDeleteConfirm = false

    var canRegister: Bool {
        !endpoint.isEmpty && !email.isEmpty && !password.isEmpty && !deviceName.isEmpty
    }

    var body: some View {
        VStack(spacing: 16) {
            if !isRegistered {
                registerFields

                if let error {
                    Text(error)
                        .foregroundStyle(.red)
                        .font(.callout)
                        .frame(maxWidth: 320)
                }

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
                Label(L.setupConnected, systemImage: "checkmark.circle.fill")
                    .foregroundStyle(.green)

                PasswordField(L.setupEncryptionPassword, text: $encryptionPassword)
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

                if failedAttempts >= 5 {
                    Button(action: { showDeleteConfirm = true }) {
                        Text(L.unlockDeleteAndStartOver)
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(.red)
                }
            }
        }
        .alert(L.unlockDeleteConfirmTitle, isPresented: $showDeleteConfirm) {
            Button(L.unlockDeleteConfirmAction, role: .destructive) {
                appState.wipeAllData()
            }
            Button(L.unlockDeleteConfirmCancel, role: .cancel) {}
        } message: {
            Text(L.unlockDeleteConfirmMessage)
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

            PasswordField(L.setupServerPassword, text: $password)
                .textFieldStyle(.roundedBorder)
                .frame(maxWidth: 320)

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
                try appState.syncRepo?.beginAccount(userId: response.userId)

                appState.settings.syncEnabled = true
                appState.settings.syncEndpoint = normalised

                // No envelope setup here: the vault does not exist yet on first
                // launch, and `password` is the server password, which must never
                // wrap the master key. The key is published (or downloaded) after
                // the vault is created, using the encryption password.

                registeredApiKey = response.apiKey
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
        syncProgress = L.setupProgressCreatingVault

        Task {
            do {
                // If the account already has a master key on the server, make sure
                // the encryption password opens it BEFORE anything is created.
                // Once the vault exists the UI moves on and an error can no longer
                // be shown, and carrying on with a key of our own would leave this
                // device unable to read the account's notes (and vice versa).
                // Server failures are not swallowed either: they stop setup here.
                //
                // Only the encryption password is ever tried: the server knows
                // the server password, so it could wrap a key of its own with it.
                var serverKeyData: Data?
                var onboardingContext: (client: SyncClient, userId: String)?
                if let syncRepo = appState.syncRepo,
                   let syncMeta = try? syncRepo.getMetadata(),
                   let userId = syncMeta.userId, !userId.isEmpty,
                   let endpoint = appState.settings.syncEndpoint,
                   let apiKey = registeredApiKey {
                    let client = SyncClient(endpoint: endpoint, apiKey: apiKey)
                    onboardingContext = (client, userId)
                    serverKeyData = try await EnvelopeService.accountKeyForNewVault(
                        syncClient: client,
                        userId: userId,
                        notesPassword: encryptionPassword,
                        accountPassword: password
                    )
                } else if let violation = PasswordPolicy.violation(
                    newNotesPassword: encryptionPassword, accountPassword: password
                ) {
                    throw violation == .tooShort
                        ? SyncSetupError.notesPasswordTooShort
                        : SyncSetupError.notesPasswordMatchesAccountPassword
                }

                syncProgress = L.setupProgressCreatingVault
                try await appState.createVault(password: encryptionPassword)

                // createVault() sets isFirstLaunch=false, which transitions
                // the UI to NoteListView. From here on, update appState
                // properties so progress is visible on the NoteListView
                // status bar (this view's @State is about to become invisible).
                syncProgress = L.setupProgressSettingUp
                appState.isSyncing = true
                appState.syncStatusMessage = L.setupProgressSettingUp

                // Envelope onboarding, using the key fetched above. This MUST
                // happen before setupSync, which triggers a background sync that
                // would race with it. Adopting is purely local, so it cannot
                // fail for a server reason now.
                if let context = onboardingContext, let encryptionRepo = appState.encryptionRepo {
                    if let serverKeyData {
                        do {
                            _ = try EnvelopeService.adoptServerKey(
                                serverKeyData,
                                encryptionRepo: encryptionRepo,
                                localPassword: encryptionPassword
                            )
                            appState.keyManager.unlockWithKeyData(serverKeyData)
                            try appState.syncRepo?.markServerKeyOpened()
                            if PasswordPolicy.matchesAccountPassword(encryptionPassword, accountPassword: password) {
                                appState.accountKeyNotice = .passwordMatchesAccount
                            }
                        } catch {
                            // Never sync with a key of our own: stop here.
                            Log.debug("[Envelope] Adopting server key failed: \(error)")
                            appState.syncError = SyncEndpoint.describeSyncFailure(error)
                            appState.syncStatusMessage = nil
                            appState.isSyncing = false
                            syncProgress = nil
                            isWorking = false
                            return
                        }
                    } else if let masterKey = appState.keyManager.masterKey {
                        // First device — no server key yet. Publish this vault's
                        // key, wrapped with the encryption password, so other
                        // devices can connect. Failure is retried on next unlock.
                        Log.debug("[Envelope] No server key — this device is the first, publishing ours")
                        do {
                            try await EnvelopeService.uploadWrappedKey(
                                syncClient: context.client,
                                userId: context.userId,
                                password: encryptionPassword,
                                masterKeyData: masterKey.withUnsafeBytes { Data($0) },
                                as: .firstUpload
                            )
                            try appState.syncRepo?.markServerKeyOpened()
                        } catch {
                            Log.debug("[Envelope] Publishing key failed (will retry on unlock): \(error)")
                        }
                    }
                }

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
                try await appState.loadNotes()
                appState.lastSyncAt = Date()
                appState.isSyncing = false

                let count = appState.notes.count
                appState.syncStatusMessage = L.syncStatusNotes(count)
                isWorking = false

                // Clear the status message after a few seconds
                try? await Task.sleep(for: .seconds(4))
                appState.syncStatusMessage = nil
            } catch {
                self.error = SyncEndpoint.describeSyncFailure(error)
                appState.syncError = SyncEndpoint.describeSyncFailure(error)
                appState.syncStatusMessage = nil
                appState.isSyncing = false
                syncProgress = nil
                isWorking = false
                // Only a wrong guess at the real password counts towards offering
                // to start over, not a password the rules refuse or a failed request.
                guard case SyncSetupError.encryptionPasswordMismatch = error else { return }
                failedAttempts += 1
                if failedAttempts >= 5 {
                    self.error = L.unlockFailedAttempts
                }
            }
        }
    }
}
