import CryptoKit
import Foundation
import GRDB
import SwiftUI

/// Root application state — drives the entire UI.
@MainActor @Observable
final class AppState {

    // MARK: - Lock State

    var isFirstLaunch: Bool = true
    var isLocked: Bool = true

    // MARK: - Notes

    var notes: [DecryptedNote] = []
    var selectedNoteId: String?
    var searchQuery: String = ""
    var sortOrder: SortOrder = .recent

    // MARK: - Sync

    var isSyncing: Bool = false
    var lastSyncAt: Date?
    var syncError: String?
    var syncEnabled: Bool = false
    var syncStatusMessage: String?

    // MARK: - Settings

    var settings: UserSettings = .defaults

    // MARK: - Internal

    private(set) var db: DatabaseManager?
    private(set) var keyManager = KeyManager()
    var syncClient: SyncClient?
    var syncService: SyncService?

    // Repositories (lazily initialised after DB is ready)
    private(set) var noteRepo: NoteRepository?
    private(set) var encryptionRepo: EncryptionRepository?
    private(set) var settingsRepo: SettingsRepository?
    private(set) var syncRepo: SyncRepository?

    // MARK: - Computed

    var filteredNotes: [DecryptedNote] {
        let filtered = SearchService.filter(notes: notes, query: searchQuery)
        return SearchService.sort(notes: filtered, order: sortOrder)
    }

    var noteCount: Int { notes.count }
    var filteredCount: Int { filteredNotes.count }

    var selectedNote: DecryptedNote? {
        guard let id = selectedNoteId else { return nil }
        return notes.first { $0.id == id }
    }

    // MARK: - Initialisation

    func initialise() {
        guard db == nil else { return }  // Already initialised
        do {
            let database = try DatabaseManager()
            self.db = database
            self.noteRepo = NoteRepository(db: database)
            self.encryptionRepo = EncryptionRepository(db: database)
            self.settingsRepo = SettingsRepository(db: database)
            self.syncRepo = SyncRepository(db: database)

            // Check if vault exists
            let hasVault = try encryptionRepo?.isVaultSetUp() ?? false
            isFirstLaunch = !hasVault

            // Load settings
            if let loaded = try settingsRepo?.get() {
                settings = loaded
                sortOrder = loaded.sort
                keyManager.autoLockTimeout = TimeInterval(loaded.autoLockTimeout * 60)
            }
        } catch {
            // Database init failed — stay on first launch screen
            isFirstLaunch = true
        }
    }

    // MARK: - Vault Setup

    /// Create a new vault with the given password.
    /// When importing from another device, pass the existing `salt` and `iterations`
    /// so the same password derives the same encryption key.
    func createVault(password: String, existingSalt: Data? = nil, existingIterations: UInt32? = nil) throws {
        guard let encryptionRepo else { throw AppStateError.notInitialised }

        let salt = existingSalt ?? CryptoService.generateSalt()
        let iterations = existingIterations ?? CryptoService.defaultIterations

        // Derive key first so we can create the verification token
        let key = keyManager.unlock(password: password, salt: salt, iterations: iterations)

        // Encrypt a known plaintext to verify the password later
        let verificationEncrypted = try CryptoService.encryptText(
            EncryptionMetadata.verificationPlaintext, key: key
        )
        let verificationJSON = try CryptoService.serializeEncryptedJSON(verificationEncrypted)

        let metadata = EncryptionMetadata.new(
            salt: salt, iterations: iterations, verification: verificationJSON
        )
        try encryptionRepo.store(metadata)

        isFirstLaunch = false
        isLocked = false

        // Load notes (should be empty for new vault)
        try loadNotes()
    }

    // MARK: - Unlock

