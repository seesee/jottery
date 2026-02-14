import SwiftUI

struct SettingsView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss
    @State private var settings: UserSettings = .defaults

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
                Section("Sync") {
                    Toggle("Sync Enabled", isOn: $settings.syncEnabled)

                    if settings.syncEnabled {
                        TextField("Server URL", text: Binding(
                            get: { settings.syncEndpoint ?? "" },
                            set: { settings.syncEndpoint = $0.isEmpty ? nil : $0 }
                        ))
                        .textContentType(.URL)
                        .autocorrectionDisabled()
                        .textInputAutocapitalization(.never)

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
                    }
                }

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
        }
    }
}
