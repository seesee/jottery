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
    static var settingsLanguageHint: String { String(localized: "settings.languageHint") }
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
    static var syncSetupErrorWrongVaultPassword: String { String(localized: "syncSetupError.wrongVaultPassword") }
    static var syncSetupErrorEncryptionPasswordMismatch: String { String(localized: "syncSetupError.encryptionPasswordMismatch") }
    static var syncSetupErrorAccountKeyDiffers: String { String(localized: "syncSetupError.accountKeyDiffers") }
    static var syncSetupErrorAccountKeyUnreadable: String { String(localized: "syncSetupError.accountKeyUnreadable") }
    static var syncSetupErrorNotesPasswordMatchesAccount: String { String(localized: "syncSetupError.notesPasswordMatchesAccount") }
    static func notesPasswordTooShort(_ minimum: Int) -> String {
        String(localized: "notesPassword.tooShort \(minimum)")
    }

    // MARK: - Sync Errors
    static var syncErrorAccountDisabled: String { String(localized: "syncError.accountDisabled") }
    static var syncErrorAccountPending: String { String(localized: "syncError.accountPending") }
    static var syncErrorPasswordChangeRequired: String { String(localized: "syncError.passwordChangeRequired") }
    static func syncErrorRateLimited(_ seconds: Int) -> String {
        String(localized: "syncError.rateLimited \(seconds)")
    }
    static func syncErrorServerBusy(_ seconds: Int) -> String {
        String(localized: "syncError.serverBusy \(seconds)")
    }
    static var syncErrorTimedOut: String { String(localized: "syncError.timedOut") }
    static var syncErrorStorageFull: String { String(localized: "syncError.storageFull") }
    static var syncErrorKeyChanged: String { String(localized: "syncError.keyChanged") }
    static var syncErrorClockAhead: String { String(localized: "syncError.clockAhead") }
    static func syncNotesTooLarge(_ count: Int) -> String {
        String(format: String(localized: "syncError.notesTooLarge"), count)
    }
    static var syncServerRemovedTitle: String { String(localized: "syncServerRemoved.title") }
    static func syncServerRemovedMessage(_ count: Int) -> String {
        String(format: String(localized: "syncServerRemoved.message"), count)
    }
    static var syncServerRemovedRestore: String { String(localized: "syncServerRemoved.restore") }
    static var syncServerRemovedKeep: String { String(localized: "syncServerRemoved.keep") }

    // MARK: - Sync Merge
    static var syncMergeTitle: String { String(localized: "syncMerge.title") }
    static func syncMergeMessage(server: String, noteCount: Int) -> String {
        String(format: String(localized: "syncMerge.message"), server, noteCount)
    }
    static var syncMergeConfirm: String { String(localized: "syncMerge.confirm") }
    static var syncMergeFailed: String { String(localized: "syncMerge.failed") }
    static var syncMergeFailedOnUnlock: String { String(localized: "syncMerge.failedOnUnlock") }

    // MARK: - Unlock (Face ID key check)
    static var unlockBiometricOutOfDate: String { String(localized: "unlock.biometricOutOfDate") }

    // MARK: - Account Key (Face ID unlock)
    static var accountKeyPromptTitle: String { String(localized: "accountKey.promptTitle") }
    static var accountKeyPromptMessage: String { String(localized: "accountKey.promptMessage") }
    static var accountKeyPromptWrongPassword: String { String(localized: "accountKey.promptWrongPassword") }
    static var accountKeyPromptPublish: String { String(localized: "accountKey.promptPublish") }
    static var accountKeyPromptNotNow: String { String(localized: "accountKey.promptNotNow") }
    static var accountKeyPublishFailed: String { String(localized: "accountKey.publishFailed") }

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

    // MARK: - Backup

    static var backupErrorKeyNotInBackup: String { String(localized: "backupError.keyNotInBackup") }

    // MARK: - Common

    static var commonCancel: String { String(localized: "common.cancel") }
    static var commonOK: String { String(localized: "common.ok") }
    static var commonShowPassword: String { String(localized: "common.showPassword") }
    static var commonHidePassword: String { String(localized: "common.hidePassword") }

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
    /// The warning, with how many notes wiping would lose for good.
    static func unlockDeleteConfirmMessage(unsyncedNotes: Int) -> String {
        guard unsyncedNotes > 0 else { return unlockDeleteConfirmMessage }
        return unlockDeleteConfirmMessage + "\n\n"
            + String(format: String(localized: "unlock.deleteUnsyncedNotes"), unsyncedNotes)
    }
    static var unlockDeleteConfirmAction: String { String(localized: "unlock.deleteConfirmAction") }
    static var unlockDeleteConfirmCancel: String { String(localized: "unlock.deleteConfirmCancel") }
    static var launchOpening: String { String(localized: "launch.opening") }
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
    static var setupServerUrl: String { String(localized: "setup.serverUrl") }
    static var setupEmail: String { String(localized: "setup.email") }
    static var setupServerPassword: String { String(localized: "setup.serverPassword") }
    static var setupDeviceName: String { String(localized: "setup.deviceName") }
    static var setupRegisterDevice: String { String(localized: "setup.registerDevice") }
    static var setupConnected: String { String(localized: "setup.connected") }
    static var setupEncryptionPassword: String { String(localized: "setup.encryptionPassword") }
    static var setupEncryptionPasswordHint: String { String(localized: "setup.encryptionPasswordHint") }
    static var setupUnlockAndSync: String { String(localized: "setup.unlockAndSync") }
    static var setupProgressCreatingVault: String { String(localized: "setup.progressCreatingVault") }
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
    static func recycleBinEmptyConfirmMessage(_ count: Int) -> String {
        String(localized: "recycleBin.emptyConfirmMessage \(count)")
    }
    static var recycleBinEmpty: String { String(localized: "recycleBin.empty") }
    static var recycleBinEmptyDescription: String { String(localized: "recycleBin.emptyDescription") }

    // MARK: - Sync Setup

    static var syncSetupTitle: String { String(localized: "syncSetup.title") }
    static var syncSetupRegisterDevice: String { String(localized: "syncSetup.registerDevice") }
    static var syncSetupServerUrl: String { String(localized: "syncSetup.serverUrl") }
    static var syncSetupEmail: String { String(localized: "syncSetup.email") }
    static var syncSetupServerPassword: String { String(localized: "syncSetup.serverPassword") }
    static var syncSetupDeviceName: String { String(localized: "syncSetup.deviceName") }
    static var syncSetupNotesPassword: String { String(localized: "syncSetup.notesPassword") }
    static var syncSetupNotesPasswordHint: String { String(localized: "syncSetup.notesPasswordHint") }
    static var syncSetupEnvelopeUploadFailed: String { String(localized: "syncSetup.envelopeUploadFailed") }
    static var syncSetupPasswordsMatchWarning: String { String(localized: "syncSetup.passwordsMatchWarning") }
    static var accountKeyPasswordChangedElsewhereTitle: String { String(localized: "accountKey.passwordChangedElsewhereTitle") }
    static var accountKeyPasswordChangedElsewhereMessage: String { String(localized: "accountKey.passwordChangedElsewhereMessage") }
    static var accountKeyPasswordMatchesAccountTitle: String { String(localized: "accountKey.passwordMatchesAccountTitle") }
    static var accountKeyPasswordMatchesAccountMessage: String { String(localized: "accountKey.passwordMatchesAccountMessage") }
    static var changePasswordServerKeyPending: String { String(localized: "changePassword.serverKeyPending") }
    static var syncSetupRegisterAction: String { String(localized: "syncSetup.registerAction") }
    static var syncSetupAccountChoice: String { String(localized: "syncSetup.accountChoice") }
    static var syncSetupExistingAccount: String { String(localized: "syncSetup.existingAccount") }
    static var syncSetupNewAccount: String { String(localized: "syncSetup.newAccount") }
    static var syncSetupCreateAccountAction: String { String(localized: "syncSetup.createAccountAction") }
    static var syncSetupAwaitingApproval: String { String(localized: "syncSetup.awaitingApproval") }
    static var syncSetupCheckApproval: String { String(localized: "syncSetup.checkApproval") }
    static var syncErrorAccountExists: String { String(localized: "syncError.accountExists") }
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
    static var attachmentsUnreadableName: String { String(localized: "attachments.unreadableName") }

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
    static var conflictUnreadable: String { String(localized: "conflict.unreadable") }
    static var editorShownAsPlainText: String { String(localized: "editor.shownAsPlainText") }
    static var conflictCouldntResolve: String { String(localized: "conflict.couldntResolve") }
    static func conflictBanner(_ count: Int) -> String {
        String(localized: "conflict.banner \(count)")
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
        String(localized: "bulk.deleteConfirmMessage \(count)")
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
    static func backupProgress(_ completed: Int, _ total: Int) -> String {
        String(format: String(localized: "backup.progress"), completed, total)
    }
    static func exportProgress(_ completed: Int, _ total: Int) -> String {
        String(format: String(localized: "export.progress"), completed, total)
    }
    static func restoreProgress(_ completed: Int, _ total: Int) -> String {
        String(format: String(localized: "backup.restoreProgress"), completed, total)
    }
    static func backupRestoreKeptNewer(_ count: Int) -> String {
        String(localized: "backup.restoreKeptNewer \(count)")
    }
    static func backupRestoreSkipped(_ count: Int) -> String {
        String(localized: "backup.restoreSkipped \(count)")
    }
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

    // MARK: - Errors (Toasts)

    static var errorCouldntAddAttachment: String { String(localized: "error.couldntAddAttachment") }
    static var errorCouldntSaveNote: String { String(localized: "error.couldntSaveNote") }
    static var vaultReKeying: String { String(localized: "vault.reKeying") }
    static var vaultBusy: String { String(localized: "vault.busy") }
    static var editorCopy: String { String(localized: "editor.copy") }
    static var editorCopyLink: String { String(localized: "editor.copyLink") }
    static var editorShare: String { String(localized: "editor.share") }
    static var editorPrint: String { String(localized: "editor.print") }
    static var previewNoteNotFound: String { String(localized: "preview.noteNotFound") }
    static var errorCouldntSaveSettings: String { String(localized: "error.couldntSaveSettings") }
    static var errorCouldntRestoreVersion: String { String(localized: "error.couldntRestoreVersion") }
    static var unlockDeleteAuthReason: String { String(localized: "unlock.deleteAuthReason") }
    static func attachmentsDeleteTitle(_ filename: String) -> String {
        String(format: String(localized: "attachments.deleteTitle"), filename)
    }
    static var attachmentsDeleteMessage: String { String(localized: "attachments.deleteMessage") }
    static var attachmentsDeleteAction: String { String(localized: "attachments.deleteAction") }
    static var attachmentsShare: String { String(localized: "attachments.share") }
    static var inboxDeleteAllTitle: String { String(localized: "inbox.deleteAllTitle") }
    static func inboxDeleteAllMessage(_ count: Int) -> String {
        String(format: String(localized: "inbox.deleteAllMessage"), count)
    }
    static var errorCouldntExport: String { String(localized: "error.couldntExport") }
    static func errorCouldntAddTags(_ count: Int) -> String {
        String(localized: "error.couldntAddTags \(count)")
    }
    static var errorCouldntArchiveNote: String { String(localized: "error.couldntArchiveNote") }
    static func errorCouldntArchiveNotes(_ count: Int) -> String {
        String(localized: "error.couldntArchiveNotes \(count)")
    }
    static var errorCouldntDeleteNote: String { String(localized: "error.couldntDeleteNote") }
    static var errorCouldntDeleteNoteForever: String { String(localized: "error.couldntDeleteNoteForever") }
    static func errorCouldntDeleteNotes(_ count: Int) -> String {
        String(localized: "error.couldntDeleteNotes \(count)")
    }
    static var errorCouldntDuplicateNote: String { String(localized: "error.couldntDuplicateNote") }
    static var errorCouldntEmptyRecycleBin: String { String(localized: "error.couldntEmptyRecycleBin") }
    static var errorCouldntLockNote: String { String(localized: "error.couldntLockNote") }
    static var errorCouldntPinNote: String { String(localized: "error.couldntPinNote") }
    static var errorCouldntRemoveAttachment: String { String(localized: "error.couldntRemoveAttachment") }
    static func errorCouldntRemoveTags(_ count: Int) -> String {
        String(localized: "error.couldntRemoveTags \(count)")
    }
    static var errorCouldntRestoreNote: String { String(localized: "error.couldntRestoreNote") }
    static func errorCouldntSetColor(_ count: Int) -> String {
        String(localized: "error.couldntSetColor \(count)")
    }
    static var errorCouldntUnarchiveNote: String { String(localized: "error.couldntUnarchiveNote") }
    static var errorCouldntUnlockNote: String { String(localized: "error.couldntUnlockNote") }
    static var errorCouldntUnpinNote: String { String(localized: "error.couldntUnpinNote") }
}
