import SwiftUI

/// When the last backup was, and whether to be reminded about the next one.
struct BackupReminderSection: View {
    @Environment(AppState.self) private var appState
    @State private var notificationsBlocked = false

    var body: some View {
        Section {
            LabeledContent(L.backupLastBackup, value: lastBackup)

            Picker(L.backupReminder, selection: frequency) {
                ForEach(BackupReminderFrequency.allCases) { frequency in
                    Text(frequency.displayName).tag(frequency)
                }
            }
        } footer: {
            if notificationsBlocked {
                VStack(alignment: .leading, spacing: 4) {
                    Text(L.backupReminderBlocked)
                    Button(L.backupReminderOpenSettings, action: openSystemSettings)
                        .font(.footnote)
                }
            } else {
                Text(L.backupReminderHint)
            }
        }
        .task {
            // It can be turned off in the system's Settings at any time.
            guard appState.backupReminderFrequency != .off else { return }
            notificationsBlocked = await !appState.backupReminders.isAllowed()
        }
    }

    private var lastBackup: String {
        appState.settings.lastBackupDate?.formatted(date: .abbreviated, time: .shortened) ?? L.backupNeverBackedUp
    }

    /// Follows what was saved, so a choice that's refused doesn't stay selected.
    private var frequency: Binding<BackupReminderFrequency> {
        Binding(
            get: { appState.backupReminderFrequency },
            set: { choice in
                Task { @MainActor in
                    notificationsBlocked = await !appState.setBackupReminder(choice)
                }
            }
        )
    }

    private func openSystemSettings() {
        if let url = URL(string: UIApplication.openNotificationSettingsURLString) {
            UIApplication.shared.open(url)
        }
    }
}
