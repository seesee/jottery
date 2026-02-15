import SwiftUI

struct SettingsView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss
    @State private var settings: UserSettings = .defaults
    @State private var showSyncSetup = false
    @State private var showWipeConfirmation = false
    @State private var showImport = false
    @State private var exportFile: ExportFile?

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

                    Picker(L.settingsLanguage, selection: $settings.language) {
                        Text(L.settingsLanguageSystem).tag("system")
                        Text(L.settingsLanguageEnGB).tag("en-GB")
                        Text(L.settingsLanguageEnUS).tag("en-US")
                    }
                }

                // Biometrics
                if appState.keyManager.isBiometricAvailable {
                    Section(L.settingsSecurity) {
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
                }

                // Sync
                syncSection

                // Data
                Section(L.settingsData) {
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
                        Text("1.0.0")
                            .foregroundStyle(.secondary)
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

                // Debug
                Section(L.settingsDebug) {
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
            .sheet(isPresented: $showImport) {
                ImportView()
            }
            .sheet(item: $exportFile) { file in
                ActivityView(activityItems: [file.url])
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

                Button(L.settingsSyncDisconnect, role: .destructive) {
                    disconnectSync()
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

    private func disconnectSync() {
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
