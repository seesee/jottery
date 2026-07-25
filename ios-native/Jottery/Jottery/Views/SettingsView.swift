import SwiftUI

struct SettingsView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss
    @State private var settings: UserSettings = .defaults
    @State private var showSyncSetup = false
    @State private var showWipeConfirmation = false
    @State private var showImport = false
    @State private var showChangePassword = false
    @State private var showBackup = false
    @State private var showForceSyncConfirmation = false
    @State private var exportFile: ExportFile?
    @State private var isDisconnecting = false
    @State private var disconnectWarning: String?
    @State private var showDeleteAccount = false

    private var appVersion: String {
        let version = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "?"
        let build = Bundle.main.infoDictionary?["CFBundleVersion"] as? String
        return build.map { "\(version) (\($0))" } ?? version
    }

    var body: some View {
        NavigationStack {
            Form {
                // General
                Section(L.settingsGeneral) {
                    Picker(L.settingsTheme, selection: $settings.theme) {
                        Text(L.settingsThemeSystem).tag("auto")
                        Text(L.settingsThemeLight).tag("light")
                        Text(L.settingsThemeDark).tag("dark")
                    }

                    Picker(L.settingsSortOrder, selection: $settings.sortOrder) {
                        ForEach(SortOrder.allCases, id: \.rawValue) { order in
                            Text(order.displayName).tag(order.rawValue)
                        }
                    }

                    Picker(L.settingsAutoLock, selection: $settings.autoLockTimeout) {
                        Text(L.settingsAutoLock1Min).tag(1)
                        Text(L.settingsAutoLock5Min).tag(5)
                        Text(L.settingsAutoLock15Min).tag(15)
                        Text(L.settingsAutoLock30Min).tag(30)
                        Text(L.settingsAutoLock1Hour).tag(60)
                        Text(L.settingsAutoLockNever).tag(0)
                    }

                    // Only en-GB is translated. Offering en-US changed date and
                    // number formatting but left the copy in British English,
                    // which reads as a bug; "System" already covers users who
                    // want their own regional formats.
                    Picker(L.settingsLanguage, selection: $settings.language) {
                        Text(L.settingsLanguageSystem).tag("system")
                        Text(L.settingsLanguageEnGB).tag("en-GB")
                    }
                }

                // Security
                Section(L.settingsSecurity) {
                    if appState.keyManager.isBiometricAvailable {
                        Toggle(L.settingsBiometric, isOn: Binding(
                            get: { appState.keyManager.isBiometricEnabled },
                            set: { enabled in
                                if enabled {
                                    try? appState.keyManager.enableBiometricUnlock()
                                } else {
                                    appState.keyManager.disableBiometricUnlock()
                                }
                            }
                        ))
                    }

                    Button {
                        showChangePassword = true
                    } label: {
                        Label(L.changePasswordTitle, systemImage: "key")
                    }
                }

                // Sync
                syncSection

                // Data
                Section(L.settingsData) {
                    Button {
                        showBackup = true
                    } label: {
                        Label(L.backupTitle, systemImage: "externaldrive.badge.shield")
                    }

                    Button {
                        exportAllNotes()
                    } label: {
                        Label(L.settingsExportAll, systemImage: "square.and.arrow.up")
                    }

                    Button {
                        showImport = true
                    } label: {
                        Label(L.settingsImport, systemImage: "square.and.arrow.down")
                    }
                }

                // About
                Section(L.settingsAbout) {
                    HStack {
                        Text(L.settingsNotes)
                        Spacer()
                        Text("\(appState.noteCount)")
                            .foregroundStyle(.secondary)
                    }

                    HStack {
                        Text(L.settingsVersion)
                        Spacer()
                        Text(appVersion)
                            .foregroundStyle(.secondary)
                    }

                    Link(destination: URL(string: "https://jottery.org/privacy")!) {
                        Label(L.settingsPrivacyPolicy, systemImage: "hand.raised")
                    }

                    Link(destination: URL(string: "https://github.com/seesee/jottery/issues")!) {
                        Label(L.settingsSupport, systemImage: "questionmark.circle")
                    }
                }

                // Lock
                Section {
                    Button(L.settingsLockNow) {
                        dismiss()
                        appState.lock()
                    }
                    .foregroundStyle(.red)
                }

                // Reset
                Section(L.settingsReset) {
                    Button(L.settingsWipeAllData, role: .destructive) {
                        showWipeConfirmation = true
                    }
                }
            }
            .navigationTitle(L.settingsTitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button(L.settingsDone) {
                        try? appState.updateSettings(settings)
                        dismiss()
                    }
                }
            }
            .onAppear {
                settings = appState.settings
            }
            .alert(L.settingsWipeConfirmTitle, isPresented: $showWipeConfirmation) {
                Button(L.settingsWipeConfirmAction, role: .destructive) {
                    dismiss()
                    wipeAllData()
                }
                Button(L.commonCancel, role: .cancel) {}
            } message: {
                Text(L.settingsWipeConfirmMessage)
            }
            .alert(L.forceSyncTitle, isPresented: $showForceSyncConfirmation) {
                Button(L.forceSyncAction, role: .destructive) {
                    Task { @MainActor in await appState.forceFullSync() }
                }
                Button(L.commonCancel, role: .cancel) {}
            } message: {
                Text(L.forceSyncMessage)
            }
            .sheet(isPresented: $showImport) {
                ImportView()
            }
            .sheet(item: $exportFile) { file in
                ActivityView(activityItems: [file.url])
            }
            .sheet(isPresented: $showChangePassword) {
                ChangePasswordView()
            }
            .sheet(isPresented: $showBackup) {
                BackupView()
            }
            .sheet(isPresented: $showDeleteAccount) {
                DeleteAccountView(onDeleted: {
                    Task { await disconnectLocallyAfterAccountDeletion() }
                })
            }
            .sheet(isPresented: $showSyncSetup) {
                SyncSetupView(onComplete: { apiKey in
                    // Reload settings from DB after registration
                    if let loaded = try? appState.settingsRepo?.get() {
                        settings = loaded
                        appState.settings = loaded
                    }
                    appState.setupSync(apiKey: apiKey)
                    // Trigger initial sync
                    Task { await appState.triggerSync() }
                })
            }
        }
    }

    // MARK: - Sync Section

    @ViewBuilder
    private var syncSection: some View {
        if appState.syncEnabled {
            // Sync is configured — show status and controls
            Section(L.settingsSync) {
                HStack {
                    Text(L.settingsSyncServer)
                    Spacer()
                    Text(appState.settings.syncEndpoint ?? "")
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }

                if let lastSync = appState.lastSyncAt {
                    HStack {
                        Text(L.settingsSyncLastSync)
                        Spacer()
                        Text(lastSync.relativeDescription)
                            .foregroundStyle(.secondary)
                    }
                }

                if let error = appState.syncError {
                    Text(error)
                        .foregroundStyle(.red)
                        .font(.caption)
                }

                Button {
                    Task { @MainActor in await appState.triggerSync() }
                } label: {
                    HStack {
                        Text(L.settingsSyncNow)
                        Spacer()
                        if appState.isSyncing {
                            ProgressView()
                                .controlSize(.small)
                        }
                    }
                }
                .disabled(appState.isSyncing)

                Button {
                    showForceSyncConfirmation = true
                } label: {
                    Text(L.forceSyncTitle)
                }
                .disabled(appState.isSyncing)

                Button(L.settingsSyncDisconnect, role: .destructive) {
                    Task { await disconnectSync() }
                }
                .disabled(isDisconnecting)

                if let disconnectWarning {
                    Text(disconnectWarning)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                Button(L.deleteAccountTitle, role: .destructive) {
                    showDeleteAccount = true
                }
            }
        } else {
            // Sync is not configured — show setup button
            Section(L.settingsSync) {
                Button {
                    showSyncSetup = true
                } label: {
                    Label(L.settingsSyncSetUp, systemImage: "arrow.triangle.2.circlepath")
                }
            }
        }
    }

    private func wipeAllData() {
        appState.wipeAllData()
    }

    private func exportAllNotes() {
        guard let key = appState.keyManager.masterKey else { return }
        let allNotes = appState.notes + appState.archivedNotes
        guard let data = try? ExportService.exportAll(
            notes: allNotes,
            attachmentRepo: appState.attachmentRepo,
            key: key
        ) else { return }

        let filename = ExportService.defaultFilename()
        let tempURL = FileManager.default.temporaryDirectory.appendingPathComponent(filename)
        try? data.write(to: tempURL)
        exportFile = ExportFile(url: tempURL)
    }

    /// Clear local sync state after the server account has been deleted.
    ///
    /// No revoke call: the account is gone, so the device credentials are dead
    /// already and the request would only fail. Notes stay on the device — the
    /// user deleted their sync account, not their notes.
    private func disconnectLocallyAfterAccountDeletion() async {
        KeychainService.deleteAPIKey()
        KeychainService.deleteClientId()
        try? appState.settingsRepo?.updateSync(enabled: false, endpoint: nil)

        settings.syncEnabled = false
        settings.syncEndpoint = nil
        appState.settings.syncEnabled = false
        appState.settings.syncEndpoint = nil
        appState.syncEnabled = false
        appState.syncClient = nil
        appState.syncService = nil
    }

    /// Unlink this device, server-side first and then locally.
    ///
    /// The server call is best-effort: it is attempted before the credentials
    /// are destroyed (they are what authenticates it), but the local disconnect
    /// always proceeds. A user who is offline, or whose self-hosted server
    /// predates the endpoint, must still be able to disconnect — they are told
    /// the device is still registered so they can revoke it from the web UI.
    private func disconnectSync() async {
        let client = appState.syncClient
        isDisconnecting = true
        defer { isDisconnecting = false }

        if let client {
            do {
                try await client.revokeThisDevice()
            } catch {
                disconnectWarning = String(
                    format: L.syncDisconnectStillRegistered,
                    SyncEndpoint.describeSyncFailure(error)
                )
            }
        }

        KeychainService.deleteAPIKey()
        KeychainService.deleteClientId()
        try? appState.settingsRepo?.updateSync(enabled: false, endpoint: nil)

        settings.syncEnabled = false
        settings.syncEndpoint = nil
        appState.settings.syncEnabled = false
        appState.settings.syncEndpoint = nil
        appState.syncEnabled = false
        appState.syncClient = nil
        appState.syncService = nil
    }
}

// MARK: - Export File Wrapper

struct ExportFile: Identifiable {
    let id = UUID()
    let url: URL
}

// MARK: - Activity View (Share Sheet)

struct ActivityView: UIViewControllerRepresentable {
    let activityItems: [Any]

    func makeUIViewController(context: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: activityItems, applicationActivities: nil)
    }

    func updateUIViewController(_ uiViewController: UIActivityViewController, context: Context) {}
}
