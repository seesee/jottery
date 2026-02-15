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
    var archivedNotes: [DecryptedNote] = []
    var showArchive: Bool = false
    var selectedNoteId: String?
    var searchQuery: String = ""
    var sortOrder: SortOrder = .recent

    // MARK: - Sync

    var isSyncing: Bool = false
    var lastSyncAt: Date?
    var syncError: String?
    var syncEnabled: Bool = false
    var syncStatusMessage: String?

    // MARK: - Lifecycle

    var backgroundedAt: Date?

    // MARK: - Settings

    var settings: UserSettings = .defaults

    // MARK: - Internal

    private(set) var db: DatabaseManager?
    private(set) var keyManager = KeyManager()
    var syncClient: SyncClient?
    var syncService: SyncService?

    /// Current unsaved editor state — NoteEditorView keeps this updated so
    /// `lock()` can flush pending changes before wiping the encryption key.
    @ObservationIgnored var pendingEditorNote: DecryptedNote?

    // Repositories (lazily initialised after DB is ready)
    private(set) var noteRepo: NoteRepository?
    private(set) var encryptionRepo: EncryptionRepository?
    private(set) var settingsRepo: SettingsRepository?
    private(set) var syncRepo: SyncRepository?
    private(set) var versionRepo: VersionRepository?
    private(set) var attachmentRepo: AttachmentRepository?
    private(set) var savedSearchRepo: SavedSearchRepository?

    // MARK: - Saved Searches

    var savedSearches: [SavedSearchRepository.SavedSearch] = []

    // MARK: - Computed

    /// Notes displayed in the list — either active or archived depending on mode.
    var displayedNotes: [DecryptedNote] {
        showArchive ? archivedNotes : notes
    }

    var filteredNotes: [DecryptedNote] {
        let filtered = SearchService.filter(notes: displayedNotes, query: searchQuery)
        return SearchService.sort(notes: filtered, order: sortOrder)
    }

    var noteCount: Int { displayedNotes.count }
    var filteredCount: Int { filteredNotes.count }

    var selectedNote: DecryptedNote? {
        guard let id = selectedNoteId else { return nil }
        return notes.first { $0.id == id }
    }

    // MARK: - Initialisation

    func initialise() {
        guard db == nil else { return }  // Already initialised

        // Wire auto-lock so the timer triggers a full app lock (UI + key wipe)
        keyManager.onAutoLock = { [weak self] in
            self?.lock()
        }

        do {
            let database = try DatabaseManager()
            self.db = database
            let verRepo = VersionRepository(db: database)
            self.versionRepo = verRepo
            self.noteRepo = NoteRepository(db: database, versionRepo: verRepo)
            self.encryptionRepo = EncryptionRepository(db: database)
            self.settingsRepo = SettingsRepository(db: database)
            self.syncRepo = SyncRepository(db: database)
            self.attachmentRepo = AttachmentRepository(db: database)
            self.savedSearchRepo = SavedSearchRepository(db: database)

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
        print("[Sync] unlock: calling setupSync()")
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
        print("[Sync] lock: clearing syncService, syncEnabled=false")
        // Flush any pending editor save while the key is still available
        if let pending = pendingEditorNote, let noteRepo, let key = keyManager.masterKey {
            try? noteRepo.update(pending, key: key)
            pendingEditorNote = nil
        }

        // Stop SSE and release sync objects (releases the SymmetricKey reference)
        if let service = syncService {
            Task { await service.stopSSE() }
        }
        syncClient = nil
        syncService = nil
        syncEnabled = false

        keyManager.lock()
        isLocked = true
        notes = []
        selectedNoteId = nil
    }

    // MARK: - Notes CRUD

    func loadNotes() throws {
        guard let noteRepo, let key = keyManager.masterKey else { return }
        notes = try noteRepo.listActive(key: key)
        archivedNotes = try noteRepo.listArchived(key: key)
        savedSearches = (try? savedSearchRepo?.listAll(key: key)) ?? []
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

    func archiveNote(id: String) throws {
        guard let noteRepo else { return }
        try noteRepo.archive(id: id)
        notes.removeAll { $0.id == id }
        if selectedNoteId == id { selectedNoteId = nil }
        try loadArchivedNotes()
    }

    func unarchiveNote(id: String) throws {
        guard let noteRepo else { return }
        try noteRepo.unarchive(id: id)
        archivedNotes.removeAll { $0.id == id }
        if selectedNoteId == id { selectedNoteId = nil }
        try loadNotes()
    }

    func toggleLock(id: String) throws {
        guard let noteRepo else { return }
        try noteRepo.toggleLock(id: id)
        if let index = notes.firstIndex(where: { $0.id == id }) {
            notes[index].locked.toggle()
            notes[index].lockedAt = notes[index].locked ? Date() : nil
        }
        if let index = archivedNotes.firstIndex(where: { $0.id == id }) {
            archivedNotes[index].locked.toggle()
            archivedNotes[index].lockedAt = archivedNotes[index].locked ? Date() : nil
        }
    }

    func loadArchivedNotes() throws {
        guard let noteRepo, let key = keyManager.masterKey else { return }
        archivedNotes = try noteRepo.listArchived(key: key)
    }

    // MARK: - Saved Searches

    func saveSearch(name: String, query: String) throws {
        guard let savedSearchRepo, let key = keyManager.masterKey else { return }
        let search = try savedSearchRepo.create(name: name, query: query, key: key)
        savedSearches.append(search)
    }

    func deleteSavedSearch(id: String) throws {
        guard let savedSearchRepo else { return }
        try savedSearchRepo.delete(id: id)
        savedSearches.removeAll { $0.id == id }
    }

    func applySavedSearch(id: String) {
        guard let search = savedSearches.first(where: { $0.id == id }) else { return }
        searchQuery = search.query
    }

    /// Restore a note to a previous version. Creates a snapshot of current state first.
    func restoreVersion(noteId: String, version: NoteVersionRecord) throws {
        guard let noteRepo, let versionRepo, let key = keyManager.masterKey else { return }

        // Snapshot current state before restoring
        if let currentRecord = try noteRepo.getRaw(id: noteId) {
            try versionRepo.createVersion(from: currentRecord, reason: "pre-restore")
        }

        // Decrypt the version's content and tags
        let encContent = try CryptoService.parseEncryptedJSON(version.content)
        let content = try CryptoService.decryptText(encContent, key: key)

        let encTags = try CryptoService.parseEncryptedJSON(version.tags)
        let tagsText = try CryptoService.decryptText(encTags, key: key)
        let tags: [String]
        if tagsText.isEmpty {
            tags = []
        } else if tagsText.hasPrefix("[") {
            tags = (try? JSONDecoder().decode([String].self, from: Data(tagsText.utf8))) ?? []
        } else {
            tags = tagsText.split(separator: ",").map { String($0).trimmingCharacters(in: .whitespaces) }
        }

        // Get current decrypted note and update with restored content
        guard var note = try noteRepo.get(id: noteId, key: key) else { return }
        note.content = content
        note.tags = tags
        note.syntaxLanguage = version.syntaxLanguage ?? note.syntaxLanguage
        note.wordWrap = version.wordWrap ?? note.wordWrap
        note.showPreview = version.showPreview ?? note.showPreview
        note.color = version.color
        try noteRepo.update(note, key: key)

        // Update in-memory
        if let index = notes.firstIndex(where: { $0.id == noteId }) {
            notes[index] = try noteRepo.get(id: noteId, key: key) ?? notes[index]
        }
    }

    // MARK: - Sync

    /// Set up the sync client. Pass `apiKey` directly after registration
    /// to avoid Keychain retrieval timing issues; otherwise reads from Keychain.
    func setupSync(apiKey providedKey: String? = nil) {
        guard settings.syncEnabled else {
            print("[Sync] setupSync: settings.syncEnabled is false — skipping")
            return
        }
        guard let endpoint = settings.syncEndpoint else {
            print("[Sync] setupSync: no syncEndpoint — skipping")
            return
        }
        guard let noteRepo, let syncRepo, let versionRepo, let attachmentRepo else {
            print("[Sync] setupSync: repos not initialised — skipping")
            return
        }
        guard let key = keyManager.masterKey else {
            print("[Sync] setupSync: no masterKey — skipping")
            return
        }

        let apiKey = providedKey ?? KeychainService.retrieveAPIKey()
        guard let apiKey, !apiKey.isEmpty else {
            print("[Sync] setupSync: no API key in Keychain")
            syncError = "No API key found. Please re-register the device."
            return
        }
        print("[Sync] setupSync: OK — endpoint=\(endpoint)")
        let client = SyncClient(endpoint: endpoint, apiKey: apiKey)
        self.syncClient = client
        let service = SyncService(
            syncClient: client,
            noteRepo: noteRepo,
            syncRepo: syncRepo,
            versionRepo: versionRepo,
            attachmentRepo: attachmentRepo,
            key: key
        )
        self.syncService = service
        syncEnabled = true
        syncError = nil

        // Start SSE and wire the post-sync handler to reload notes
        Task { [weak self] in
            await service.setPostSyncHandler { [weak self] in
                await MainActor.run {
                    guard let self else { return }
                    try? self.loadNotes()
                    self.lastSyncAt = Date()
                }
            }
            await service.startSSE()
        }
    }

    func triggerSync() async {
        guard let syncService else {
            print("[Sync] triggerSync: syncService is nil — aborting")
            return
        }
        print("[Sync] triggerSync: starting sync cycle")
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
            print("[Sync] triggerSync: ERROR — \(error)")
            syncError = error.localizedDescription
            syncStatusMessage = nil
            isSyncing = false
        }
    }

    // MARK: - Lifecycle

    func handleScenePhaseChange(_ phase: ScenePhase) {
        switch phase {
        case .background, .inactive:
            // Only record the first transition away — .inactive fires again
            // on the way *back* from background, which would reset the timestamp.
            if backgroundedAt == nil {
                backgroundedAt = Date()
            }
            Task { await syncService?.stopSSE() }
        case .active:
            guard !isLocked else { return }
            // Check auto-lock (timeout 0 means "never")
            if let bg = backgroundedAt, keyManager.autoLockTimeout > 0 {
                let elapsed = Date().timeIntervalSince(bg)
                if elapsed >= keyManager.autoLockTimeout {
                    lock()
                    backgroundedAt = nil
                    return
                }
            }
            backgroundedAt = nil
            keyManager.recordActivity()
            // Restart SSE and trigger a sync on return
            if syncService != nil {
                Task {
                    await syncService?.startSSE()
                    await triggerSync()
                }
            }
        @unknown default:
            break
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
        versionRepo = nil
        attachmentRepo = nil
        savedSearchRepo = nil
        syncClient = nil
        syncService = nil
        notes = []
        archivedNotes = []
        savedSearches = []
        showArchive = false
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
