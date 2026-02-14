import SwiftUI

struct SettingsView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss
    @State private var settings: UserSettings = .defaults
    @State private var showSyncSetup = false

    var body: some View {
        NavigationStack {
            Form {
                // General
                Section("General") {
                    Picker("Theme", selection: $settings.theme) {
                        Text("System").tag("auto")
                        Text("Light").tag("light")
                        Text("Dark").tag("dark")
                    }

                    Picker("Sort Order", selection: $settings.sortOrder) {
                        ForEach(SortOrder.allCases, id: \.rawValue) { order in
                            Text(order.displayName).tag(order.rawValue)
                        }
                    }

                    Picker("Auto-lock", selection: $settings.autoLockTimeout) {
                        Text("1 minute").tag(1)
                        Text("5 minutes").tag(5)
                        Text("15 minutes").tag(15)
                        Text("30 minutes").tag(30)
                        Text("1 hour").tag(60)
                        Text("Never").tag(0)
                    }
                }

                // Biometrics
                if appState.keyManager.isBiometricAvailable {
                    Section("Security") {
                        Toggle("Face ID / Touch ID", isOn: Binding(
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

                // About
                Section("About") {
                    HStack {
                        Text("Notes")
                        Spacer()
                        Text("\(appState.noteCount)")
                            .foregroundStyle(.secondary)
                    }

                    HStack {
                        Text("Version")
                        Spacer()
                        Text("1.0.0")
                            .foregroundStyle(.secondary)
                    }
                }

                // Lock
                Section {
                    Button("Lock Now") {
                        dismiss()
                        appState.lock()
                    }
                    .foregroundStyle(.red)
                }
            }
            .navigationTitle("Settings")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") {
                        try? appState.updateSettings(settings)
                        dismiss()
                    }
                }
            }
            .onAppear {
                settings = appState.settings
            }
            .sheet(isPresented: $showSyncSetup) {
                SyncSetupView(onComplete: {
                    // Reload settings from DB after registration
                    if let loaded = try? appState.settingsRepo?.get() {
                        settings = loaded
                        appState.settings = loaded
                    }
                    appState.setupSync()
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
            Section("Sync") {
                HStack {
                    Text("Server")
                    Spacer()
                    Text(appState.settings.syncEndpoint ?? "")
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }

                if let lastSync = appState.lastSyncAt {
                    HStack {
                        Text("Last Sync")
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
                        Text("Sync Now")
                        Spacer()
                        if appState.isSyncing {
                            ProgressView()
                                .controlSize(.small)
                        }
                    }
                }
                .disabled(appState.isSyncing)

                Button("Disconnect", role: .destructive) {
                    disconnectSync()
                }
            }
        } else {
            // Sync is not configured — show setup button
            Section("Sync") {
                Button {
                    showSyncSetup = true
                } label: {
                    Label("Set Up Sync", systemImage: "arrow.triangle.2.circlepath")
                }
            }
        }
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
