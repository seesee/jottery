import SwiftUI

/// Centralised localised string accessors.
///
/// Usage: `L.settingsTitle` instead of hardcoded `"Settings"`.
/// All keys correspond to entries in `Localizable.xcstrings`.
enum L {

    // MARK: - Settings

    static var settingsTitle: String { String(localized: "settings.title") }
    static var settingsGeneral: String { String(localized: "settings.general") }
    static var settingsTheme: String { String(localized: "settings.theme") }
    static var settingsThemeSystem: String { String(localized: "settings.themeSystem") }
    static var settingsThemeLight: String { String(localized: "settings.themeLight") }
    static var settingsThemeDark: String { String(localized: "settings.themeDark") }
    static var settingsSortOrder: String { String(localized: "settings.sortOrder") }
    static var settingsAutoLock: String { String(localized: "settings.autoLock") }
    static var settingsAutoLock1Min: String { String(localized: "settings.autoLock1Min") }
    static var settingsAutoLock5Min: String { String(localized: "settings.autoLock5Min") }
    static var settingsAutoLock15Min: String { String(localized: "settings.autoLock15Min") }
    static var settingsAutoLock30Min: String { String(localized: "settings.autoLock30Min") }
    static var settingsAutoLock1Hour: String { String(localized: "settings.autoLock1Hour") }
    static var settingsAutoLockNever: String { String(localized: "settings.autoLockNever") }
    static var settingsSecurity: String { String(localized: "settings.security") }
    static var settingsBiometric: String { String(localized: "settings.biometric") }
    static var settingsLanguage: String { String(localized: "settings.language") }
    static var settingsLanguageSystem: String { String(localized: "settings.languageSystem") }
    static var settingsLanguageEnGB: String { String(localized: "settings.languageEnGB") }
    static var settingsAbout: String { String(localized: "settings.about") }
    static var settingsNotes: String { String(localized: "settings.notes") }
    static var settingsVersion: String { String(localized: "settings.version") }
    static var settingsPrivacyPolicy: String { String(localized: "settings.privacyPolicy") }
    static var settingsSupport: String { String(localized: "settings.support") }
    static var settingsLockNow: String { String(localized: "settings.lockNow") }
    static var settingsReset: String { String(localized: "settings.reset") }
    static var settingsWipeAllData: String { String(localized: "settings.wipeAllData") }
    static var settingsDone: String { String(localized: "settings.done") }
    static var settingsWipeConfirmTitle: String { String(localized: "settings.wipeConfirmTitle") }
    static var settingsWipeConfirmAction: String { String(localized: "settings.wipeConfirmAction") }
    static var settingsWipeConfirmMessage: String { String(localized: "settings.wipeConfirmMessage") }
    static var settingsSync: String { String(localized: "settings.sync") }
    static var settingsSyncServer: String { String(localized: "settings.syncServer") }
    static var settingsSyncLastSync: String { String(localized: "settings.syncLastSync") }
    static var settingsSyncNow: String { String(localized: "settings.syncNow") }
    static var settingsSyncDisconnect: String { String(localized: "settings.syncDisconnect") }
    static var settingsSyncSetUp: String { String(localized: "settings.syncSetUp") }
    static var syncDisconnectServerTooOld: String { String(localized: "syncDisconnect.serverTooOld") }
    static var syncDisconnectStillRegistered: String { String(localized: "syncDisconnect.stillRegistered") }
    static var syncSetupErrorInvalidCredentials: String { String(localized: "syncSetupError.invalidCredentials") }
    static var syncSetupErrorNeedsEndpoint: String { String(localized: "syncSetupError.needsEndpoint") }
    static var syncSetupErrorWrongPassword: String { String(localized: "syncSetupError.wrongPassword") }
    static var syncSetupErrorWrongVaultPassword: String { String(localized: "syncSetupError.wrongVaultPassword") }

    // MARK: - Delete Account

