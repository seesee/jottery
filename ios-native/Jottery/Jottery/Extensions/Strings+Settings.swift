import Foundation

extension L {

    // MARK: - Restoring on First Run

    static var setupRestoreFromBackup: String { String(localized: "setup.restoreFromBackup") }
    static var setupRestoreTitle: String { String(localized: "setup.restoreTitle") }
    static var setupRestoreFile: String { String(localized: "setup.restoreFile") }
    static var setupRestoreHint: String { String(localized: "setup.restoreHint") }
    static var setupRestoreAction: String { String(localized: "setup.restoreAction") }
    static var restoreVaultExists: String { String(localized: "restore.vaultExists") }
    static var backupErrorNothingToRestore: String { String(localized: "backupError.nothingToRestore") }

    // MARK: - Backup Reminders

    static var backupLastBackup: String { String(localized: "backup.lastBackup") }
    static var backupNeverBackedUp: String { String(localized: "backup.neverBackedUp") }
    static var backupReminder: String { String(localized: "backup.reminder") }
    static var backupReminderOff: String { String(localized: "backup.reminderOff") }
    static var backupReminderDaily: String { String(localized: "backup.reminderDaily") }
    static var backupReminderWeekly: String { String(localized: "backup.reminderWeekly") }
    static var backupReminderMonthly: String { String(localized: "backup.reminderMonthly") }
    static var backupReminderHint: String { String(localized: "backup.reminderHint") }
    static var backupReminderBlocked: String { String(localized: "backup.reminderBlocked") }
    static var backupReminderOpenSettings: String { String(localized: "backup.reminderOpenSettings") }
    static var backupReminderTitle: String { String(localized: "backup.reminderTitle") }
    static var backupReminderBody: String { String(localized: "backup.reminderBody") }

    // MARK: - Vault Health

    static var healthTitle: String { String(localized: "health.title") }
    static var healthScanning: String { String(localized: "health.scanning") }
    static var healthScanAgain: String { String(localized: "health.scanAgain") }
    static var healthScanFailed: String { String(localized: "health.scanFailed") }
    static var healthAllGood: String { String(localized: "health.allGood") }
    static var healthProblemsFound: String { String(localized: "health.problemsFound") }
    static func healthNotesChecked(_ count: Int) -> String { String(localized: "health.notesChecked \(count)") }
    static var healthUnknownDate: String { String(localized: "health.unknownDate") }
    static func healthNoteInBin(_ id: String) -> String { String(localized: "health.noteInBin \(id)") }
    static var healthDeleteNote: String { String(localized: "health.deleteNote") }
    static var healthDeleteFailed: String { String(localized: "health.deleteFailed") }
    static var healthDeleteConfirmTitle: String { String(localized: "health.deleteConfirmTitle") }
    static var healthDeleteConfirmMessage: String { String(localized: "health.deleteConfirmMessage") }
    static var healthDeleteConfirmAction: String { String(localized: "health.deleteConfirmAction") }
    static var healthUnreadableHeader: String { String(localized: "health.unreadableHeader") }
    static var healthUnreadableFooter: String { String(localized: "health.unreadableFooter") }
    static var healthUnreadableNoteTitle: String { String(localized: "health.unreadableNoteTitle") }
    static var healthMissingHeader: String { String(localized: "health.missingHeader") }
    static var healthMissingFooter: String { String(localized: "health.missingFooter") }
    static var healthMissingFooterNoSync: String { String(localized: "health.missingFooterNoSync") }
    static var healthRepairFromServer: String { String(localized: "health.repairFromServer") }
    static var healthRepairNotOnServer: String { String(localized: "health.repairNotOnServer") }
    static var healthRepairUnreadable: String { String(localized: "health.repairUnreadable") }
    static var healthRepairFailed: String { String(localized: "health.repairFailed") }
    static var healthServerHeader: String { String(localized: "health.serverHeader") }
    static var healthServerFooter: String { String(localized: "health.serverFooter") }
    static var healthCheckServer: String { String(localized: "health.checkServer") }
    static var healthCheckingServer: String { String(localized: "health.checkingServer") }
    static var healthServerHasEverything: String { String(localized: "health.serverHasEverything") }
    static var healthSendToServer: String { String(localized: "health.sendToServer") }
    static var healthUploadNotKept: String { String(localized: "health.uploadNotKept") }
    static var healthUploadUnreadable: String { String(localized: "health.uploadUnreadable") }
}
