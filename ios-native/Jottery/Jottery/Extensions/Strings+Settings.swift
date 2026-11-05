import Foundation

extension L {

    // MARK: - Restoring on First Run

    static var setupRestoreFromBackup: String { String(localized: "setup.restoreFromBackup") }
    static var setupRestoreTitle: String { String(localized: "setup.restoreTitle") }
    static var setupRestoreFile: String { String(localized: "setup.restoreFile") }
    static var setupRestoreChooseFile: String { String(localized: "setup.restoreChooseFile") }
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

    // MARK: - Note Types and Colours

    static var settingsDefaultNoteType: String { String(localized: "settings.defaultNoteType") }
    static var coloursTitle: String { String(localized: "colours.title") }
    static var coloursCategoriesHeader: String { String(localized: "colours.categoriesHeader") }
    static var coloursCategoriesFooter: String { String(localized: "colours.categoriesFooter") }
    static func coloursCategoryLabel(_ colour: String) -> String { String(localized: "colours.categoryLabel \(colour)") }
    static var coloursTagsHeader: String { String(localized: "colours.tagsHeader") }
    static var coloursTagsFooter: String { String(localized: "colours.tagsFooter") }
    static var coloursNoTags: String { String(localized: "colours.noTags") }
    static var coloursFilterTags: String { String(localized: "colours.filterTags") }

    static var healthServerCheckFailed: String { String(localized: "health.serverCheckFailed") }

    // MARK: - Sync Section

    static var settingsSyncDevice: String { String(localized: "settings.syncDevice") }
    static var settingsSyncPending: String { String(localized: "settings.syncPending") }

    // MARK: - Account

    static var accountTitle: String { String(localized: "account.title") }
    static var accountSignIn: String { String(localized: "account.signIn") }
    static var accountSignInHint: String { String(localized: "account.signInHint") }
    static var accountSessionEnded: String { String(localized: "account.sessionEnded") }
    static var accountRequestFailed: String { String(localized: "account.requestFailed") }
    static var accountUsage: String { String(localized: "account.usage") }
    static var accountAttachments: String { String(localized: "account.attachments") }
    static var accountStorage: String { String(localized: "account.storage") }
    static func accountStorageOfQuota(_ used: String, _ quota: String) -> String {
        String(localized: "account.storageOfQuota \(used) \(quota)")
    }
    static var accountInbox: String { String(localized: "account.inbox") }
    static var accountInboxHint: String { String(localized: "account.inboxHint") }
    static var accountInboxItems: String { String(localized: "account.inboxItems") }
    static var accountInboxToken: String { String(localized: "account.inboxToken") }
    static var accountInboxTokenSet: String { String(localized: "account.inboxTokenSet") }
    static var accountInboxTokenNotSet: String { String(localized: "account.inboxTokenNotSet") }
    static var accountGenerateToken: String { String(localized: "account.generateToken") }
    static var accountReplaceToken: String { String(localized: "account.replaceToken") }
    static var accountReplaceTokenTitle: String { String(localized: "account.replaceTokenTitle") }
    static var accountReplaceTokenMessage: String { String(localized: "account.replaceTokenMessage") }
    static var accountReplaceTokenAction: String { String(localized: "account.replaceTokenAction") }
    static var accountRevokeToken: String { String(localized: "account.revokeToken") }
    static var accountRevokeTokenTitle: String { String(localized: "account.revokeTokenTitle") }
    static var accountRevokeTokenMessage: String { String(localized: "account.revokeTokenMessage") }
    static var accountRevokeTokenAction: String { String(localized: "account.revokeTokenAction") }
    static var accountCopyToken: String { String(localized: "account.copyToken") }
    static var accountTokenCopied: String { String(localized: "account.tokenCopied") }
    static var accountTokenShownOnce: String { String(localized: "account.tokenShownOnce") }
    static var accountDeleteNotesTitle: String { String(localized: "account.deleteNotesTitle") }
    static func accountDeleteNotesMessage(_ count: Int) -> String {
        String(localized: "account.deleteNotesMessage \(count)")
    }
    static var accountDeleteNotesMessageNoCount: String { String(localized: "account.deleteNotesMessageNoCount") }
    static var accountDeleteNotesHint: String { String(localized: "account.deleteNotesHint") }
    static var accountDeleteNotesAction: String { String(localized: "account.deleteNotesAction") }
    static var accountDeleteWrongPassword: String { String(localized: "account.deleteWrongPassword") }

    // MARK: - Conflicts

    static var conflictAncestor: String { String(localized: "conflict.ancestor") }
}