    static var deleteAccountTitle: String { String(localized: "deleteAccount.title") }
    static var deleteAccountExplanation: String { String(localized: "deleteAccount.explanation") }
    static var deleteAccountCredentials: String { String(localized: "deleteAccount.credentials") }
    static var deleteAccountMode: String { String(localized: "deleteAccount.mode") }
    static var deleteAccountModeDeactivate: String { String(localized: "deleteAccount.modeDeactivate") }
    static var deleteAccountModeDelete: String { String(localized: "deleteAccount.modeDelete") }
    static var deleteAccountDeactivateHint: String { String(localized: "deleteAccount.deactivateHint") }
    static var deleteAccountDeleteHint: String { String(localized: "deleteAccount.deleteHint") }
    static var deleteAccountAction: String { String(localized: "deleteAccount.action") }
    static var deleteAccountConfirmTitle: String { String(localized: "deleteAccount.confirmTitle") }
    static var deleteAccountWrongCredentials: String { String(localized: "deleteAccount.wrongCredentials") }
    static var deleteAccountNoServer: String { String(localized: "deleteAccount.noServer") }

    // MARK: - Common

    static var commonCancel: String { String(localized: "common.cancel") }

    // MARK: - Note List

    static var noteListTitle: String { String(localized: "noteList.title") }
    static var noteListSearch: String { String(localized: "noteList.search") }
    static var noteListNewNote: String { String(localized: "noteList.newNote") }
    static var noteListSort: String { String(localized: "noteList.sort") }
    static var noteListRecycleBin: String { String(localized: "noteList.recycleBin") }
    static var noteListSettings: String { String(localized: "noteList.settings") }
    static var noteListLock: String { String(localized: "noteList.lock") }
    static var noteListNoNotes: String { String(localized: "noteList.noNotes") }
    static var noteListNoNotesDescription: String { String(localized: "noteList.noNotesDescription") }
    static var noteListDelete: String { String(localized: "noteList.delete") }
    static var noteListPin: String { String(localized: "noteList.pin") }
    static var noteListUnpin: String { String(localized: "noteList.unpin") }

    // MARK: - Note List (Archive & Search Count)

    static var noteListArchive: String { String(localized: "noteList.archive") }
    static var noteListUnarchive: String { String(localized: "noteList.unarchive") }
    static var noteListArchiveTitle: String { String(localized: "noteList.archiveTitle") }
    static var noteListShowArchive: String { String(localized: "noteList.showArchive") }
    static var noteListShowNotes: String { String(localized: "noteList.showNotes") }
    static var noteListArchiveEmpty: String { String(localized: "noteList.archiveEmpty") }
    static var noteListArchiveEmptyDescription: String { String(localized: "noteList.archiveEmptyDescription") }
    static func noteListSearchCount(_ matches: Int, _ total: Int) -> String {
        String(format: String(localized: "noteList.searchCount"), matches, total)
    }

    // MARK: - Editor

    static var editorPin: String { String(localized: "editor.pin") }
    static var editorUnpin: String { String(localized: "editor.unpin") }
    static var editorEnableWordWrap: String { String(localized: "editor.enableWordWrap") }
    static var editorDisableWordWrap: String { String(localized: "editor.disableWordWrap") }
    static var editorResetTextSize: String { String(localized: "editor.resetTextSize") }
    static func editorLanguage(_ name: String) -> String {
        String(format: String(localized: "editor.language"), name)
    }
    static var editorCategory: String { String(localized: "editor.category") }
    static var editorCategoryNone: String { String(localized: "editor.categoryNone") }
    static var editorLock: String { String(localized: "editor.lock") }
    static var editorUnlock: String { String(localized: "editor.unlock") }
    static var editorArchive: String { String(localized: "editor.archive") }
    static var editorUnarchive: String { String(localized: "editor.unarchive") }
    static var editorVersionHistory: String { String(localized: "editor.versionHistory") }
    static var editorAddAttachment: String { String(localized: "editor.addAttachment") }
    static var editorAddPhoto: String { String(localized: "editor.addPhoto") }
    static var editorPasteFromClipboard: String { String(localized: "editor.pasteFromClipboard") }
    static var editorShowPreview: String { String(localized: "editor.showPreview") }
    static var editorHidePreview: String { String(localized: "editor.hidePreview") }
    static var editorDuplicate: String { String(localized: "editor.duplicate") }
    static var editorDelete: String { String(localized: "editor.delete") }
    static var editorNoNoteSelected: String { String(localized: "editor.noNoteSelected") }
    static var editorNoNoteSelectedDescription: String { String(localized: "editor.noNoteSelectedDescription") }

