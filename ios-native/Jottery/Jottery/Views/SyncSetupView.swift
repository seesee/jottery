import SwiftUI

/// Sync setup for a device that already has a vault: register it with the
/// sync server using the account's email and password.
struct SyncSetupView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var scenePhase

    /// Called with the API key after successful registration.
    var onComplete: ((_ apiKey: String) -> Void)?

    @State private var endpoint = ""
    @State private var email = ""
    @State private var password = ""
    @State private var notesPassword = ""
    @State private var deviceName = ""
    @State private var error: String?
    @State private var isWorking = false
    @State private var success = false
    @State private var registeredApiKey: String?
    /// Set while the user is asked whether to merge this device's notes into
    /// an account that already has a different key.
    @State private var pendingMerge: PendingMerge?
    @State private var showMergePrompt = false
    @State private var isNewAccount = false
    /// The account exists, and is waiting for an administrator.
    @State private var isAwaitingApproval = false

    var body: some View {
        NavigationStack {
            Form {
                registerSection

                if isAwaitingApproval {
                    awaitingApprovalSection
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
                    // Not while a device is being registered or a merge runs:
                    // dismissing then would leave the new device on the account.
                    Button(L.syncSetupCancel) { dismiss() }
                        .disabled(isWorking)
                }
            }
            .interactiveDismissDisabled(isWorking)
            // Approval happens somewhere else, so this is when it may have come.
            .onChange(of: scenePhase) { _, phase in
                if phase == .active { checkApproval() }
            }
            .alert(L.syncMergeTitle, isPresented: $showMergePrompt, presenting: pendingMerge) { pending in
                Button(L.syncMergeConfirm, role: .destructive) { mergeAndJoin(pending) }
                Button(L.syncSetupCancel, role: .cancel) { declineMerge(pending) }
            } message: { pending in
                Text(L.syncMergeMessage(
                    server: SyncEndpoint.displayHost(pending.device.endpoint),
                    noteCount: appState.notes.count + appState.archivedNotes.count
                ))
            }
            .onDisappear {
                // Never leave a device registered for a join that didn't happen.
                if let pending = pendingMerge {
                    Task { try? await pending.device.client.revokeThisDevice() }
                }
            }
        }
    }

    private var registerSection: some View {
        Section(L.syncSetupRegisterDevice) {
            Picker(L.syncSetupAccountChoice, selection: $isNewAccount) {
                Text(L.syncSetupExistingAccount).tag(false)
                Text(L.syncSetupNewAccount).tag(true)
            }
            .pickerStyle(.segmented)
            .disabled(isAwaitingApproval)

            TextField(L.syncSetupServerUrl, text: $endpoint)
                .textContentType(.URL)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)

            TextField(L.syncSetupEmail, text: $email)
                .textContentType(.emailAddress)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)

            PasswordField(L.syncSetupServerPassword, text: $password)

            TextField(L.syncSetupDeviceName, text: $deviceName)

            Text(L.syncSetupNotesPasswordHint)
                .font(.caption)
                .foregroundStyle(.secondary)

            PasswordField(L.syncSetupNotesPassword, text: $notesPassword)

            // On an existing account this is a warning and no more: refusing it
            // would stop these users adding a device. A new account's password is
            // being chosen now, so that's refused, by `createAccount`.
            if PasswordPolicy.matchesAccountPassword(notesPassword, accountPassword: password) {
                Label(L.syncSetupPasswordsMatchWarning, systemImage: "exclamationmark.triangle")
                    .font(.caption)
                    .foregroundStyle(.orange)
            }

            Button(action: submit) {
                if isWorking {
                    ProgressView()
                } else {
                    Text(isNewAccount ? L.syncSetupCreateAccountAction : L.syncSetupRegisterAction)
                }
            }
            .disabled(!canSubmit)
        }
    }

    private var canSubmit: Bool {
        let filledIn = ![endpoint, email, password, deviceName, notesPassword].contains(where: \.isEmpty)
        return filledIn && !isWorking && !success && !isAwaitingApproval
    }

    private func submit() {
        if isNewAccount {
            createAccount()
        } else {
            registerDevice()
        }
    }

    private var awaitingApprovalSection: some View {
        Section {
            Label(L.syncSetupAwaitingApproval, systemImage: "hourglass")
            Button(L.syncSetupCheckApproval, action: checkApproval)
                .disabled(isWorking)
        }
    }

    private func registerDevice() {
        run {
            // Verify the notes (vault) password against the local vault
            // BEFORE registering — this is the password that ends up
            // wrapping the envelope master key, and getting it wrong
            // would silently upload a key other devices can't unwrap
            // (jottery-md6b). Registration must not proceed on failure.
            guard await appState.verifyVaultPassword(notesPassword) else {
                throw SyncSetupError.wrongVaultPassword
            }
            try await connectDevice(endpoint: SyncEndpoint.normalise(endpoint))
        }
    }

    /// Create the account, then connect this device, now or once it's approved.
    private func createAccount() {
        run {
            // The account password is being chosen now, and the vault's notes
            // password is known. Checked before either goes anywhere.
            guard !PasswordPolicy.matchesAccountPassword(notesPassword, accountPassword: password) else {
                throw SyncSetupError.notesPasswordMatchesAccountPassword
            }
            guard await appState.verifyVaultPassword(notesPassword) else {
                throw SyncSetupError.wrongVaultPassword
            }
            let normalised = try SyncEndpoint.normalise(endpoint)
            let account = try await SyncClient(endpoint: normalised).registerUser(email: email, password: password)
            if account.isApproved {
                try await connectDevice(endpoint: normalised)
            } else {
                isAwaitingApproval = true
            }
        }
    }

    /// See whether the account has been approved, and carry on if it has.
    private func checkApproval() {
        guard isAwaitingApproval, !isWorking else { return }
        run {
            let normalised = try SyncEndpoint.normalise(endpoint)
            let status = try await SyncClient(endpoint: normalised).userStatus(email: email, password: password)
            if status.isApproved {
                isAwaitingApproval = false
                try await connectDevice(endpoint: normalised)
            }
        }
    }

    /// Run a step of setting up, showing what went wrong if it fails.
    private func run(_ step: @escaping @MainActor () async throws -> Void) {
        error = nil
        isWorking = true
        Task {
            do {
                try await step()
            } catch {
                self.error = error.localizedDescription
            }
            isWorking = false
        }
    }

    private func connectDevice(endpoint normalised: String) async throws {
        let response = try await SyncClient(endpoint: normalised).registerDevice(
            email: email,
            password: password,
            deviceName: deviceName
        )
        let device = NewDevice(endpoint: normalised, response: response)
        success = try await join(device, notesPassword: notesPassword)
    }

    // MARK: - Joining

    /// Join the account with a device registered moments ago: check the
    /// account's key, then save everything. Returns false instead when the
    /// account's key differs and the user is being asked whether to merge
    /// (`mergeAndJoin` / `declineMerge` take it from there).
    private func join(_ device: NewDevice, notesPassword: String) async throws -> Bool {
        if case .differs(let accountKey) = try await checkAccountKeyBeforeJoining(
            device, notesPassword: notesPassword
        ) {
            pendingMerge = PendingMerge(device: device, accountKey: accountKey, notesPassword: notesPassword)
            showMergePrompt = true
            return false
        }
        try await completeJoin(device, notesPassword: notesPassword)
        return true
    }

    /// Save the new device's credentials and turn sync on, and publish this
    /// vault's key if the account has none.
    private func completeJoin(_ device: NewDevice, notesPassword: String) async throws {
        try KeychainService.storeAPIKey(device.apiKey)
        try KeychainService.storeClientId(device.clientId)
        try appState.settingsRepo?.updateSync(enabled: true, endpoint: device.endpoint)

        // Store userId for envelope encryption
        try appState.syncRepo?.beginAccount(userId: device.userId)

        appState.settings.syncEnabled = true
        appState.settings.syncEndpoint = device.endpoint
        registeredApiKey = device.apiKey

        await publishAccountKey(deviceClient: device.client, notesPassword: notesPassword)
    }

    /// The user chose to merge: re-encrypt this vault onto the account's key
    /// (all or nothing), then finish joining. Sync then uploads this device's
    /// notes, which are all flagged for it, and downloads the account's.
    private func mergeAndJoin(_ pending: PendingMerge) {
        pendingMerge = nil
        error = nil
        isWorking = true
        Task {
            do {
                try await appState.switchVaultToMasterKey(
                    pending.accountKey, notesPassword: pending.notesPassword
                )
            } catch {
                // Nothing on this device changed; don't leave the device behind.
                try? await pending.device.client.revokeThisDevice()
                self.error = L.syncMergeFailed
                isWorking = false
                return
            }
            do {
                try await completeJoin(pending.device, notesPassword: pending.notesPassword)
                success = true
            } catch {
                // The vault is on the account's key now, so trying again simply
                // finds that the keys match.
                try? await pending.device.client.revokeThisDevice()
                self.error = error.localizedDescription
            }
            isWorking = false
        }
    }

    /// The user declined the merge: revoke the new device and explain the
    /// manual way to join (export the notes, reset, connect, import the notes).
    private func declineMerge(_ pending: PendingMerge) {
        pendingMerge = nil
        error = SyncSetupError.accountKeyDiffers.localizedDescription
        Task { try? await pending.device.client.revokeThisDevice() }
    }

    // MARK: - Account key

    /// A device that already has a vault must share the account's master key,
    /// or syncing would leave notes unreadable in both directions. Compare them
    /// straight after the device is registered, BEFORE anything is saved
    /// locally. A failed check revokes the device again; `.differs` keeps it,
    /// for `join` to offer the merge. Only the notes password is ever tried.
    private func checkAccountKeyBeforeJoining(
        _ device: NewDevice,
        notesPassword: String
    ) async throws -> EnvelopeService.AccountKeyStatus? {
        guard let masterKey = appState.keyManager.masterKey else { return nil }
        do {
            return try await EnvelopeService.checkAccountKeyForNewDevice(
                syncClient: device.client,
                userId: device.userId,
                password: notesPassword,
                localMasterKeyData: masterKey.withUnsafeBytes { Data($0) }
            )
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

/// A device registered on the account moments ago, not yet saved locally.
private struct NewDevice {
    let endpoint: String
    let apiKey: String
    let clientId: String
    let userId: String
    let client: SyncClient

    init(endpoint: String, response: RegisterDeviceResponse) {
        self.endpoint = endpoint
        apiKey = response.apiKey
        clientId = response.clientId
        userId = response.userId
        client = SyncClient(endpoint: endpoint, apiKey: response.apiKey)
    }
}

/// The account's key differs from this vault's, and the user is being asked
/// whether to merge this device's notes into the account.
private struct PendingMerge {
    let device: NewDevice
    let accountKey: Data
    let notesPassword: String
}

enum SyncSetupError: LocalizedError {
    case wrongVaultPassword
    case encryptionPasswordMismatch
    case accountKeyDiffers
    case accountKeyUnreadable
    case notesPasswordMatchesAccountPassword
    case notesPasswordTooShort

    var errorDescription: String? {
        switch self {
        case .wrongVaultPassword: return L.syncSetupErrorWrongVaultPassword
        case .encryptionPasswordMismatch: return L.syncSetupErrorEncryptionPasswordMismatch
        case .accountKeyDiffers: return L.syncSetupErrorAccountKeyDiffers
        case .accountKeyUnreadable: return L.syncSetupErrorAccountKeyUnreadable
        case .notesPasswordMatchesAccountPassword: return L.syncSetupErrorNotesPasswordMatchesAccount
        case .notesPasswordTooShort: return L.notesPasswordTooShort(PasswordPolicy.minimumNotesPasswordLength)
        }
    }
}