    /// Attempt to unlock with the given password.
    /// Returns true if successful.
    @discardableResult
    func unlock(password: String) throws -> Bool {
        guard let encryptionRepo else { throw AppStateError.notInitialised }

        guard let metadata = try encryptionRepo.get() else {
            throw AppStateError.noVault
        }

        guard let saltData = metadata.saltData else {
            throw AppStateError.invalidSalt
        }

        let key = keyManager.unlock(
            password: password,
            salt: saltData,
            iterations: UInt32(metadata.iterations)
        )

        // Verify password
        let verified: Bool
        if let verificationJSON = metadata.verification {
            // Use stored verification token
            verified = verifyWithToken(verificationJSON, key: key)
        } else {
            // No token (old vault) — try decrypting a note instead
            verified = verifyByDecryptingNote(key: key)
        }

        guard verified else {
            keyManager.lock()
            isLocked = true
            throw AppStateError.wrongPassword
        }

        try loadNotes()
        isLocked = false
        setupSync()

        // Upgrade old vaults: store a verification token if missing
        if metadata.verification == nil {
            try? upgradeVaultVerification(key: key)
        }

        return true
    }

    /// Verify the key by decrypting the stored verification token.
    private func verifyWithToken(_ json: String, key: SymmetricKey) -> Bool {
        do {
            let encrypted = try CryptoService.parseEncryptedJSON(json)
            let plaintext = try CryptoService.decryptText(encrypted, key: key)
            return plaintext == EncryptionMetadata.verificationPlaintext
        } catch {
            return false
        }
    }

    /// Verify the key by decrypting one note from the database.
    /// Returns true if decryption succeeds, or if there are no notes to test against.
    private func verifyByDecryptingNote(key: SymmetricKey) -> Bool {
        guard let db else { return false }
        do {
            let record = try db.dbPool.read { database in
                try NoteRecord.filter(Column("deleted") == false).limit(1).fetchOne(database)
            }
            guard let record else {
                // No notes — can't verify, but empty vault is safe to enter
                return true
            }
            let encContent = try CryptoService.parseEncryptedJSON(record.content)
            _ = try CryptoService.decryptText(encContent, key: key)
            return true
        } catch {
            return false
        }
    }

    /// Store a verification token on an old vault that doesn't have one.
    private func upgradeVaultVerification(key: SymmetricKey) throws {
        guard let encryptionRepo, var metadata = try encryptionRepo.get() else { return }
        let encrypted = try CryptoService.encryptText(
            EncryptionMetadata.verificationPlaintext, key: key
        )
        metadata.verification = try CryptoService.serializeEncryptedJSON(encrypted)
        try encryptionRepo.store(metadata)
    }

    /// Lock the application.
    func lock() {
        keyManager.lock()
        isLocked = true
        notes = []
        selectedNoteId = nil
    }

    // MARK: - Notes CRUD

    func loadNotes() throws {
        guard let noteRepo, let key = keyManager.masterKey else { return }
        notes = try noteRepo.listActive(key: key)
    }

    func createNote(content: String = "", tags: [String] = []) throws -> DecryptedNote? {
        guard let noteRepo, let key = keyManager.masterKey else { return nil }
        let note = try noteRepo.create(content: content, tags: tags, key: key)
        notes.insert(note, at: 0)
        selectedNoteId = note.id
        return note
    }

    func saveNote(_ note: DecryptedNote) throws {
        guard let noteRepo, let key = keyManager.masterKey else { return }
        try noteRepo.update(note, key: key)

        // Update in-memory
        if let index = notes.firstIndex(where: { $0.id == note.id }) {
            var updated = note
            updated.modifiedAt = Date()
            notes[index] = updated
        }
    }

    func deleteNote(id: String) throws {
        guard let noteRepo else { return }
        try noteRepo.softDelete(id: id)
        notes.removeAll { $0.id == id }
        if selectedNoteId == id {
            selectedNoteId = nil
        }
    }

    func restoreNote(id: String) throws {
        guard let noteRepo else { return }
        try noteRepo.restore(id: id)
        try loadNotes()
    }

    func togglePin(id: String) throws {
        guard let noteRepo else { return }
        try noteRepo.togglePin(id: id)
        if let index = notes.firstIndex(where: { $0.id == id }) {
            notes[index].pinned.toggle()
        }
    }