    // MARK: - Version History

    static var versionHistoryTitle: String { String(localized: "versionHistory.title") }
    static var versionHistoryDone: String { String(localized: "versionHistory.done") }
    static var versionHistoryRestore: String { String(localized: "versionHistory.restore") }
    static var versionHistoryNoVersions: String { String(localized: "versionHistory.noVersions") }
    static var versionHistoryNoVersionsDescription: String { String(localized: "versionHistory.noVersionsDescription") }
    static func versionHistoryVersion(_ num: Int) -> String {
        String(format: String(localized: "versionHistory.version"), num)
    }

    // MARK: - Unlock

    static var unlockTitle: String { String(localized: "unlock.title") }
    static var unlockPrompt: String { String(localized: "unlock.prompt") }
    static var unlockPassword: String { String(localized: "unlock.password") }
    static var unlockAction: String { String(localized: "unlock.action") }
    static var unlockIncorrectPassword: String { String(localized: "unlock.incorrectPassword") }
    static var unlockFailedAttempts: String { String(localized: "unlock.failedAttempts") }
    static var unlockDeleteAndStartOver: String { String(localized: "unlock.deleteAndStartOver") }
    static var unlockDeleteConfirmTitle: String { String(localized: "unlock.deleteConfirmTitle") }
    static var unlockDeleteConfirmMessage: String { String(localized: "unlock.deleteConfirmMessage") }
    static var unlockDeleteConfirmAction: String { String(localized: "unlock.deleteConfirmAction") }
    static var unlockDeleteConfirmCancel: String { String(localized: "unlock.deleteConfirmCancel") }
    static var unlockFaceId: String { String(localized: "unlock.faceId") }
    static var unlockEnableFaceIdTitle: String { String(localized: "unlock.enableFaceIdTitle") }
    static var unlockEnableFaceIdAction: String { String(localized: "unlock.enableFaceIdAction") }
    static var unlockEnableFaceIdLater: String { String(localized: "unlock.enableFaceIdLater") }
    static var unlockEnableFaceIdMessage: String { String(localized: "unlock.enableFaceIdMessage") }

    // MARK: - Setup

    static var setupWelcome: String { String(localized: "setup.welcome") }
    static var setupSubtitle: String { String(localized: "setup.subtitle") }
    static var setupModePicker: String { String(localized: "setup.modePicker") }
    static var setupNewVault: String { String(localized: "setup.newVault") }
    static var setupConnectToServer: String { String(localized: "setup.connectToServer") }
    static var setupPassword: String { String(localized: "setup.password") }
    static var setupConfirmPassword: String { String(localized: "setup.confirmPassword") }
    static var setupPasswordsDoNotMatch: String { String(localized: "setup.passwordsDoNotMatch") }
    static var setupPasswordWarning: String { String(localized: "setup.passwordWarning") }
    static var setupCreateVault: String { String(localized: "setup.createVault") }
    static var setupMethod: String { String(localized: "setup.method") }
    static var setupRegister: String { String(localized: "setup.register") }
    static var setupImport: String { String(localized: "setup.import") }
    static var setupServerUrl: String { String(localized: "setup.serverUrl") }
    static var setupEmail: String { String(localized: "setup.email") }
    static var setupServerPassword: String { String(localized: "setup.serverPassword") }
    static var setupDeviceName: String { String(localized: "setup.deviceName") }
    static var setupRegisterDevice: String { String(localized: "setup.registerDevice") }
    static var setupImportCredentials: String { String(localized: "setup.importCredentials") }
    static var setupCredentials: String { String(localized: "setup.credentials") }
    static var setupConnected: String { String(localized: "setup.connected") }
    static var setupEncryptionPassword: String { String(localized: "setup.encryptionPassword") }
    static var setupEncryptionPasswordHint: String { String(localized: "setup.encryptionPasswordHint") }
    static var setupUnlockAndSync: String { String(localized: "setup.unlockAndSync") }
    static var setupProgressCreatingVault: String { String(localized: "setup.progressCreatingVault") }
    static var setupProgressDecrypting: String { String(localized: "setup.progressDecrypting") }
    static var setupProgressRegistering: String { String(localized: "setup.progressRegistering") }
    static var setupProgressSettingUp: String { String(localized: "setup.progressSettingUp") }
    static var setupProgressPushing: String { String(localized: "setup.progressPushing") }
    static var setupProgressPulling: String { String(localized: "setup.progressPulling") }
    static var setupProgressFinishing: String { String(localized: "setup.progressFinishing") }

