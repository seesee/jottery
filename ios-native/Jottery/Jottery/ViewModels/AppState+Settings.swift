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
}
