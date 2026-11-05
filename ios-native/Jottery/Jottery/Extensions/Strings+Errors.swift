import Foundation

/// What errors and sync status say. Most of these only reach the user when something
/// has gone wrong, which isn't the moment to switch language.
extension L {

    // MARK: - Notes

    /// What a colour is called. `key` is what's stored and what `category:` finds,
    /// so it doesn't change with the language.
    static func colourName(_ key: String) -> String {
        Bundle.main.localizedString(forKey: "colour.\(key)", value: key.capitalized, table: nil)
    }

    static var changePasswordOldBackups: String { String(localized: "changePassword.oldBackups") }
    static var noteUntitled: String { String(localized: "note.untitled") }
    static var versionHistoryCouldntDecrypt: String { String(localized: "versionHistory.couldntDecrypt") }

    // MARK: - Sync status

    static var syncStatusSyncing: String { String(localized: "syncStatus.syncing") }
    static var syncStatusUpToDate: String { String(localized: "syncStatus.upToDate") }
    static var syncStatusFullSyncRunning: String { String(localized: "syncStatus.fullSyncRunning") }
    static var syncStatusFullSyncDone: String { String(localized: "syncStatus.fullSyncDone") }
    static func syncStatusNewNotes(_ count: Int) -> String { String(localized: "syncStatus.newNotes \(count)") }
    static func syncStatusNotes(_ count: Int) -> String { String(localized: "syncStatus.notes \(count)") }
    static var syncErrorNoApiKey: String { String(localized: "syncError.noApiKey") }

    // MARK: - App

    static var appErrorNotInitialised: String { String(localized: "appError.notInitialised") }
    static var appErrorNoVault: String { String(localized: "appError.noVault") }
    static var appErrorInvalidSalt: String { String(localized: "appError.invalidSalt") }
    static var appErrorWrongPassword: String { String(localized: "appError.wrongPassword") }
    static var appErrorSyncBusy: String { String(localized: "appError.syncBusy") }

    // MARK: - Crypto and Keychain

    static var cryptoErrorInvalidData: String { String(localized: "cryptoError.invalidData") }
    static var cryptoErrorEncodingFailed: String { String(localized: "cryptoError.encodingFailed") }
    static var cryptoErrorKeyDerivationFailed: String { String(localized: "cryptoError.keyDerivationFailed") }
    static var cryptoErrorDecryptionFailed: String { String(localized: "cryptoError.decryptionFailed") }
    static var keyErrorNoKeyLoaded: String { String(localized: "keyError.noKeyLoaded") }
    static var keychainErrorAccessControl: String { String(localized: "keychainError.accessControl") }
    static func keychainErrorStore(_ status: Int) -> String {
        String(format: String(localized: "keychainError.store"), status)
    }
    static func keychainErrorRetrieve(_ status: Int) -> String {
        String(format: String(localized: "keychainError.retrieve"), status)
    }
    static var unlockBiometricReason: String { String(localized: "unlock.biometricReason") }

    // MARK: - Server

    static var syncErrorInvalidResponse: String { String(localized: "syncError.invalidResponse") }
    static var syncErrorUnauthorised: String { String(localized: "syncError.unauthorised") }
    static var envelopeErrorNoServerKey: String { String(localized: "envelopeError.noServerKey") }
    static var envelopeErrorCantOpenServerKey: String { String(localized: "envelopeError.cantOpenServerKey") }
    static var envelopeErrorInvalidKeyLength: String { String(localized: "envelopeError.invalidKeyLength") }
    static var envelopeErrorNotInitialised: String { String(localized: "envelopeError.notInitialised") }

    // MARK: - Backups and imports

    static var backupErrorNoVault: String { String(localized: "backupError.noVault") }
    static var backupErrorInvalidFormat: String { String(localized: "backupError.invalidFormat") }
    static func backupErrorUnsupportedVersion(_ version: String) -> String {
        String(format: String(localized: "backupError.unsupportedVersion"), version)
    }
    static var backupErrorInvalidSalt: String { String(localized: "backupError.invalidSalt") }
    static var backupErrorWrongPassword: String { String(localized: "backupError.wrongPassword") }
    static var backupErrorInvalidData: String { String(localized: "backupError.invalidData") }
    static var backupErrorCompressionFailed: String { String(localized: "backupError.compressionFailed") }
    static var backupErrorDecompressionFailed: String { String(localized: "backupError.decompressionFailed") }
    static func importErrorUnsupportedVersion(_ version: String) -> String {
        String(format: String(localized: "importError.unsupportedVersion"), version)
    }
    static var importErrorInvalidFormat: String { String(localized: "importError.invalidFormat") }
    static var vaultReKeyErrorUnreadable: String { String(localized: "vaultReKeyError.unreadable") }
}