    // MARK: - Recycle Bin

    static var recycleBinTitle: String { String(localized: "recycleBin.title") }
    static var recycleBinDone: String { String(localized: "recycleBin.done") }
    static var recycleBinEmptyBin: String { String(localized: "recycleBin.emptyBin") }
    static var recycleBinRestore: String { String(localized: "recycleBin.restore") }
    static var recycleBinDeleteForever: String { String(localized: "recycleBin.deleteForever") }
    static var recycleBinEmptyConfirmTitle: String { String(localized: "recycleBin.emptyConfirmTitle") }
    static var recycleBinEmptyConfirmAction: String { String(localized: "recycleBin.emptyConfirmAction") }
    static var recycleBinEmpty: String { String(localized: "recycleBin.empty") }
    static var recycleBinEmptyDescription: String { String(localized: "recycleBin.emptyDescription") }

    // MARK: - Sync Setup

    static var syncSetupTitle: String { String(localized: "syncSetup.title") }
    static var syncSetupMethod: String { String(localized: "syncSetup.method") }
    static var syncSetupRegister: String { String(localized: "syncSetup.register") }
    static var syncSetupImport: String { String(localized: "syncSetup.import") }
    static var syncSetupRegisterDevice: String { String(localized: "syncSetup.registerDevice") }
    static var syncSetupServerUrl: String { String(localized: "syncSetup.serverUrl") }
    static var syncSetupEmail: String { String(localized: "syncSetup.email") }
    static var syncSetupServerPassword: String { String(localized: "syncSetup.serverPassword") }
    static var syncSetupDeviceName: String { String(localized: "syncSetup.deviceName") }
    static var syncSetupNotesPassword: String { String(localized: "syncSetup.notesPassword") }
    static var syncSetupNotesPasswordHint: String { String(localized: "syncSetup.notesPasswordHint") }
    static var syncSetupEnvelopeUploadFailed: String { String(localized: "syncSetup.envelopeUploadFailed") }
    static var syncSetupRegisterAction: String { String(localized: "syncSetup.registerAction") }
    static var syncSetupImportCredentials: String { String(localized: "syncSetup.importCredentials") }
    static var syncSetupImportHint: String { String(localized: "syncSetup.importHint") }
    static var syncSetupCredentials: String { String(localized: "syncSetup.credentials") }
    static var syncSetupImportAction: String { String(localized: "syncSetup.importAction") }
    static var syncSetupSuccess: String { String(localized: "syncSetup.success") }
    static var syncSetupDone: String { String(localized: "syncSetup.done") }
    static var syncSetupCancel: String { String(localized: "syncSetup.cancel") }
    static var syncEndpointEmpty: String { String(localized: "syncEndpoint.empty") }
    static var syncEndpointMalformed: String { String(localized: "syncEndpoint.malformed") }
    static var syncEndpointInsecurePublicHost: String { String(localized: "syncEndpoint.insecurePublicHost") }
    static var syncEndpointInsecureStored: String { String(localized: "syncEndpoint.insecureStored") }

    // MARK: - Tags

    static var tagsAddTag: String { String(localized: "tags.addTag") }

    // MARK: - Attachments

    static func attachmentsHeader(_ count: Int) -> String {
        String(format: String(localized: "attachments.header"), count)
    }
    static var attachmentsDataNotAvailable: String { String(localized: "attachments.dataNotAvailable") }
    static var attachmentsInvalidData: String { String(localized: "attachments.invalidData") }
    static var attachmentsDecryptFailed: String { String(localized: "attachments.decryptFailed") }

    // MARK: - Saved Searches

