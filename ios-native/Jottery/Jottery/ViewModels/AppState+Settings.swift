import Foundation

extension AppState {

    // MARK: - Restoring on First Run

    /// Make the vault from a backup, for a device that has none, and open it.
    /// `password` is the backup's, and from now on the vault's. Throws, with
    /// nothing changed, unless all of it worked (`VaultFromBackup`). Sync, if the
    /// device is already registered, starts at the next unlock, as it does for a
    /// new vault, and that's when a key the account doesn't share comes to light.
    @discardableResult
    func restoreVault(
        fromBackup url: URL,
        password: String,
        progress: (@Sendable (BackupService.Progress) -> Void)? = nil
    ) async throws -> BackupService.RestoreResult {
        guard let db else { throw AppStateError.notInitialised }

        var outcome = try await offMainActor {
            try VaultFromBackup.restore(from: url, password: password, into: db, progress: progress)
        }
        defer { outcome.masterKey.wipe() }

        keyManager.unlockWithKeyData(outcome.masterKey)
        isFirstLaunch = false
        isLocked = false

        // The vault is whole by now, so a failed load doesn't undo the restore.
        do {
            try await loadNotes()
        } catch {
            Log.debug("[Restore] Couldn't load the restored notes: \(error)")
        }
        return outcome.result
    }

    // MARK: - Backup Reminders

    var backupReminderFrequency: BackupReminderFrequency {
        settings.backupReminderDays.flatMap(BackupReminderFrequency.init(rawValue:)) ?? .off
    }

    /// Set how often to be reminded. Turning it on is the only thing that ever
    /// asks to show notifications. Returns false if they aren't allowed, and
    /// the reminder then stays as it was.
    @discardableResult
    func setBackupReminder(_ frequency: BackupReminderFrequency) async -> Bool {
        guard frequency != .off else {
            backupReminders.cancel()
            saveBackupSettings { $0.backupReminderDays = nil; $0.backupReminderSince = nil }
            return true
        }
        guard await backupReminders.requestPermission() else { return false }
        let since = settings.backupReminderSince ?? Date().iso8601
        saveBackupSettings {
            $0.backupReminderDays = frequency.rawValue
            $0.backupReminderSince = since
        }
        await refreshBackupReminder()
        return true
    }

    /// Note that a backup has left the device. Making the file doesn't count:
    /// it isn't a backup until the share sheet has put it somewhere.
    func recordBackupMade(at date: Date = Date()) {
        saveBackupSettings { $0.lastBackupAt = date.iso8601 }
        Task { await refreshBackupReminder() }
    }

    /// Set the reminder for its next date, if there is a reminder.
    func refreshBackupReminder(now: Date = Date()) async {
        let frequency = backupReminderFrequency
        guard frequency != .off else { return }
        let anchor = settings.lastBackupDate
            ?? settings.backupReminderSince.flatMap { Date(iso8601: $0) }
            ?? now
        let date = BackupReminder.nextDate(anchor: anchor, everyDays: frequency.rawValue, after: now)
        do {
            try await backupReminders.schedule(
                title: L.backupReminderTitle, body: L.backupReminderBody, at: date
            )
        } catch {
            Log.debug("[Backup] Couldn't schedule the reminder: \(error)")
        }
    }

    private func saveBackupSettings(_ change: (inout UserSettings) -> Void) {
        var updated = settings
        change(&updated)
        do {
            try updateSettings(updated)
        } catch {
            reportError(L.errorCouldntSaveSettings)
        }
    }
}