    // MARK: - Sync

    /// Set up the sync client. Pass `apiKey` directly after registration
    /// to avoid Keychain retrieval timing issues; otherwise reads from Keychain.
    func setupSync(apiKey providedKey: String? = nil) {
        guard settings.syncEnabled,
              let endpoint = settings.syncEndpoint,
              let noteRepo, let syncRepo, let key = keyManager.masterKey else { return }

        let apiKey = providedKey ?? KeychainService.retrieveAPIKey()
        guard let apiKey, !apiKey.isEmpty else {
            syncError = "No API key found. Please re-register the device."
            return
        }
        let client = SyncClient(endpoint: endpoint, apiKey: apiKey)
        self.syncClient = client
        self.syncService = SyncService(
            syncClient: client,
            noteRepo: noteRepo,
            syncRepo: syncRepo,
            key: key
        )
        syncEnabled = true
        syncError = nil
    }

    func triggerSync() async {
        guard let syncService else { return }
        isSyncing = true
        syncError = nil
        syncStatusMessage = "Pushing changes…"

        do {
            syncStatusMessage = "Pushing…"
            try await syncService.push()
            syncStatusMessage = "Pulling…"
            try await syncService.pull()
            syncStatusMessage = "Finishing…"
            try await syncService.finalise()
            let previousCount = notes.count
            try? loadNotes()
            isSyncing = false
            lastSyncAt = Date()

            let diff = notes.count - previousCount
            if diff > 0 {
                syncStatusMessage = "Synced — \(diff) new note\(diff == 1 ? "" : "s")"
            } else {
                syncStatusMessage = "Synced — up to date"
            }

            // Clear the status message after a few seconds
            let clearMessage = syncStatusMessage
            try? await Task.sleep(for: .seconds(4))
            if syncStatusMessage == clearMessage {
                syncStatusMessage = nil
            }
        } catch {
            syncError = error.localizedDescription
            syncStatusMessage = nil
            isSyncing = false
        }
    }

    // MARK: - Wipe & Re-onboard

    /// Delete everything — database, Keychain, in-memory state — so the user
    /// can start fresh as if the app was just installed.
    func wipeAllData() {
        // Clear Keychain
        KeychainService.deleteAPIKey()
        KeychainService.deleteClientId()
        KeychainService.deleteBiometricKey()

        // Delete database file
        let fm = FileManager.default
        if let appSupport = try? fm.url(
            for: .applicationSupportDirectory,
            in: .userDomainMask,
            appropriateFor: nil,
            create: false
        ) {
            let dbDir = appSupport.appendingPathComponent("Jottery", isDirectory: true)
            try? fm.removeItem(at: dbDir)
        }

        // Reset all in-memory state
        db = nil
        noteRepo = nil
        encryptionRepo = nil
        settingsRepo = nil
        syncRepo = nil
        syncClient = nil
        syncService = nil
        notes = []
        selectedNoteId = nil
        searchQuery = ""
        syncEnabled = false
        isSyncing = false
        lastSyncAt = nil
        syncError = nil
        syncStatusMessage = nil
        settings = .defaults
        keyManager.lock()
        isLocked = true
        isFirstLaunch = true

        // Re-initialise so the DB is recreated fresh
        initialise()
    }

    // MARK: - Settings

    func updateSettings(_ newSettings: UserSettings) throws {
        guard let settingsRepo else { return }
        settings = newSettings
        try settingsRepo.save(newSettings)
        sortOrder = newSettings.sort
        keyManager.autoLockTimeout = TimeInterval(newSettings.autoLockTimeout * 60)
    }
}

enum AppStateError: LocalizedError {
    case notInitialised
    case noVault
    case invalidSalt
    case wrongPassword

    var errorDescription: String? {
        switch self {
        case .notInitialised: return "Application not initialised"
        case .noVault: return "No vault found"
        case .invalidSalt: return "Invalid encryption salt"
        case .wrongPassword: return "Incorrect password"
        }
    }
}