    static var savedSearchTitle: String { String(localized: "savedSearch.title") }
    static var savedSearchSaveCurrent: String { String(localized: "savedSearch.saveCurrent") }
    static var savedSearchEmpty: String { String(localized: "savedSearch.empty") }
    static var savedSearchEmptyDescription: String { String(localized: "savedSearch.emptyDescription") }
    static var savedSearchSaveTitle: String { String(localized: "savedSearch.saveTitle") }
    static var savedSearchNamePlaceholder: String { String(localized: "savedSearch.namePlaceholder") }
    static var savedSearchSaveAction: String { String(localized: "savedSearch.saveAction") }
    static func savedSearchSaveMessage(_ query: String) -> String {
        String(format: String(localized: "savedSearch.saveMessage"), query)
    }
    static var savedSearchAddTitle: String { String(localized: "savedSearch.addTitle") }
    static var savedSearchQueryPlaceholder: String { String(localized: "savedSearch.queryPlaceholder") }

    // MARK: - Inbox

    static var inboxTitle: String { String(localized: "inbox.title") }
    static var inboxEmpty: String { String(localized: "inbox.empty") }
    static var inboxEmptyDescription: String { String(localized: "inbox.emptyDescription") }
    static var inboxAccept: String { String(localized: "inbox.accept") }
    static var inboxAcceptAll: String { String(localized: "inbox.acceptAll") }
    static var inboxDeleteAll: String { String(localized: "inbox.deleteAll") }

    // MARK: - Conflicts

    static var conflictTitle: String { String(localized: "conflict.title") }
    static var conflictNoConflicts: String { String(localized: "conflict.noConflicts") }
    static var conflictNoConflictsDescription: String { String(localized: "conflict.noConflictsDescription") }
    static var conflictResolve: String { String(localized: "conflict.resolve") }
    static var conflictVersion: String { String(localized: "conflict.version") }
    static var conflictLocal: String { String(localized: "conflict.local") }
    static var conflictServer: String { String(localized: "conflict.server") }
    static var conflictKeepLocal: String { String(localized: "conflict.keepLocal") }
    static var conflictKeepServer: String { String(localized: "conflict.keepServer") }
    static var conflictKeepBoth: String { String(localized: "conflict.keepBoth") }
    static func conflictBanner(_ count: Int) -> String {
        String(format: String(localized: "conflict.banner"), count)
    }

    // MARK: - Bulk Operations

    static var bulkSelect: String { String(localized: "bulk.select") }
    static var bulkDone: String { String(localized: "bulk.done") }
    static func bulkSelected(_ count: Int) -> String {
        String(format: String(localized: "bulk.selected"), count)
    }
    static var bulkSelectAll: String { String(localized: "bulk.selectAll") }
    static var bulkDeselectAll: String { String(localized: "bulk.deselectAll") }
    static var bulkAddTags: String { String(localized: "bulk.addTags") }
    static var bulkRemoveTags: String { String(localized: "bulk.removeTags") }
    static var bulkSetColour: String { String(localized: "bulk.setColour") }
    static var bulkExport: String { String(localized: "bulk.export") }
    static var bulkApply: String { String(localized: "bulk.apply") }
    static var bulkTagsPlaceholder: String { String(localized: "bulk.tagsPlaceholder") }
    static var bulkTagsHint: String { String(localized: "bulk.tagsHint") }
    static var bulkDeleteConfirmTitle: String { String(localized: "bulk.deleteConfirmTitle") }
    static var bulkDeleteConfirmAction: String { String(localized: "bulk.deleteConfirmAction") }
    static func bulkDeleteConfirmMessage(_ count: Int) -> String {
        String(format: String(localized: "bulk.deleteConfirmMessage"), count)
    }

    // MARK: - Settings (Data)

    static var settingsData: String { String(localized: "settings.data") }
    static var settingsExportAll: String { String(localized: "settings.exportAll") }
    static var settingsImport: String { String(localized: "settings.import") }

    // MARK: - Import

