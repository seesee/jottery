import CryptoKit
import Foundation
import GRDB
import Network
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

    // MARK: - Quick Actions

    var pendingQuickAction: String?
    var searchFocused: Bool = false

    // MARK: - Lifecycle

    var backgroundedAt: Date?

    // MARK: - Editor Font

    /// Multiplier applied to the system body font size.
    /// Persisted in UserDefaults so it survives app restarts.
    var editorFontScale: Double = {
        let saved = UserDefaults.standard.double(forKey: "editorFontScale")
        return saved > 0 ? saved : 1.0
    }() {
        didSet { UserDefaults.standard.set(editorFontScale, forKey: "editorFontScale") }
    }

    /// Effective font size in points (system body size * scale).
    var editorFontSize: CGFloat {
        let base = UIFont.preferredFont(forTextStyle: .body).pointSize
        return max(9, base * editorFontScale)
    }

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
    @ObservationIgnored private var networkMonitor: NWPathMonitor?
    @ObservationIgnored private var networkWasSatisfied = true

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

    // MARK: - Conflicts

    var pendingConflicts: [ConflictInfo] = []

    // MARK: - Inbox

    var inboxItems: [InboxItem] = []
    var inboxCount: Int { inboxItems.count }

    // MARK: - Search (background-threaded, debounced)

    /// Filtered + sorted notes, updated asynchronously to avoid blocking the main thread.
    var filteredNotes: [DecryptedNote] = []
    @ObservationIgnored private var searchTask: Task<Void, Never>?

    /// Notes displayed in the list — either active or archived depending on mode.
    var displayedNotes: [DecryptedNote] {
        showArchive ? archivedNotes : notes
    }

    var noteCount: Int { displayedNotes.count }
    var filteredCount: Int { filteredNotes.count }

    /// Call whenever `searchQuery`, `notes`, `archivedNotes`, `showArchive`,
    /// or `sortOrder` changes to recompute `filteredNotes` off the main thread.
    /// Debounces by 150 ms when a search query is active; immediate otherwise.
    func scheduleSearch() {
        searchTask?.cancel()
        let query = searchQuery
        let source = displayedNotes
        let order = sortOrder

        // No debounce needed when query is empty — just sort (cheap).
        guard !query.trimmingCharacters(in: .whitespaces).isEmpty else {
            filteredNotes = SearchService.sort(notes: source, order: order)
            return
        }

        searchTask = Task {
            // Debounce while typing
            try? await Task.sleep(for: .milliseconds(150))
            guard !Task.isCancelled else { return }

            // Run expensive filter + sort off the main thread
            let results = await Task.detached(priority: .userInitiated) {
                let filtered = SearchService.filter(notes: source, query: query)
                return SearchService.sort(notes: filtered, order: order)
            }.value

            guard !Task.isCancelled else { return }
            self.filteredNotes = results
        }
    }

    /// Pre-warm formatters and the keyboard subsystem so the first
    /// search-bar tap doesn't stall. Runs after the UI has transitioned.
    func scheduleSearchWarmUp() {
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(200))
            Date.warmUpForSearch()
        }
    }

    /// All unique tags across all notes, sorted alphabetically.
    var allTags: [String] {
        let tagSets = notes.flatMap(\.tags) + archivedNotes.flatMap(\.tags)
        return Array(Set(tagSets)).sorted()
    }

    var selectedNote: DecryptedNote? {
        guard let id = selectedNoteId else { return nil }
        return notes.first { $0.id == id }
    }

    // MARK: - Initialisation

    func initialise() {
        guard db == nil else { return }  // Already initialised

        do {
            try initialise(database: DatabaseManager())
        } catch {
            // Database init failed — stay on first launch screen
            isFirstLaunch = true
        }
    }

    /// Wire repositories to a specific database. Internal so tests can
    /// inject a temporary database instead of the app-support default.
    func initialise(database: DatabaseManager) throws {
        // Wire auto-lock so the timer triggers a full app lock (UI + key wipe)
        keyManager.onAutoLock = { [weak self] in
            self?.lock()
        }

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
    }

    // MARK: - Vault Setup

    /// Create a new vault with the given password.
    /// When importing from another device, pass the existing `salt` and `iterations`
    /// so the same password derives the same encryption key.
    func createVault(password: String, existingSalt: Data? = nil, existingIterations: UInt32? = nil) throws {
        guard let encryptionRepo else { throw AppStateError.notInitialised }

        if existingSalt != nil {
            // Legacy import path — use provided salt/iterations for compatibility
            let salt = existingSalt!
            let iterations = existingIterations ?? CryptoService.defaultIterations

            let key = keyManager.unlock(password: password, salt: salt, iterations: iterations)

            let verificationEncrypted = try CryptoService.encryptText(
                EncryptionMetadata.verificationPlaintext, key: key
            )
            let verificationJSON = try CryptoService.serializeEncryptedJSON(verificationEncrypted)

            let metadata = EncryptionMetadata.new(
                salt: salt, iterations: iterations, verification: verificationJSON
            )
            try encryptionRepo.store(metadata)
        } else {
            // New vault — use envelope encryption from the start
            let masterKeyData = CryptoService.generateMasterKey()
            let masterKey = SymmetricKey(data: masterKeyData)

            // Generate device salt and derive device key
            let deviceSalt = CryptoService.generateSalt()
            let deviceKey = CryptoService.deriveKey(
                password: password,
                salt: deviceSalt,
                iterations: CryptoService.defaultIterations
            )

            // Wrap master key locally
            let localWrapped = try CryptoService.wrapMasterKey(masterKeyData, with: deviceKey)
            let localBlob = try CryptoService.serializeEncryptedJSON(localWrapped)

            // Create verification token encrypted with the master key
            let verificationEncrypted = try CryptoService.encryptText(
                EncryptionMetadata.verificationPlaintext, key: masterKey
            )
            let verificationJSON = try CryptoService.serializeEncryptedJSON(verificationEncrypted)

            // Store initial metadata (needed before saveEnvelope which does UPDATE)
            let initialMetadata = EncryptionMetadata.new(
                salt: deviceSalt, iterations: CryptoService.defaultIterations,
                verification: verificationJSON
            )
            try encryptionRepo.store(initialMetadata)

            // Convert to envelope format
            try encryptionRepo.saveEnvelope(
                envelopeVersion: 1,
                deviceSalt: deviceSalt.base64EncodedString(),
                localWrappedMaster: localBlob,
                wrappingKdfVersion: CryptoService.wrappingKdfVersion
            )

            // Update verification (saveEnvelope clears salt/iterations but not verification)
            if var metadata = try encryptionRepo.get() {
                metadata.verification = verificationJSON
                try encryptionRepo.store(metadata)
            }

            keyManager.unlockWithKeyData(masterKeyData)
        }

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

        let key: SymmetricKey

        if metadata.envelopeVersion != nil {
            // Envelope unlock path — derive device key, unwrap master key
            guard let deviceSaltB64 = metadata.deviceSalt,
                  let deviceSalt = Data(base64Encoded: deviceSaltB64),
                  let wrappedJSON = metadata.localWrappedMaster else {
                throw AppStateError.invalidSalt
            }

            let deviceKey = CryptoService.deriveKey(
                password: password,
                salt: deviceSalt,
                iterations: CryptoService.defaultIterations
            )

            let wrapped = try CryptoService.parseEncryptedJSON(wrappedJSON)
            let masterKeyData: Data
            do {
                masterKeyData = try CryptoService.unwrapMasterKey(wrapped, with: deviceKey)
            } catch {
                keyManager.lock()
                isLocked = true
                throw AppStateError.wrongPassword
            }

            key = SymmetricKey(data: masterKeyData)
            keyManager.unlockWithKeyData(masterKeyData)

            // Verify with token if available
            if let verificationJSON = metadata.verification {
                guard verifyWithToken(verificationJSON, key: key) else {
                    keyManager.lock()
                    isLocked = true
                    throw AppStateError.wrongPassword
                }
            }
        } else {
            // Legacy unlock path — derive key directly from password + salt
            guard let saltData = metadata.saltData else {
                throw AppStateError.invalidSalt
            }

            key = keyManager.unlock(
                password: password,
                salt: saltData,
                iterations: UInt32(metadata.iterations)
            )

            // Verify password
            let verified: Bool
            if let verificationJSON = metadata.verification {
                verified = verifyWithToken(verificationJSON, key: key)
            } else {
                verified = verifyByDecryptingNote(key: key)
            }

            guard verified else {
                keyManager.lock()
                isLocked = true
                throw AppStateError.wrongPassword
            }
        }

        try loadNotes()
        isLocked = false

        // Auto-purge deleted notes older than 30 days
        let purged = (try? noteRepo?.purgeOldDeletedNotes()) ?? 0
        if purged > 0 {
            print("[Purge] Removed \(purged) deleted note(s) older than 30 days")
        }

        print("[Sync] unlock: calling setupSync()")
        setupSync()
        scheduleSearchWarmUp()

        // Upgrade old vaults: store a verification token if missing
        if metadata.verification == nil {
            try? upgradeVaultVerification(key: key)
        }

        // Auto-migrate legacy vaults to envelope in background (non-fatal)
        if metadata.envelopeVersion == nil, settings.syncEnabled {
            Task {
                await EnvelopeService.tryEnvelopeSetup(
                    appState: self,
                    password: password,
                    masterKey: key
                )
            }
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

        // Stop SSE, network monitor, and release sync objects
        stopNetworkMonitor()
        if let service = syncService {
            Task { await service.stopSSE() }
        }
        syncClient = nil
        syncService = nil
        syncEnabled = false

        keyManager.lock()
        isLocked = true
        notes = []
        archivedNotes = []
        pendingConflicts = []
        inboxItems = []
        selectedNoteId = nil
    }

    // MARK: - Notes CRUD

    func loadNotes() throws {
        guard let noteRepo, let key = keyManager.masterKey else { return }
        notes = try noteRepo.listActive(key: key)
        archivedNotes = try noteRepo.listArchived(key: key)
        savedSearches = (try? savedSearchRepo?.listAll(key: key)) ?? []
        scheduleSearch()
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

    func duplicateNote(id: String) throws {
        guard let noteRepo, let key = keyManager.masterKey else { return }
        guard let original = try noteRepo.get(id: id, key: key) else { return }
        let duplicate = try noteRepo.create(content: original.content, tags: original.tags, key: key)
        notes.insert(duplicate, at: 0)
        selectedNoteId = duplicate.id
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

    // MARK: - Conflict Resolution

    func resolveConflict(noteId: String, strategy: ConflictResolutionStrategy) async throws {
        guard let syncService else { return }
        try await syncService.resolveConflict(noteId: noteId, strategy: strategy)
        pendingConflicts = await syncService.pendingConflicts
        try? loadNotes()
        // Sync immediately so keepLocal pushes the resolved note and keepBoth
        // pushes the duplicate — no manual refresh needed.
        await triggerSync()
    }

    // MARK: - Attachments

    func addAttachment(to noteId: String, url: URL, filename: String, mimeType: String) throws {
        try addAttachment(to: noteId, data: try Data(contentsOf: url), filename: filename, mimeType: mimeType)
    }

    func addAttachment(to noteId: String, data fileData: Data, filename: String, mimeType: String) throws {
        guard let noteRepo, let attachmentRepo, let key = keyManager.masterKey else { return }
        guard try noteRepo.get(id: noteId, key: key) != nil else { return }

        // Encrypt the file data
        let encrypted = try CryptoService.encrypt(fileData, key: key)
        let encryptedJSON = try CryptoService.serializeEncryptedJSON(encrypted)
        let blobData = Data(encryptedJSON.utf8)

        // Single ID for both metadata and blob — the server's attachments_data.id
        // is a FK to attachments_meta.id, so they must match.
        let attachmentId = CryptoService.generateUUID()

        // Store encrypted blob with the same ID used for the attachment ref
        try attachmentRepo.storeBlob(id: attachmentId, filename: filename, mimeType: mimeType, size: fileData.count, data: blobData)

        // Encrypt the filename for storage
        let encFilename = try CryptoService.encryptText(filename, key: key)
        let encFilenameJSON = try CryptoService.serializeEncryptedJSON(encFilename)

        // Create attachment ref (data = attachmentId so blob lookup matches)
        let ref = AttachmentRef(
            id: attachmentId,
            filename: encFilenameJSON,
            mimeType: mimeType,
            size: fileData.count,
            data: attachmentId
        )

        // Save with encrypted filename in the raw record
        try noteRepo.addAttachment(noteId: noteId, ref: ref)

        // Re-fetch so the in-memory copy carries the bumped modifiedAt —
        // DecryptedNote's Equatable ignores `attachments`, so a stale
        // modifiedAt would stop SwiftUI re-rendering the editor.
        refreshNoteFromStore(id: noteId)
    }

    func removeAttachment(from noteId: String, attachmentId: String) throws {
        guard let noteRepo, let attachmentRepo, let key = keyManager.masterKey else { return }
        guard let note = try noteRepo.get(id: noteId, key: key) else { return }

        // Find the attachment to get its blob ID
        guard let ref = note.attachments.first(where: { $0.id == attachmentId }) else { return }

        // Remove from note
        try noteRepo.removeAttachment(noteId: noteId, attachmentId: attachmentId)

        // Delete blob
        try? attachmentRepo.deleteBlob(id: ref.data)

        refreshNoteFromStore(id: noteId)
    }

    /// Replace the in-memory copy of a note with the current database state.
    private func refreshNoteFromStore(id: String) {
        guard let noteRepo, let key = keyManager.masterKey else { return }
        guard let fresh = try? noteRepo.get(id: id, key: key) else { return }
        if let index = notes.firstIndex(where: { $0.id == id }) {
            notes[index] = fresh
        }
        if let index = archivedNotes.firstIndex(where: { $0.id == id }) {
            archivedNotes[index] = fresh
        }
    }

    // MARK: - Inbox

    func loadInboxItems() async throws {
        guard let syncClient else { return }
        inboxItems = try await syncClient.listInboxItems()
    }

    func acceptInboxItem(_ item: InboxItem) async throws {
        guard let noteRepo, let key = keyManager.masterKey, let syncClient else { return }
        // Create a new encrypted note from the inbox item (showPreview forced false for safety)
        let note = try noteRepo.create(content: item.content, tags: item.tags, key: key)
        notes.insert(note, at: 0)
        // Delete from server
        try? await syncClient.deleteInboxItem(id: item.id)
        inboxItems.removeAll { $0.id == item.id }
    }

    func deleteInboxItem(_ item: InboxItem) async throws {
        guard let syncClient else { return }
        try await syncClient.deleteInboxItem(id: item.id)
        inboxItems.removeAll { $0.id == item.id }
    }

    func deleteAllInboxItems() async throws {
        guard let syncClient else { return }
        try await syncClient.deleteAllInboxItems()
        inboxItems.removeAll()
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
        guard let noteRepo, let syncRepo, let versionRepo, let attachmentRepo, let savedSearchRepo else {
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
            savedSearchRepo: savedSearchRepo,
            key: key
        )
        self.syncService = service
        syncEnabled = true
        syncError = nil

        // Wire the post-sync handler, start SSE, and run the initial sync
        Task { [weak self] in
            await service.setPostSyncHandler { [weak self] in
                await MainActor.run {
                    guard let self else { return }
                    try? self.loadNotes()
                    self.lastSyncAt = Date()
                }
            }
            await service.startSSE()
            await self?.triggerSync()
        }

        // Restart SSE immediately when the network recovers
        startNetworkMonitor()
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

            // Pull any conflicts detected during push
            pendingConflicts = await syncService.pendingConflicts

            let diff = notes.count - previousCount
            if diff > 0 {
                syncStatusMessage = "Synced — \(diff) new note\(diff == 1 ? "" : "s")"
            } else {
                syncStatusMessage = "Synced — up to date"
            }

            // Refresh inbox items in the background (don't block sync completion)
            Task { [weak self] in try? await self?.loadInboxItems() }

            // Clear the status message after a few seconds (non-blocking)
            let clearMessage = syncStatusMessage
            Task { @MainActor [weak self] in
                try? await Task.sleep(for: .seconds(4))
                if self?.syncStatusMessage == clearMessage {
                    self?.syncStatusMessage = nil
                }
            }
        } catch {
            print("[Sync] triggerSync: ERROR — \(error)")
            syncError = error.localizedDescription
            syncStatusMessage = nil
            isSyncing = false
        }
    }

    /// Force a full sync — re-downloads all notes from the server.
    func forceFullSync() async {
        guard let syncService else { return }
        isSyncing = true
        syncError = nil
        syncStatusMessage = "Full sync in progress…"

        do {
            try await syncService.forceFullSync()
            try? loadNotes()
            isSyncing = false
            lastSyncAt = Date()
            pendingConflicts = await syncService.pendingConflicts
            syncStatusMessage = "Full sync complete"

            Task { [weak self] in try? await self?.loadInboxItems() }

            let msg = syncStatusMessage
            Task { @MainActor [weak self] in
                try? await Task.sleep(for: .seconds(4))
                if self?.syncStatusMessage == msg { self?.syncStatusMessage = nil }
            }
        } catch {
            syncError = error.localizedDescription
            syncStatusMessage = nil
            isSyncing = false
        }
    }

    // MARK: - Network Monitoring

    /// Observe network path changes and restart SSE only when transitioning
    /// from no-connectivity to connectivity (not on every path fluctuation).
    private func startNetworkMonitor() {
        stopNetworkMonitor()
        networkWasSatisfied = true // assume connected at start
        let monitor = NWPathMonitor()
        networkMonitor = monitor
        monitor.pathUpdateHandler = { [weak self] path in
            let satisfied = path.status == .satisfied
            Task { @MainActor [weak self] in
                guard let self else { return }
                let wasDown = !self.networkWasSatisfied
                self.networkWasSatisfied = satisfied
                // Only restart SSE on actual recovery: unsatisfied → satisfied
                guard satisfied && wasDown else { return }
                guard !self.isLocked,
                      self.backgroundedAt == nil,
                      let service = self.syncService else { return }
                await service.startSSE()
            }
        }
        monitor.start(queue: DispatchQueue(label: "jottery.network"))
    }

    private func stopNetworkMonitor() {
        networkMonitor?.cancel()
        networkMonitor = nil
    }

    // MARK: - Lifecycle

    func handleScenePhaseChange(_ phase: ScenePhase) {
        switch phase {
        case .background:
            // Genuinely backgrounded — stop SSE and record timestamp for auto-lock.
            if backgroundedAt == nil {
                backgroundedAt = Date()
            }
            Task { await syncService?.stopSSE() }
        case .inactive:
            // Transient inactivity (Face ID prompt, notification centre, share sheet).
            // Don't stop SSE or set backgroundedAt — avoid triggering a redundant
            // SSE restart and full sync when the app returns to .active.
            break
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
            let wasBackgrounded = backgroundedAt != nil
            backgroundedAt = nil
            keyManager.recordActivity()
            // Only restart SSE and sync when returning from genuine background
            if wasBackgrounded, syncService != nil {
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
        filteredNotes = []
        savedSearches = []
        pendingConflicts = []
        inboxItems = []
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

    // MARK: - Password Change

    /// Change the vault password. With envelope encryption, only re-wraps the master key (fast).
    /// For legacy vaults, re-encrypts all notes, attachments, versions, and the verification token.
    func changePassword(currentPassword: String, newPassword: String) throws {
        guard let encryptionRepo, let noteRepo, let versionRepo else {
            throw AppStateError.notInitialised
        }
        guard let metadata = try encryptionRepo.get() else {
            throw AppStateError.noVault
        }

        if metadata.envelopeVersion != nil {
            // FAST PATH: envelope active — re-wrap only, no note re-encryption
            guard let deviceSaltB64 = metadata.deviceSalt,
                  let deviceSalt = Data(base64Encoded: deviceSaltB64),
                  let wrappedJSON = metadata.localWrappedMaster else {
                throw AppStateError.invalidSalt
            }

            // Verify current password by unwrapping the master key
            let currentDeviceKey = CryptoService.deriveKey(
                password: currentPassword,
                salt: deviceSalt,
                iterations: CryptoService.defaultIterations
            )
            let wrapped = try CryptoService.parseEncryptedJSON(wrappedJSON)
            let masterKeyData: Data
            do {
                masterKeyData = try CryptoService.unwrapMasterKey(wrapped, with: currentDeviceKey)
            } catch {
                throw AppStateError.wrongPassword
            }

            // Verify with token as extra check
            if let verificationJSON = metadata.verification {
                let masterKey = SymmetricKey(data: masterKeyData)
                guard verifyWithToken(verificationJSON, key: masterKey) else {
                    throw AppStateError.wrongPassword
                }
            }

            // Synchronous: local re-wrap (throws on failure so the UI sees the error)
            try EnvelopeService.changePasswordLocal(
                encryptionRepo: encryptionRepo,
                newPassword: newPassword,
                masterKeyData: masterKeyData
            )

            // Synchronous: update biometric key if enabled (master key unchanged)
            if keyManager.isBiometricEnabled {
                try? keyManager.enableBiometricUnlock()
            }

            // Background: server re-wrap (non-fatal, will retry on next unlock/sync)
            if let syncRepo, let syncClient {
                let dataForServer = masterKeyData
                Task {
                    await EnvelopeService.changePasswordServer(
                        syncRepo: syncRepo,
                        syncClient: syncClient,
                        newPassword: newPassword,
                        masterKeyData: dataForServer
                    )
                }
            }
        } else {
            // LEGACY PATH: full re-encryption
            guard let saltData = metadata.saltData else {
                throw AppStateError.invalidSalt
            }

            // Verify current password
            let currentKey = CryptoService.deriveKey(
                password: currentPassword,
                salt: saltData,
                iterations: UInt32(metadata.iterations)
            )
            if let verificationJSON = metadata.verification {
                guard verifyWithToken(verificationJSON, key: currentKey) else {
                    throw AppStateError.wrongPassword
                }
            } else {
                guard verifyByDecryptingNote(key: currentKey) else {
                    throw AppStateError.wrongPassword
                }
            }

            // Generate new salt and derive new key
            let newSalt = CryptoService.generateSalt()
            let iterations = UInt32(metadata.iterations)
            let newKey = CryptoService.deriveKey(
                password: newPassword,
                salt: newSalt,
                iterations: iterations
            )

            // Re-encrypt all active, deleted, and archived notes
            let allNotes = try noteRepo.listActive(key: currentKey) +
                            noteRepo.listDeleted(key: currentKey) +
                            noteRepo.listArchived(key: currentKey)

            for note in allNotes {
                let encContent = try CryptoService.encryptText(note.content, key: newKey)
                let encTags = try CryptoService.encryptStringArray(note.tags, key: newKey)
                let contentJSON = try CryptoService.serializeEncryptedJSON(encContent)
                let tagsJSON = try CryptoService.serializeEncryptedJSON(encTags)

                // Re-encrypt attachment filenames
                var reEncryptedAttachments = note.attachments
                for i in reEncryptedAttachments.indices {
                    let encFilename = try CryptoService.encryptText(reEncryptedAttachments[i].filename, key: newKey)
                    reEncryptedAttachments[i].filename = try CryptoService.serializeEncryptedJSON(encFilename)
                }
                let attachmentsJSON = try JSONEncoder().encode(reEncryptedAttachments)
                let attachmentsString = String(data: attachmentsJSON, encoding: .utf8) ?? "[]"

                guard var record = try noteRepo.getRaw(id: note.id) else { continue }
                record.content = contentJSON
                record.tags = tagsJSON
                record.attachments = attachmentsString
                record.needsSync = true
                try noteRepo.updateRaw(record)
            }

            // Re-encrypt attachment blobs
            if let attachmentRepo {
                let blobIds = try attachmentRepo.listBlobIds()
                for blobId in blobIds {
                    guard let blobData = try attachmentRepo.getBlob(id: blobId) else { continue }
                    let blobStr = String(data: blobData, encoding: .utf8) ?? ""
                    guard !blobStr.isEmpty else { continue }
                    do {
                        let encrypted = try CryptoService.parseEncryptedJSON(blobStr)
                        let plainData = try CryptoService.decrypt(encrypted, key: currentKey)
                        let reEncrypted = try CryptoService.encrypt(plainData, key: newKey)
                        let reEncryptedJSON = try CryptoService.serializeEncryptedJSON(reEncrypted)
                        try attachmentRepo.updateBlobData(id: blobId, data: Data(reEncryptedJSON.utf8))
                    } catch {
                        print("[ChangePassword] Skipping blob \(blobId): \(error)")
                    }
                }
            }

            // Re-encrypt version snapshots
            for noteId in Set(allNotes.map(\.id)) {
                let versions = try versionRepo.getVersions(noteId: noteId)
                for ver in versions {
                    do {
                        let encContent = try CryptoService.parseEncryptedJSON(ver.content)
                        let plainContent = try CryptoService.decryptText(encContent, key: currentKey)
                        let newEncContent = try CryptoService.encryptText(plainContent, key: newKey)

                        let encTags = try CryptoService.parseEncryptedJSON(ver.tags)
                        let plainTagsStr = try CryptoService.decryptText(encTags, key: currentKey)
                        let newEncTags = try CryptoService.encryptText(plainTagsStr, key: newKey)

                        var updated = ver
                        updated.content = try CryptoService.serializeEncryptedJSON(newEncContent)
                        updated.tags = try CryptoService.serializeEncryptedJSON(newEncTags)
                        try versionRepo.insertOrReplace(updated)
                    } catch {
                        print("[ChangePassword] Skipping version \(ver.versionKey): \(error)")
                    }
                }
            }

            // Re-encrypt saved searches
            if let savedSearchRepo {
                let searches = try savedSearchRepo.listAll(key: currentKey)
                for search in searches {
                    let encName = try CryptoService.encryptText(search.name, key: newKey)
                    let encQuery = try CryptoService.encryptText(search.query, key: newKey)
                    try savedSearchRepo.updateEncrypted(
                        id: search.id,
                        name: try CryptoService.serializeEncryptedJSON(encName),
                        query: try CryptoService.serializeEncryptedJSON(encQuery)
                    )
                }
            }

            // Update encryption metadata with new salt and verification token
            let verificationEncrypted = try CryptoService.encryptText(
                EncryptionMetadata.verificationPlaintext, key: newKey
            )
            let verificationJSON = try CryptoService.serializeEncryptedJSON(verificationEncrypted)

            var newMetadata = metadata
            newMetadata.salt = newSalt.base64EncodedString()
            newMetadata.verification = verificationJSON
            try encryptionRepo.store(newMetadata)

            // Update in-memory master key
            keyManager.masterKey = newKey

            // Update biometric key if enabled
            if keyManager.isBiometricEnabled {
                try? keyManager.enableBiometricUnlock()
            }

            // Reload notes with new key
            try loadNotes()

            // Try envelope migration after legacy password change
            if settings.syncEnabled {
                Task {
                    await EnvelopeService.tryEnvelopeSetup(
                        appState: self,
                        password: newPassword,
                        masterKey: newKey
                    )
                }
            }
        }
    }

    // MARK: - Re-encryption

    /// Re-encrypt all notes, attachments, versions, and saved searches from one
    /// key to another. Used when onboarding from the server discovers a different
    /// master key than the local one.
    func reEncryptAllData(from oldKey: SymmetricKey, to newKey: SymmetricKey) throws {
        guard let noteRepo, let versionRepo else {
            throw AppStateError.notInitialised
        }

        // Re-encrypt all notes (active, deleted, archived)
        let allNotes = try noteRepo.listActive(key: oldKey) +
                        noteRepo.listDeleted(key: oldKey) +
                        noteRepo.listArchived(key: oldKey)

        for note in allNotes {
            do {
                let encContent = try CryptoService.encryptText(note.content, key: newKey)
                let encTags = try CryptoService.encryptStringArray(note.tags, key: newKey)
                let contentJSON = try CryptoService.serializeEncryptedJSON(encContent)
                let tagsJSON = try CryptoService.serializeEncryptedJSON(encTags)

                // Re-encrypt attachment filenames
                var reEncryptedAttachments = note.attachments
                for i in reEncryptedAttachments.indices {
                    let encFilename = try CryptoService.encryptText(reEncryptedAttachments[i].filename, key: newKey)
                    reEncryptedAttachments[i].filename = try CryptoService.serializeEncryptedJSON(encFilename)
                }
                let attachmentsJSON = try JSONEncoder().encode(reEncryptedAttachments)
                let attachmentsString = String(data: attachmentsJSON, encoding: .utf8) ?? "[]"

                guard var record = try noteRepo.getRaw(id: note.id) else { continue }
                record.content = contentJSON
                record.tags = tagsJSON
                record.attachments = attachmentsString
                record.needsSync = true
                try noteRepo.updateRaw(record)
            } catch {
                print("[ReEncrypt] Skipping note \(note.id): \(error)")
            }
        }

        // Re-encrypt attachment blobs
        if let attachmentRepo {
            let blobIds = try attachmentRepo.listBlobIds()
            for blobId in blobIds {
                guard let blobData = try attachmentRepo.getBlob(id: blobId) else { continue }
                let blobStr = String(data: blobData, encoding: .utf8) ?? ""
                guard !blobStr.isEmpty else { continue }
                do {
                    let encrypted = try CryptoService.parseEncryptedJSON(blobStr)
                    let plainData = try CryptoService.decrypt(encrypted, key: oldKey)
                    let reEncrypted = try CryptoService.encrypt(plainData, key: newKey)
                    let reEncryptedJSON = try CryptoService.serializeEncryptedJSON(reEncrypted)
                    try attachmentRepo.updateBlobData(id: blobId, data: Data(reEncryptedJSON.utf8))
                } catch {
                    print("[ReEncrypt] Skipping blob \(blobId): \(error)")
                }
            }
        }

        // Re-encrypt version snapshots
        for noteId in Set(allNotes.map(\.id)) {
            do {
                let versions = try versionRepo.getVersions(noteId: noteId)
                for ver in versions {
                    do {
                        let encContent = try CryptoService.parseEncryptedJSON(ver.content)
                        let plainContent = try CryptoService.decryptText(encContent, key: oldKey)
                        let newEncContent = try CryptoService.encryptText(plainContent, key: newKey)

                        let encTags = try CryptoService.parseEncryptedJSON(ver.tags)
                        let plainTagsStr = try CryptoService.decryptText(encTags, key: oldKey)
                        let newEncTags = try CryptoService.encryptText(plainTagsStr, key: newKey)

                        var updated = ver
                        updated.content = try CryptoService.serializeEncryptedJSON(newEncContent)
                        updated.tags = try CryptoService.serializeEncryptedJSON(newEncTags)
                        try versionRepo.insertOrReplace(updated)
                    } catch {
                        print("[ReEncrypt] Skipping version \(ver.versionKey): \(error)")
                    }
                }
            } catch {
                print("[ReEncrypt] Skipping versions for note \(noteId): \(error)")
            }
        }

        // Re-encrypt saved searches
        if let savedSearchRepo {
            do {
                let searches = try savedSearchRepo.listAll(key: oldKey)
                for search in searches {
                    do {
                        let encName = try CryptoService.encryptText(search.name, key: newKey)
                        let encQuery = try CryptoService.encryptText(search.query, key: newKey)
                        try savedSearchRepo.updateEncrypted(
                            id: search.id,
                            name: try CryptoService.serializeEncryptedJSON(encName),
                            query: try CryptoService.serializeEncryptedJSON(encQuery)
                        )
                    } catch {
                        print("[ReEncrypt] Skipping saved search \(search.id): \(error)")
                    }
                }
            } catch {
                print("[ReEncrypt] Failed to list saved searches: \(error)")
            }
        }

        print("[ReEncrypt] All data re-encrypted successfully")
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