    static var importTitle: String { String(localized: "import.title") }
    static var importSelectFile: String { String(localized: "import.selectFile") }
    static var importFileLoaded: String { String(localized: "import.fileLoaded") }
    static var importNoteCount: String { String(localized: "import.noteCount") }
    static var importExportDate: String { String(localized: "import.exportDate") }
    static var importStrategy: String { String(localized: "import.strategy") }
    static var importStrategySkip: String { String(localized: "import.strategySkip") }
    static var importStrategyReplace: String { String(localized: "import.strategyReplace") }
    static var importStrategyMerge: String { String(localized: "import.strategyMerge") }
    static var importStrategySkipDescription: String { String(localized: "import.strategySkipDescription") }
    static var importStrategyReplaceDescription: String { String(localized: "import.strategyReplaceDescription") }
    static var importStrategyMergeDescription: String { String(localized: "import.strategyMergeDescription") }
    static var importAction: String { String(localized: "import.action") }
    static func importProgress(_ current: Int, _ total: Int) -> String {
        String(format: String(localized: "import.progress"), current, total)
    }
    static var importComplete: String { String(localized: "import.complete") }
    static var importImported: String { String(localized: "import.imported") }
    static var importSkipped: String { String(localized: "import.skipped") }
    static var importErrors: String { String(localized: "import.errors") }

    // MARK: - Change Password

    static var changePasswordTitle: String { String(localized: "changePassword.title") }
    static var changePasswordCurrent: String { String(localized: "changePassword.current") }
    static var changePasswordNew: String { String(localized: "changePassword.new") }
    static var changePasswordConfirm: String { String(localized: "changePassword.confirm") }
    static var changePasswordAction: String { String(localized: "changePassword.action") }
    static var changePasswordWarning: String { String(localized: "changePassword.warning") }
    static var changePasswordSuccessTitle: String { String(localized: "changePassword.successTitle") }
    static var changePasswordSuccessMessage: String { String(localized: "changePassword.successMessage") }

    // MARK: - Note Info

    static var noteInfoTitle: String { String(localized: "noteInfo.title") }
    static var noteInfoNoteId: String { String(localized: "noteInfo.noteId") }
    static var noteInfoCreated: String { String(localized: "noteInfo.created") }
    static var noteInfoModified: String { String(localized: "noteInfo.modified") }
    static var noteInfoSyncedAt: String { String(localized: "noteInfo.syncedAt") }
    static var noteInfoVersion: String { String(localized: "noteInfo.version") }
    static var noteInfoContentHash: String { String(localized: "noteInfo.contentHash") }
    static var noteInfoWordCount: String { String(localized: "noteInfo.wordCount") }
    static var noteInfoCharacterCount: String { String(localized: "noteInfo.characterCount") }
    static var noteInfoAttachmentCount: String { String(localized: "noteInfo.attachmentCount") }
    static var noteInfoSyncStatus: String { String(localized: "noteInfo.syncStatus") }
    static var noteInfoSynced: String { String(localized: "noteInfo.synced") }
    static var noteInfoPendingSync: String { String(localized: "noteInfo.pendingSync") }
    static var noteInfoNeverSynced: String { String(localized: "noteInfo.neverSynced") }

    // MARK: - Backup

    static var backupTitle: String { String(localized: "backup.title") }
    static var backupCreate: String { String(localized: "backup.create") }
    static var backupCreateDescription: String { String(localized: "backup.createDescription") }
    static var backupRestore: String { String(localized: "backup.restore") }
    static var backupRestoreDescription: String { String(localized: "backup.restoreDescription") }
    static var backupCreating: String { String(localized: "backup.creating") }
    static var backupRestoring: String { String(localized: "backup.restoring") }
    static var backupSuccessTitle: String { String(localized: "backup.successTitle") }
    static var backupSuccessMessage: String { String(localized: "backup.successMessage") }
    static var backupRestoreSuccessTitle: String { String(localized: "backup.restoreSuccessTitle") }
    static var backupRestoreSuccessMessage: String { String(localized: "backup.restoreSuccessMessage") }
    static var backupRestoreWarning: String { String(localized: "backup.restoreWarning") }
    static var backupPassword: String { String(localized: "backup.password") }
    static var backupPasswordHint: String { String(localized: "backup.passwordHint") }

    // MARK: - Force Sync

    static var forceSyncTitle: String { String(localized: "forceSync.title") }
    static var forceSyncMessage: String { String(localized: "forceSync.message") }
    static var forceSyncAction: String { String(localized: "forceSync.action") }

    // MARK: - Sort Order

    static var sortRecentlyModified: String { String(localized: "sort.recentlyModified") }
    static var sortOldestModified: String { String(localized: "sort.oldestModified") }
    static var sortAlphabetical: String { String(localized: "sort.alphabetical") }
    static var sortDateCreated: String { String(localized: "sort.dateCreated") }
}
