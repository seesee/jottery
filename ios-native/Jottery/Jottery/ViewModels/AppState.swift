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

    var notes: [DecryptedNote] = [] {
        didSet { rebuildAllTags() }
    }
    var archivedNotes: [DecryptedNote] = [] {
        didSet { rebuildAllTags() }
    }
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

    /// Set when a Face ID unlock finds that the account has no wrapped key on
    /// the server. `MainView` then asks for the notes password once, so the key
    /// can be published (see `checkAccountKeyAfterBiometricUnlock()`).
    var needsNotesPasswordForAccountKey = false

    /// Set when a legacy vault finds that the account holds a different key.
    /// `MainView` then asks whether to move the vault onto it, because that
    /// re-encrypts everything and uploads it (see `mergeOntoAccountKey(_:)`).
    var pendingAccountKeyMerge: PendingAccountKeyMerge?

    /// Something about the account's key the user needs to know, shown by
    /// `MainView`. Nothing here stops the app working.
    var accountKeyNotice: AccountKeyNotice?

    /// Notes in use that the server listed as permanently deleted, when there
    /// are enough of them to be suspicious. They're in the bin, and `MainView`
    /// offers to put them back (see `restoreNotesRemovedByServer()`).
    var notesRemovedByServer: [String] = []
    /// Offer to turn on Face ID. Set here, not in the unlock screen, because a
    /// successful unlock replaces that screen before an alert on it could show.
    var offersFaceID = false

    /// Set while the whole vault is being re-encrypted, which can take a while.
    /// `ContentView` covers the app, and nothing may be encrypted until it's
    /// done: under the old key it would be unreadable once the switch is made.
    var isReKeying = false

    // MARK: - User-Facing Errors

    /// A transient, localised error message surfaced to the user via a
    /// toast (see `ToastView`), e.g. "Couldn't delete note." Set by
    /// `reportError(_:duration:)`, which also schedules it to auto-clear.
    var userErrorMessage: String?

    /// How a message waits out its `duration`. Tests replace this, to decide
    /// when a timer fires rather than race it.
    @ObservationIgnored var waitOutErrorMessage: @Sendable (Duration) async -> Void = { duration in
        try? await Task.sleep(for: duration)
    }

    #if DEBUG
    /// Demo screenshot mode: presents the settings sheet (see DemoSeedService).
    var demoShowSettings = false
    #endif

    // MARK: - Quick Actions

    var pendingQuickAction: String?
    var searchFocused: Bool = false

    // MARK: - Note List

    /// Never stored, and cleared at lock (see `RecentSearches`).
    var recentSearches = RecentSearches()
    /// How many notes match the search in the view that isn't showing, counted
    /// when the one that is showing has none.
    var matchesInOtherView = 0
    /// Opened from the toolbar or a keyboard command, and presented by `MainView`.
    var presentedSheet: AppSheet?
    /// Goes up when something needs the list on screen, as it may not be: see
    /// `focusSearch()`.
    var listRevealRequests = 0

    // MARK: - Lifecycle

    var backgroundedAt: ContinuousClock.Instant?

    // MARK: - Colours

    /// Category names and tag colours, decrypted. Empty while locked.
    var noteColours = NoteColourSettings()

    // MARK: - Backup Reminders

    /// Replaced in tests, which mustn't reach the real notification centre.
    @ObservationIgnored var backupReminders: any BackupReminderCentre = SystemBackupReminderCentre()

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
    /// Swept at launch, lock and wipe, so nothing decrypted outlives the session.
    @ObservationIgnored var temporaryFiles = TemporaryFiles.shared
    @ObservationIgnored var attachmentThumbnails = AttachmentThumbnails.shared
    @ObservationIgnored private var networkMonitor: NWPathMonitor?
    @ObservationIgnored private var networkWasSatisfied = true
    @ObservationIgnored private var autoSyncTask: Task<Void, Never>?
    /// Waits out the delay before a scheduled sync. Only replaced by tests.
    @ObservationIgnored var autoSyncPause: @Sendable (Duration) async -> Void = { try? await Task.sleep(for: $0) }
    /// Retries in a row after failures worth retrying (see `SyncRetry`).
    @ObservationIgnored private var syncRetryAttempts = 0
    @ObservationIgnored private var userErrorClearTask: Task<Void, Never>?

    /// Re-entrancy guard for `unlock(password:)` — checked-and-set at entry,
    /// cleared on every exit (including throw) via `defer`. Two concurrent
    /// unlock attempts (e.g. a biometric prompt racing a manual password
    /// entry) must have exactly one real effect (jottery-t94v).
    @ObservationIgnored private var isUnlocking = false

    /// Monotonically increasing ticket source shared by every refresh
    /// operation — both full reloads (`loadNotes()`) and incremental applies
    /// (`applySyncChanges(_:)`). Each refresh draws its own ticket from this
    /// counter at the very start, on the main actor, before any `await` —
    /// so ticket order always matches start order, regardless of which kind
    /// of refresh it is (jottery-t94v).
    @ObservationIgnored private var loadTicketCounter = 0

    /// The ticket of the most recently *published* refresh. A refresh may
    /// only publish if its ticket is newer than this — see
    /// `publishIfNewest(ticket:_:)`. Using `>` rather than `==` (the old
    /// epoch scheme) means a full reload and an incremental refresh that
    /// started around the same time are compared by start order instead of
    /// by kind: whichever started later wins, whether it's the full reload
    /// or the incremental. The old epoch scheme only bumped on full
    /// reloads, so a full reload that started *before* a concurrent
    /// incremental could still publish *after* it and clobber the
    /// incremental's fresher rows with its own older snapshot.
    @ObservationIgnored private var lastPublishedTicket = 0

    /// A dropped publication can lose data: the refresh that beat it may not
    /// have covered every id the dropped one touched. A dropped *full*
    /// reload obviously needs a retry (its own ids may be stale again). A
    /// dropped *incremental* also needs one — the refresh that beat it only
    /// covers all ids for certain if that refresh was itself a full reload;
    /// if it was a later-starting incremental, it may not have touched the
    /// ids the dropped one did. So any dropped publication, full or
    /// incremental, schedules exactly one follow-up full reload. This flag
    /// stops multiple drops in quick succession from each queuing their own
    /// retry (jottery-t94v).
    @ObservationIgnored private var isRetryReloadPending = false

    /// Draws the next ticket for a refresh operation. Must be called on the
    /// main actor before any `await` in the caller, so it captures start
    /// order.
    func nextLoadTicket() -> Int {
        loadTicketCounter += 1
        return loadTicketCounter
    }

    // Repositories (lazily initialised after DB is ready)
    private(set) var noteRepo: NoteRepository?
    private(set) var encryptionRepo: EncryptionRepository?
    private(set) var settingsRepo: SettingsRepository?
    private(set) var syncRepo: SyncRepository?
    @ObservationIgnored private var filenameCleanup: Task<Void, Never>?
    /// False from launch until the database has been tried, so that nothing decides
    /// which screen to show from the defaults.
    private(set) var isDatabaseReady = false
    @ObservationIgnored private var isOpeningDatabase = false
    /// Replaced by tests, to hold the database open partway.
    @ObservationIgnored var openDatabase: @Sendable () throws -> DatabaseManager = { try DatabaseManager() }
    /// Replaced by tests, which share the simulator's keychain.
    @ObservationIgnored var forgetEarlierInstall: () -> Void = KeychainService.deleteAll
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
        let order = sortOrder

        // No debounce needed when query is empty — just sort (cheap).
        guard !query.trimmingCharacters(in: .whitespaces).isEmpty else {
            filteredNotes = SearchService.sort(notes: displayedNotes, order: order)
            matchesInOtherView = 0
            return
        }

        // Both lists, because `archived` reaches whichever one isn't showing.
        let source = notes + archivedNotes
        let archiveMode = showArchive
        let colours = noteColours

        searchTask = Task {
            // Debounce while typing
            try? await Task.sleep(for: .milliseconds(150))
            guard !Task.isCancelled else { return }

            // Run expensive filter + sort off the main thread
            let (results, elsewhere) = await Task.detached(priority: .userInitiated) {
                let filtered = SearchService.filter(notes: source, query: query, archiveMode: archiveMode, colours: colours)
                let elsewhere = filtered.isEmpty
                    ? SearchService.filter(notes: source, query: query, archiveMode: !archiveMode, colours: colours).count
                    : 0
                return (SearchService.sort(notes: filtered, order: order), elsewhere)
            }.value

            guard !Task.isCancelled else { return }
            self.filteredNotes = results
            self.matchesInOtherView = elsewhere
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
    ///
    /// Memoised (jottery-fxmq): `TagInputView` reads this on every keystroke,
    /// and it used to `flatMap` both note arrays on every access — O(notes)
    /// per character typed. Rebuilding is now driven by `didSet` on
    /// `notes`/`archivedNotes` instead: those are the only two properties
    /// `allTags` derives from, and Swift fires `didSet` for *every* mutation
    /// of a stored array property — full reassignment (`notes = active`),
    /// `insert`/`removeAll`, and in-place element writes (`notes[i] = x`)
    /// alike. That means every current and future publication site (loadNotes,
    /// applySyncChanges, saveNote, createNote, delete/archive/restore/
    /// unarchive, importSharedInboxItems) invalidates `allTags` for free,
    /// without each one needing its own call to a rebuild function — the
    /// fewest-touch-points option versus threading an explicit
    /// `rebuildAllTags()` call (or invalidation-flag set) through every one
    /// of those call sites individually. A plain read is then an O(1)
    /// stored-property access, so per-keystroke cost is back to a lookup,
    /// not a rebuild.
    private(set) var allTags: [String] = []

    private func rebuildAllTags() {
        let tagSets = notes.flatMap(\.tags) + archivedNotes.flatMap(\.tags)
        allTags = Array(Set(tagSets)).sorted()
    }

    var selectedNote: DecryptedNote? {
        guard let id = selectedNoteId else { return nil }
        return notes.first { $0.id == id }
    }

    // MARK: - Initialisation

    /// Open the database, off the main thread. iOS kills an app that takes over 10
    /// seconds to start, and opening can take that long: a migration, or a checkpoint
    /// of a large write-ahead log, scales with the vault. Killed partway, the same
    /// work starts again at the next launch, so it can stop the app opening for good.
    func initialise() async {
        guard db == nil, !isOpeningDatabase else { return }
        isOpeningDatabase = true
        defer { isOpeningDatabase = false }

        removeTemporaryFiles()

        let open = openDatabase
        do {
            let database = try await withCheckedThrowingContinuation { continuation in
                DispatchQueue.global(qos: .userInitiated).async {
                    continuation.resume(with: Result { try open() })
                }
            }
            try initialise(database: database, forgetEarlierInstall: forgetEarlierInstall)
            clearOldFilenamesInBackground()
        } catch {
            // Database init failed — stay on first launch screen
            isFirstLaunch = true
            isDatabaseReady = true
        }
    }

    /// Blank the plaintext filenames older builds stored, a few rows at a time. Each
    /// row's attachment data gets rewritten with it, so this keeps out of the way of
    /// everything else, and picks up at the next launch if it doesn't finish.
    private func clearOldFilenamesInBackground() {
        guard let attachmentRepo else { return }
        filenameCleanup = Task.detached(priority: .background) {
            while !Task.isCancelled, (try? attachmentRepo.clearOldFilenames(limit: 5)) ?? 0 > 0 {
                try? await Task.sleep(for: .milliseconds(100))
            }
        }
    }

    /// Remove every file that held something on its way in or out of the vault,
    /// and the pictures of attachments kept in memory.
    private func removeTemporaryFiles() {
        temporaryFiles.removeAll()
        TemporaryFiles.removeLeftovers()
        attachmentThumbnails.removeAll()
    }

    /// Wire repositories to a specific database. Internal so tests can
    /// inject a temporary database instead of the app-support default.
    ///
    /// `forgetEarlierInstall` runs when the database opened and holds no vault:
    /// an earlier install's Keychain items are all that could be left, and they
    /// include its master key and API key. A database that can't be opened, as
    /// on a locked device, throws before that, and says nothing about the vault.
    func initialise(database: DatabaseManager, forgetEarlierInstall: () -> Void = {}) throws {
        // Wire auto-lock so the timer triggers a full app lock (UI + key wipe)
        keyManager.onAutoLock = { [weak self] in
            self?.lock()
        }

        VaultFromBackup.removeLeftovers(besides: database)

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
        isDatabaseReady = true
        if !hasVault { forgetEarlierInstall() }

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
    ///
    /// Key derivation (PBKDF2, ~600k iterations) and the dependent
    /// wrap/verification-token encrypt run inside `Task.detached`, mirroring
    /// `unlock(password:)` — only plain `Data`/`String` values cross back to
    /// the MainActor, where `keyManager` (MainActor-bound) is updated and the
    /// resulting metadata is persisted.
    func createVault(password: String, existingSalt: Data? = nil, existingIterations: UInt32? = nil) async throws {
        guard let encryptionRepo else { throw AppStateError.notInitialised }

        if existingSalt != nil {
            // Legacy import path — use provided salt/iterations for compatibility
            let salt = existingSalt!
            let iterations = existingIterations ?? CryptoService.defaultIterations

            let (keyData, verificationJSON) = try await Task.detached(priority: .userInitiated) {
                let key = try CryptoService.deriveKey(password: password, salt: salt, iterations: iterations)

                let verificationEncrypted = try CryptoService.encryptText(
                    EncryptionMetadata.verificationPlaintext, key: key
                )
                let verificationJSON = try CryptoService.serializeEncryptedJSON(verificationEncrypted)

                let keyData = key.withUnsafeBytes { Data($0) }
                return (keyData, verificationJSON)
            }.value

            let metadata = EncryptionMetadata.new(
                salt: salt, iterations: iterations, verification: verificationJSON
            )
            try encryptionRepo.store(metadata)

            keyManager.unlockWithKeyData(keyData)
        } else {
            // New vault — use envelope encryption from the start
            let masterKeyData = CryptoService.generateMasterKey()

            // Generate device salt; the expensive device-key derivation plus
            // the dependent wrap/verification-encrypt steps run off-main.
            let deviceSalt = CryptoService.generateSalt()

            let (localBlob, verificationJSON) = try await Task.detached(priority: .userInitiated) {
                let deviceKey = try CryptoService.deriveKey(
                    password: password,
                    salt: deviceSalt,
                    iterations: CryptoService.defaultIterations
                )

                // Wrap master key locally
                let localWrapped = try CryptoService.wrapMasterKey(masterKeyData, with: deviceKey)
                let localBlob = try CryptoService.serializeEncryptedJSON(localWrapped)

                // Create verification token encrypted with the master key
                let masterKey = SymmetricKey(data: masterKeyData)
                let verificationEncrypted = try CryptoService.encryptText(
                    EncryptionMetadata.verificationPlaintext, key: masterKey
                )
                let verificationJSON = try CryptoService.serializeEncryptedJSON(verificationEncrypted)

                return (localBlob, verificationJSON)
            }.value

            // One write: a vault stopped partway through would read as a legacy
            // vault whose token never matches, and refuse every password.
            try encryptionRepo.store(EncryptionMetadata.newEnvelope(
                deviceSalt: deviceSalt, localWrappedMaster: localBlob, verification: verificationJSON
            ))

            keyManager.unlockWithKeyData(masterKeyData)
        }

        isFirstLaunch = false
        isLocked = false

        // Load notes (should be empty for new vault)
        try await loadNotes()
    }

    // MARK: - Unlock

    /// Attempt to unlock with the given password.
    /// Returns true if successful.
    ///
    /// Ordering (preserved from PR #98): derive → verify → set key (on the
    /// MainActor) → flush pending editor note → `loadNotes()` → `isLocked = false`.
    /// Key derivation (PBKDF2, ~600k iterations) and the verification-token
    /// decrypt are the expensive steps, so both run inside `Task.detached` —
    /// `keyManager` is MainActor-bound and is only touched after the detached
    /// work hands back plain `Data`.
    @discardableResult
    func unlock(password: String) async throws -> Bool {
        // Re-entrancy guard: the check-and-set below runs synchronously (no
        // `await` yet), so on MainActor exactly one concurrent caller wins —
        // the other returns false immediately rather than racing a second
        // derive/verify/loadNotes cycle against the first (jottery-t94v).
        guard !isUnlocking else {
            Log.debug("[Unlock] already in progress — ignoring re-entrant call")
            return false
        }
        isUnlocking = true
        defer { isUnlocking = false }

        guard let encryptionRepo else { throw AppStateError.notInitialised }

        guard let metadata = try encryptionRepo.get() else {
            throw AppStateError.noVault
        }

        #if DEBUG
        let unlockStart = CFAbsoluteTimeGetCurrent()
        #endif

        var masterKeyData: Data

        if metadata.envelopeVersion != nil {
            // Envelope unlock path — derive device key, unwrap master key, and
            // verify, all off the main thread; only the resulting key bytes
            // cross back to the MainActor.
            guard let deviceSaltB64 = metadata.deviceSalt,
                  let deviceSalt = Data(base64Encoded: deviceSaltB64),
                  let wrappedJSON = metadata.localWrappedMaster else {
                throw AppStateError.invalidSalt
            }
            let verificationJSON = metadata.verification

            do {
                masterKeyData = try await Task.detached(priority: .userInitiated) {
                    #if DEBUG
                    let deriveStart = CFAbsoluteTimeGetCurrent()
                    #endif
                    let deviceKey = try CryptoService.deriveKey(
                        password: password,
                        salt: deviceSalt,
                        iterations: CryptoService.defaultIterations
                    )
                    #if DEBUG
                    Log.debug(String(format: "[Unlock] derive: %.1fms", (CFAbsoluteTimeGetCurrent() - deriveStart) * 1000))
                    let verifyStart = CFAbsoluteTimeGetCurrent()
                    #endif

                    let wrapped = try CryptoService.parseEncryptedJSON(wrappedJSON)
                    let unwrapped: Data
                    do {
                        unwrapped = try CryptoService.unwrapMasterKey(wrapped, with: deviceKey)
                    } catch {
                        throw AppStateError.wrongPassword
                    }

                    if let verificationJSON {
                        let candidateKey = SymmetricKey(data: unwrapped)
                        guard AppState.verifyWithToken(verificationJSON, key: candidateKey) else {
                            throw AppStateError.wrongPassword
                        }
                    }
                    #if DEBUG
                    Log.debug(String(format: "[Unlock] verify: %.1fms", (CFAbsoluteTimeGetCurrent() - verifyStart) * 1000))
                    #endif

                    return unwrapped
                }.value
            } catch {
                if let appError = error as? AppStateError, case .wrongPassword = appError {
                    keyManager.lock()
                    isLocked = true
                }
                throw error
            }

            keyManager.unlockWithKeyData(masterKeyData)
        } else {
            // Legacy unlock path — derive key directly from password + salt,
            // then verify, both off the main thread.
            guard let saltData = metadata.saltData else {
                throw AppStateError.invalidSalt
            }
            let iterations = UInt32(metadata.iterations)
            let verificationJSON = metadata.verification
            let db = self.db

            do {
                masterKeyData = try await Task.detached(priority: .userInitiated) {
                    #if DEBUG
                    let deriveStart = CFAbsoluteTimeGetCurrent()
                    #endif
                    let key = try CryptoService.deriveKey(password: password, salt: saltData, iterations: iterations)
                    #if DEBUG
                    Log.debug(String(format: "[Unlock] derive: %.1fms", (CFAbsoluteTimeGetCurrent() - deriveStart) * 1000))
                    let verifyStart = CFAbsoluteTimeGetCurrent()
                    #endif

                    let verified: Bool
                    if let verificationJSON {
                        verified = AppState.verifyWithToken(verificationJSON, key: key)
                    } else {
                        verified = AppState.verifyByDecryptingNote(key: key, db: db)
                    }
                    #if DEBUG
                    Log.debug(String(format: "[Unlock] verify: %.1fms", (CFAbsoluteTimeGetCurrent() - verifyStart) * 1000))
                    #endif

                    guard verified else {
                        throw AppStateError.wrongPassword
                    }

                    return key.withUnsafeBytes { Data($0) }
                }.value
            } catch {
                keyManager.lock()
                isLocked = true
                throw error
            }

            keyManager.unlockWithKeyData(masterKeyData)
        }

        let key = SymmetricKey(data: masterKeyData)
        masterKeyData.wipe()

        if biometricKeyNeedsRefresh {
            biometricKeyNeedsRefresh = false
            if keyManager.isBiometricEnabled {
                try? keyManager.enableBiometricUnlock()
            }
        }

        // Retry any edit that failed to flush at lock time now that the key
        // is available again.
        flushPendingEditorNote()
        await repairStoredData()

        #if DEBUG
        let loadStart = CFAbsoluteTimeGetCurrent()
        #endif
        try await loadNotes()
        #if DEBUG
        Log.debug(String(format: "[Unlock] loadNotes: %.1fms", (CFAbsoluteTimeGetCurrent() - loadStart) * 1000))
        Log.debug(String(format: "[Unlock] total: %.1fms", (CFAbsoluteTimeGetCurrent() - unlockStart) * 1000))
        #endif
        isLocked = false

        // Auto-purge deleted notes older than 30 days
        let purged = (try? noteRepo?.purgeOldDeletedNotes()) ?? 0
        if purged > 0 {
            Log.debug("[Purge] Removed \(purged) deleted note(s) older than 30 days")
        }

        Log.debug("[Sync] unlock: calling setupSync()")
        setupSync()
        scheduleSearchWarmUp()
        importSharedInboxItems()

        // Upgrade old vaults: store a verification token if missing
        if metadata.verification == nil {
            try? upgradeVaultVerification(key: key)
        }

        // Background envelope setup (non-fatal). Legacy vaults are migrated;
        // envelope vaults have the server key published if it is missing, which
        // repairs accounts whose first device never uploaded one.
        if settings.syncEnabled {
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

    /// Clean up what older builds left in the vault. Runs on every unlock, and
    /// before sync starts, so the first push carries the repairs.
    func repairStoredData() async {
        guard let noteRepo, let key = keyManager.masterKey else { return }
        await Task.detached(priority: .userInitiated) {
            do {
                let repaired = try noteRepo.encryptPlaintextAttachmentFilenames(key: key)
                if repaired > 0 {
                    Log.debug("[Repair] Encrypted attachment filenames in \(repaired) note(s)")
                }
            } catch {
                Log.debug("[Repair] Couldn't encrypt attachment filenames: \(error)")
            }
        }.value
    }

    /// Whether the key now loaded is this vault's key, checked against the
    /// vault's verification token. Face ID keeps its own copy of the master
    /// key; if the vault moved to a new key and that copy was not refreshed
    /// (the app killed in between), Face ID would load the old key: the vault
    /// would look empty and new notes would be saved under the wrong key.
    /// Vaults too old to have a token cannot be checked, and pass.
    func loadedKeyMatchesVault() -> Bool {
        guard let key = keyManager.masterKey else { return false }
        guard let token = (try? encryptionRepo?.get())?.verification else { return true }
        return AppState.verifyWithToken(token, key: key)
    }

    /// Set when Face ID loaded an out-of-date key. The next password unlock
    /// then stores the current key for Face ID.
    @ObservationIgnored var biometricKeyNeedsRefresh = false

    /// Verify a candidate vault (notes) password against the currently
    /// stored vault metadata, without any side effects — no key is loaded
    /// into `keyManager`, `isLocked` is untouched, `loadNotes()` isn't
    /// called. Mirrors the derive → verify half of `unlock(password:)` for
    /// both the envelope and legacy metadata shapes.
    ///
    /// Used before wrapping the envelope master key with a password (e.g.
    /// `SyncSetupView`'s "Notes password" field) — getting that wrong
    /// silently uploads a key wrapped with the wrong secret, which every
    /// other device fails to onboard from (jottery-md6b). Key derivation
    /// (PBKDF2, ~600k iterations) runs off the main thread via
    /// `Task.detached`, matching `unlock(password:)`.
    func verifyVaultPassword(_ password: String) async -> Bool {
        guard let encryptionRepo, let metadata = try? encryptionRepo.get() else { return false }

        if metadata.envelopeVersion != nil {
            guard let deviceSaltB64 = metadata.deviceSalt,
                  let deviceSalt = Data(base64Encoded: deviceSaltB64),
                  let wrappedJSON = metadata.localWrappedMaster else {
                return false
            }
            let verificationJSON = metadata.verification

            return await Task.detached(priority: .userInitiated) {
                guard let deviceKey = try? CryptoService.deriveKey(
                    password: password,
                    salt: deviceSalt,
                    iterations: CryptoService.defaultIterations
                ) else { return false }
                guard let wrapped = try? CryptoService.parseEncryptedJSON(wrappedJSON),
                      let unwrapped = try? CryptoService.unwrapMasterKey(wrapped, with: deviceKey) else {
                    return false
                }
                guard let verificationJSON else { return true }
                let candidateKey = SymmetricKey(data: unwrapped)
                return AppState.verifyWithToken(verificationJSON, key: candidateKey)
            }.value
        } else {
            guard let saltData = metadata.saltData else { return false }
            let iterations = UInt32(metadata.iterations)
            let verificationJSON = metadata.verification
            let db = self.db

            return await Task.detached(priority: .userInitiated) {
                guard let key = try? CryptoService.deriveKey(password: password, salt: saltData, iterations: iterations) else {
                    return false
                }
                if let verificationJSON {
                    return AppState.verifyWithToken(verificationJSON, key: key)
                }
                return AppState.verifyByDecryptingNote(key: key, db: db)
            }.value
        }
    }

    /// Verify the key by decrypting the stored verification token.
    /// `static` (and takes no `self`) so it can run inside `Task.detached`
    /// without capturing the MainActor-isolated `AppState` instance.
    nonisolated private static func verifyWithToken(_ json: String, key: SymmetricKey) -> Bool {
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
    /// `static` with an explicit `db` parameter for the same reason as
    /// `verifyWithToken` above — safe to call from a detached task.
    nonisolated private static func verifyByDecryptingNote(key: SymmetricKey, db: DatabaseManager?) -> Bool {
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

    /// Flush the in-flight editor state. Returns true when persisted (or
    /// when there was nothing pending).
    ///
    /// Kept in memory on failure — the plaintext of the user's own unsaved
    /// edit is a smaller risk than silently destroying it at lock time; the
    /// retry happens on the next successful unlock.
    @discardableResult
    internal func flushPendingEditorNote() -> Bool {
        guard let pending = pendingEditorNote else { return true }
        guard !isReKeying, let noteRepo, let key = keyManager.masterKey else { return false }
        do {
            try noteRepo.update(pending, key: key)
            pendingEditorNote = nil
            return true
        } catch {
            Log.debug("[Lock] failed to flush pending edit: \(error)")
            return false
        }
    }

    /// Lock the application.
    func lock() {
        Log.debug("[Sync] lock: clearing syncService, syncEnabled=false")
        // Flush any pending editor save while the key is still available.
        // On failure the pending note is retained in memory and retried on
        // the next successful unlock (see flushPendingEditorNote).
        flushPendingEditorNote()

        teardownSync()

        keyManager.lock()
        isLocked = true
        removeTemporaryFiles()
        needsNotesPasswordForAccountKey = false
        pendingAccountKeyMerge = nil
        accountKeyNotice = nil
        notesRemovedByServer = []
        // A refresh decrypting right now holds an older ticket, so it can't put
        // what it decrypted back in memory behind the lock screen.
        lastPublishedTicket = nextLoadTicket()
        searchTask?.cancel()
        notes = []
        archivedNotes = []
        filteredNotes = []
        savedSearches = []
        noteColours = NoteColourSettings()
        pendingConflicts = []
        inboxItems = []
        selectedNoteId = nil
        recentSearches.clear()
        presentedSheet = nil
    }

    /// Stop everything that talks to the server and let go of the sync objects,
    /// so a revoked key stops being used and the master key they hold is
    /// released. Used whenever sync ends: lock, disconnect, deleting the
    /// account, and wiping the vault.
    func teardownSync() {
        stopNetworkMonitor()
        autoSyncTask?.cancel()
        autoSyncTask = nil
        if let service = syncService {
            Task { await service.stop() }
        }
        syncClient = nil
        syncService = nil
        syncEnabled = false
    }

    // MARK: - Account Key (Face ID Unlock)

    enum AccountKeyPublishResult: Equatable {
        case published
        case wrongPassword
        case failed
    }

    /// After a Face ID unlock, check whether the account's wrapped key is
    /// missing from the server, and if so flag that the notes password is
    /// needed to publish it.
    ///
    /// A password unlock publishes a missing key during envelope setup, but
    /// Face ID has no password to wrap the key with. A device that only ever
    /// unlocked with Face ID therefore never published, and other devices could
    /// not connect (jottery-x40w). The check itself needs no password. Server
    /// or network errors are ignored: the check runs again on the next unlock.
    func checkAccountKeyAfterBiometricUnlock() async {
        guard settings.syncEnabled,
              let syncClient,
              let userId = (try? syncRepo?.getMetadata())?.userId,
              !userId.isEmpty else {
            return
        }
        do {
            let serverKey = try await syncClient.getWrappedKey()
            // The user may have locked again while the request was in flight.
            if serverKey == nil && !isLocked {
                Log.debug("[Envelope] Face ID unlock: server has no wrapped key — asking for the notes password")
                needsNotesPasswordForAccountKey = true
            }
        } catch {
            Log.debug("[Envelope] Face ID unlock: wrapped key check failed (will retry next unlock): \(error)")
        }
    }

    /// Publish the account key with the notes password the user has just
    /// entered in response to `needsNotesPasswordForAccountKey`. Runs the same
    /// envelope setup as a password unlock, so an existing server key is never
    /// overwritten.
    func publishMissingAccountKey(password: String) async -> AccountKeyPublishResult {
        guard await verifyVaultPassword(password) else { return .wrongPassword }
        guard let masterKey = keyManager.masterKey else { return .failed }
        let ok = await EnvelopeService.tryEnvelopeSetup(
            appState: self,
            password: password,
            masterKey: masterKey
        )
        guard ok else { return .failed }
        needsNotesPasswordForAccountKey = false
        return .published
    }

    /// The user agreed to move this vault onto the account's key.
    func mergeOntoAccountKey(_ pending: PendingAccountKeyMerge) async {
        pendingAccountKeyMerge = nil
        do {
            try await switchVaultToMasterKey(pending.accountKey, notesPassword: pending.notesPassword)
        } catch {
            Log.debug("[Envelope] Merging onto the account key failed: \(error)")
            reportError(L.syncMergeFailedOnUnlock)
        }
    }

    // MARK: - User-Facing Errors

    /// Publish a localised, user-facing error message (e.g. "Couldn't
    /// delete note") and schedule it to auto-clear after `duration`.
    /// Overwriting with a newer message cancels any earlier clear-timer, so
    /// a slow-to-fire old timer can never wipe out a message that replaced
    /// it (mirrors the `syncStatusMessage` auto-clear pattern used by
    /// `triggerSync()`/`forceFullSync()`). `duration` defaults to 4s but is
    /// overridable — tests pass a short duration instead of a fake clock.
    func reportError(_ message: String, duration: Duration = .seconds(4)) {
        userErrorMessage = message
        userErrorClearTask?.cancel()
        userErrorClearTask = Task { @MainActor [weak self, waitOutErrorMessage] in
            await waitOutErrorMessage(duration)
            guard !Task.isCancelled else { return }
            if self?.userErrorMessage == message {
                self?.userErrorMessage = nil
            }
        }
    }

    // MARK: - Notes CRUD

    func loadNotes() async throws {
        guard let noteRepo, let key = keyManager.masterKey else { return }
        loadNoteColours(key: key)
        // Draw a ticket before the detached decrypt starts — on the main
        // actor, before any await — so it reflects this call's start order
        // relative to any other in-flight refresh (jottery-t94v).
        let ticket = nextLoadTicket()
        let savedSearchRepo = self.savedSearchRepo
        // Decrypting the whole vault is O(notes) — never on the main thread (jottery-bzar).
        let (active, archived, searches) = try await Task.detached(priority: .userInitiated) {
            (try noteRepo.listActive(key: key),
             try noteRepo.listArchived(key: key),
             (try? savedSearchRepo?.listAll(key: key)) ?? [])
        }.value

        let published = publishIfNewest(ticket: ticket) {
            notes = active
            archivedNotes = archived
            savedSearches = searches
        }
        guard published else {
            Log.debug("[Sync] loadNotes: superseded by a newer refresh (ticket \(ticket) <= \(lastPublishedTicket)) — discarding")
            await retryFullReloadIfNeeded()
            return
        }
        Log.debug("[Sync] loadNotes: decrypted \(active.count + archived.count) record(s) — full reload")
        scheduleSearch()
    }

    /// Test seam for the publish-ordering guard (jottery-t94v): runs
    /// `publish` only if `ticket` is newer than the last published ticket,
    /// i.e. no other refresh that started later has published since the
    /// caller drew `ticket` before its detached work. `loadNotes()` and
    /// `applySyncChanges(_:)` both funnel their publication through this
    /// single choke point, and both draw tickets from the same counter, so
    /// this compares start order across full reloads and incremental
    /// applies alike rather than favouring one kind over the other.
    /// Exercised directly in tests — deterministic — rather than by racing
    /// real `Task`s against `loadNotes()`, which is inherently
    /// timing-dependent and flaky.
    @discardableResult
    func publishIfNewest(ticket: Int, _ publish: () -> Void) -> Bool {
        guard ticket > lastPublishedTicket else { return false }
        publish()
        lastPublishedTicket = ticket
        return true
    }

    /// Runs after any dropped publication (full or incremental) to guarantee
    /// convergence — see the `isRetryReloadPending` doc comment for why both
    /// kinds need this, not just full reloads. Coalesces concurrent drops
    /// into a single follow-up `loadNotes()`.
    private func retryFullReloadIfNeeded() async {
        guard !isRetryReloadPending else {
            // This retry was dropped too, or is about to be. One more covers it.
            retryWasDropped = true
            return
        }
        isRetryReloadPending = true
        defer { isRetryReloadPending = false }
        for _ in 0..<Self.maxRetryReloads {
            retryWasDropped = false
            try? await loadNotes()
            guard retryWasDropped else { return }
        }
    }

    /// Something has to end this, and the next sync catches whatever it missed.
    private static let maxRetryReloads = 3
    @ObservationIgnored private var retryWasDropped = false

    /// Apply a change this device just made to the lists. A refresh already
    /// running read the database before that change, so it mustn't publish over
    /// it: drawing a ticket drops it, and its retry reads the change back.
    private func changeLists(_ change: () -> Void) {
        lastPublishedTicket = nextLoadTicket()
        change()
    }

    /// Change a note wherever it's held: it's in one list or the other.
    private func changeNoteInLists(id: String, _ change: (inout DecryptedNote) -> Void) {
        changeLists {
            if let index = notes.firstIndex(where: { $0.id == id }) {
                change(&notes[index])
            }
            if let index = archivedNotes.firstIndex(where: { $0.id == id }) {
                change(&archivedNotes[index])
            }
        }
    }

    /// Above this many changed ids, an incremental refresh isn't worth it —
    /// treat the change set as ambiguous/oversized and fall back to a full
    /// reload instead of decrypting them one by one.
    private static let maxIncrementalChangeCount = 100

    /// Refresh only the notes a sync cycle touched (`SyncChanges`) instead of
    /// decrypting and republishing the whole vault via `loadNotes()`. Falls
    /// back to a full reload when: the caller marks the change set ambiguous
    /// (`fullReloadRequired`), it's larger than `maxIncrementalChangeCount`
    /// ids, or a decrypt for one of the changed ids fails outright — a stale
    /// row is worse than one extra full reload, so any doubt goes to
    /// `loadNotes()`, which is always correct.
    func applySyncChanges(_ changes: SyncChanges) async {
        let totalIds = changes.updatedIds.count + changes.deletedIds.count
        guard !changes.fullReloadRequired, totalIds <= Self.maxIncrementalChangeCount else {
            try? await loadNotes()
            return
        }
        guard totalIds > 0 else { return }
        guard let noteRepo, let key = keyManager.masterKey else { return }

        // Draw a ticket before the detached decrypt starts — on the main
        // actor, before any await — exactly like loadNotes(). Both kinds of
        // refresh share the same counter, so whichever started later wins
        // the publish race, regardless of kind (jottery-t94v).
        let ticket = nextLoadTicket()

        let ids = changes.updatedIds
        // Decrypt just the changed records off the main thread, batched into
        // one detached task — mirrors loadNotes()'s off-main decrypt (jottery-bzar).
        let refreshed: [DecryptedNote]? = await Task.detached(priority: .userInitiated) {
            var result: [DecryptedNote] = []
            result.reserveCapacity(ids.count)
            for id in ids {
                guard let note = try? noteRepo.get(id: id, key: key) else { return nil }
                result.append(note)
            }
            return result
        }.value

        guard let refreshed else {
            // A decrypt failure for a changed id — don't risk leaving a
            // stale row, fall back to a full reload.
            try? await loadNotes()
            return
        }

        let deletedIds = changes.deletedIds
        let applied = publishIfNewest(ticket: ticket) {
            for note in refreshed {
                notes.removeAll { $0.id == note.id }
                archivedNotes.removeAll { $0.id == note.id }
                // Soft-deleted (or hard-deleted-but-still-listed) notes belong in
                // neither list — matches loadNotes()'s listActive/listArchived filters.
                guard !note.deleted else { continue }
                if note.archived {
                    archivedNotes.insert(note, at: 0)
                } else {
                    notes.insert(note, at: 0)
                }
            }

            for id in deletedIds {
                notes.removeAll { $0.id == id }
                archivedNotes.removeAll { $0.id == id }
            }
        }

        guard applied else {
            Log.debug("[Sync] applySyncChanges: superseded by a newer refresh (ticket \(ticket) <= \(lastPublishedTicket)) — discarding, retrying with a full reload")
            await retryFullReloadIfNeeded()
            return
        }

        Log.debug("[Sync] applySyncChanges: refreshed \(refreshed.count) record(s), removed \(changes.deletedIds.count) — incremental")
        scheduleSearch()
    }

    /// Resolve a note id for the detail pane, whichever list it lives in.
    /// Archived notes are absent from `notes` (archiveNote removes them), so a
    /// lookup limited to `notes` made archived rows unopenable (jottery-1rqp).
    func displayedNote(id: String) -> DecryptedNote? {
        notes.first(where: { $0.id == id }) ?? archivedNotes.first(where: { $0.id == id })
    }

    /// Ids and titles for the note links in `content`. Only a note that looks
    /// like it has links pays for this, or hands the titles to a web view.
    func noteTitles(linkedFrom content: String) -> [String: String] {
        guard content.contains("[[") || content.contains("(link:") || content.contains("(note:") else { return [:] }
        return Dictionary((notes + archivedNotes).map { ($0.id, $0.title) }, uniquingKeysWith: { first, _ in first })
    }

    /// Follow a note link, wherever the note is. It may have been deleted.
    func openLinkedNote(id: String) {
        if displayedNote(id: id) != nil {
            selectedNoteId = id
        } else {
            reportError(L.previewNoteNotFound)
        }
    }

    func createNote(content: String = "", tags: [String] = []) throws -> DecryptedNote? {
        guard !isReKeying else { throw AppStateError.vaultBusy }
        guard let noteRepo, let key = keyManager.masterKey else { return nil }
        let note = try noteRepo.create(
            content: content,
            tags: tags,
            // Text that came from somewhere else isn't a calculation because
            // new notes are, so this is only for a blank note.
            syntaxLanguage: content.isEmpty ? defaultNoteType : nil,
            key: key
        )
        changeLists { notes.insert(note, at: 0) }
        selectedNoteId = note.id
        scheduleAutoSync()
        return note
    }

    func saveNote(_ note: DecryptedNote) throws {
        guard !isReKeying else { throw AppStateError.vaultBusy }
        guard let noteRepo, let key = keyManager.masterKey else { return }
        try noteRepo.update(note, key: key)

        // Update in-memory
        var updated = note
        updated.modifiedAt = Date()
        changeNoteInLists(id: note.id) { $0 = updated }
        // The note list renders filteredNotes — recompute so title/preview/tags
        // update immediately, and push the change to the server once edits settle.
        scheduleSearch()
        scheduleAutoSync()
    }

    /// Save what the editor holds onto the note as it stands now, so a pin, a lock
    /// or new attachments that landed since the editor loaded it aren't undone.
    func saveEditorFields(_ fields: NoteEditorFields) throws {
        guard let latest = displayedNote(id: fields.noteId) else { return }
        try saveNote(fields.applied(to: latest))
    }

    /// Save what the editor holds, for the editor's own saves, which have no
    /// one to throw to. If it can't be saved, the edit stays staged and is
    /// retried at the next lock or unlock, and the user is told. Returns whether
    /// it was saved.
    @discardableResult
    func saveEditorEdit(_ fields: NoteEditorFields) -> Bool {
        do {
            try saveEditorFields(fields)
            if pendingEditorNote?.id == fields.noteId { pendingEditorNote = nil }
            return true
        } catch {
            Log.debug("[Editor] save failed: \(error)")
            if let latest = displayedNote(id: fields.noteId) {
                pendingEditorNote = fields.applied(to: latest)
            }
            reportError(L.errorCouldntSaveNote)
            return false
        }
    }

    /// Put the note as stored into version history, before an edit overwrites it.
    func keepCurrentVersion(ofNote id: String) throws {
        guard let noteRepo, let versionRepo, let record = try noteRepo.getRaw(id: id) else { return }
        try versionRepo.createVersion(from: record, reason: "pre-overwrite")
    }

    /// Make a new note from this one: its type, colour and attachments too, or a
    /// calc note would come back as plain markdown. Attachments are copied under
    /// new ids, because removing one deletes its blob, so sharing would take it
    /// from the other note as well. A pinned, locked or archived note's copy is
    /// none of those, as on the web.
    func duplicateNote(id: String) throws {
        guard !isReKeying else { throw AppStateError.vaultBusy }
        guard let noteRepo, let attachmentRepo, let key = keyManager.masterKey else { return }
        // What's in the editor counts, not what was last saved.
        flushPendingEditorNote()
        guard let original = try noteRepo.get(id: id, key: key) else { return }

        var attachments: [AttachmentRef] = []
        var newIds: [String: String] = [:]
        for ref in original.attachments {
            guard let blob = try attachmentRepo.getBlob(id: ref.data) else { continue }
            let newId = CryptoService.generateUUID()
            newIds[ref.id] = newId
            try attachmentRepo.storeBlob(id: newId, mimeType: ref.mimeType, size: ref.size, data: blob)
            attachments.append(AttachmentRef(
                id: newId,
                filename: try AttachmentFilename.encrypted(ref.filename, key: key),
                mimeType: ref.mimeType,
                size: ref.size,
                data: newId,
                thumbnailData: nil
            ))
        }

        let duplicate = try noteRepo.create(
            content: NoteCombining.content(original.content, followingCopies: newIds),
            tags: original.tags, copying: original, attachments: attachments, key: key
        )
        changeLists { notes.insert(duplicate, at: 0) }
        selectedNoteId = duplicate.id
        scheduleAutoSync()
    }

    func deleteNote(id: String) throws {
        guard let noteRepo, saveEditorEdits(toNotes: [id]) else { return }
        try noteRepo.softDelete(id: id)
        changeLists {
            notes.removeAll { $0.id == id }
            archivedNotes.removeAll { $0.id == id }
        }
        if selectedNoteId == id {
            selectedNoteId = nil
        }
        scheduleAutoSync()
    }

    /// Delete a note from the bin for good, on every device.
    func permanentlyDeleteNote(id: String) throws {
        guard let noteRepo else { return }
        try noteRepo.permanentlyDelete(id: id)
        scheduleAutoSync()
    }

    /// Put back what a sync moved to the bin (`notesRemovedByServer`). Pushing
    /// the notes again brings them back on the server too.
    func restoreNotesRemovedByServer() async {
        let ids = notesRemovedByServer
        notesRemovedByServer = []
        guard let noteRepo else { return }
        do {
            for id in ids { try noteRepo.restore(id: id) }
            try await loadNotes()
            scheduleAutoSync()
        } catch {
            reportError(L.errorCouldntRestoreNote)
        }
    }

    // MARK: - Many Notes at Once

    /// Move notes to the bin: one transaction and one reload, however many.
    func deleteNotes(ids: Set<String>) async throws {
        guard let noteRepo else { return }
        try await Task.detached(priority: .userInitiated) { try noteRepo.softDelete(ids: Array(ids)) }.value
        if let selected = selectedNoteId, ids.contains(selected) { selectedNoteId = nil }
        try await loadNotes()
        scheduleAutoSync()
    }

    func archiveNotes(ids: Set<String>) async throws {
        guard let noteRepo else { return }
        try await Task.detached(priority: .userInitiated) { try noteRepo.archive(ids: Array(ids)) }.value
        if let selected = selectedNoteId, ids.contains(selected) { selectedNoteId = nil }
        try await loadNotes()
        scheduleAutoSync()
    }

    /// Apply `change` to each of these notes, off the main thread, and reload
    /// once. `change` says whether it did anything. Returns how many failed.
    func updateNotes(
        ids: Set<String>,
        change: @escaping @Sendable (inout DecryptedNote) -> Bool
    ) async throws -> Int {
        guard !isReKeying else { throw AppStateError.vaultBusy }
        guard let noteRepo, let key = keyManager.masterKey else { return 0 }
        let selected = (notes + archivedNotes).filter { ids.contains($0.id) }
        let failures = await Task.detached(priority: .userInitiated) {
            var failures = 0
            for var note in selected where change(&note) {
                do {
                    try noteRepo.update(note, key: key)
                } catch {
                    failures += 1
                }
            }
            return failures
        }.value
        try await loadNotes()
        scheduleAutoSync()
        return failures
    }

    func restoreNote(id: String) async throws {
        guard let noteRepo else { return }
        try noteRepo.restore(id: id)
        try await loadNotes()
        scheduleAutoSync()
    }

    func togglePin(id: String) throws {
        guard let noteRepo else { return }
        try noteRepo.togglePin(id: id)
        changeNoteInLists(id: id) { $0.pinned.toggle() }
        scheduleSearch()
        scheduleAutoSync()
    }

    func archiveNote(id: String) throws {
        guard let noteRepo, saveEditorEdits(toNotes: [id]) else { return }
        try noteRepo.archive(id: id)
        changeLists { notes.removeAll { $0.id == id } }
        if selectedNoteId == id { selectedNoteId = nil }
        try loadArchivedNotes()
        scheduleAutoSync()
    }

    func unarchiveNote(id: String) async throws {
        guard let noteRepo else { return }
        try noteRepo.unarchive(id: id)
        changeLists { archivedNotes.removeAll { $0.id == id } }
        if selectedNoteId == id { selectedNoteId = nil }
        try await loadNotes()
        scheduleAutoSync()
    }

    func toggleLock(id: String) throws {
        guard let noteRepo else { return }
        try noteRepo.toggleLock(id: id)
        changeNoteInLists(id: id) {
            $0.locked.toggle()
            $0.lockedAt = $0.locked ? Date() : nil
        }
        scheduleSearch()
        scheduleAutoSync()
    }

    func loadArchivedNotes() throws {
        guard let noteRepo, let key = keyManager.masterKey else { return }
        let archived = try noteRepo.listArchived(key: key)
        changeLists { archivedNotes = archived }
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
        try? await loadNotes()
        // Sync immediately so keepLocal pushes the resolved note and keepBoth
        // pushes the duplicate — no manual refresh needed.
        await triggerSync()
    }

    // MARK: - Attachments

    func addAttachment(to noteId: String, url: URL, filename: String, mimeType: String) throws {
        try addAttachment(to: noteId, data: try Data(contentsOf: url), filename: filename, mimeType: mimeType)
    }

    /// The attachment as it was added, with its name in the clear, or nil if
    /// there's nothing to add it to.
    @discardableResult
    func addAttachment(
        to noteId: String,
        data fileData: Data,
        filename rawFilename: String,
        mimeType: String
    ) throws -> AttachmentRef? {
        guard !isReKeying else { throw AppStateError.vaultBusy }
        guard let noteRepo, let attachmentRepo, let key = keyManager.masterKey else { return nil }
        guard try noteRepo.get(id: noteId, key: key) != nil else { return nil }
        let filename = AttachmentFilename.safe(rawFilename)

        // Encrypt the file data
        let encrypted = try CryptoService.encrypt(fileData, key: key)
        let encryptedJSON = try CryptoService.serializeEncryptedJSON(encrypted)
        let blobData = Data(encryptedJSON.utf8)

        // Single ID for both metadata and blob — the server's attachments_data.id
        // is a FK to attachments_meta.id, so they must match.
        let attachmentId = CryptoService.generateUUID()

        // Store encrypted blob with the same ID used for the attachment ref
        try attachmentRepo.storeBlob(id: attachmentId, mimeType: mimeType, size: fileData.count, data: blobData)

        // Encrypt the filename for storage
        let encFilenameJSON = try AttachmentFilename.encrypted(filename, key: key)

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

        var added = ref
        added.filename = filename
        return added
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
        attachmentThumbnails.remove(attachmentId)

        refreshNoteFromStore(id: noteId)
    }

    /// Replace the in-memory copy of a note with the current database state.
    private func refreshNoteFromStore(id: String) {
        guard let noteRepo, let key = keyManager.masterKey else { return }
        guard let fresh = try? noteRepo.get(id: id, key: key) else { return }
        changeNoteInLists(id: id) { $0 = fresh }
        scheduleSearch()
        scheduleAutoSync()
    }

    // MARK: - Shared Inbox (share-extension hand-off)

    /// Convert items staged by the share extension into encrypted notes.
    /// Safe to call on every unlock/foreground — consumed items are removed.
    func importSharedInboxItems(from root: URL? = nil) {
        guard !isLocked, !isReKeying, let key = keyManager.masterKey, let noteRepo else { return }
        SharedInboxStore.removeAbandoned(in: root)

        // createNote selects the new note — imports must not steal selection.
        let previousSelection = selectedNoteId
        defer { selectedNoteId = previousSelection }

        for item in SharedInboxStore.pendingItems(in: root) {
            do {
                let note: DecryptedNote
                if let resumeId = item.manifest.importedNoteId {
                    // A previous pass already created the note but failed
                    // before finishing (e.g. an attachment threw) — resume
                    // onto it instead of creating a duplicate. If the user has
                    // deleted it since, they've answered: the rest goes too.
                    guard let resumed = try noteRepo.get(id: resumeId, key: key), !resumed.deleted else {
                        SharedInboxStore.remove(item.directory)
                        continue
                    }
                    note = resumed
                } else {
                    var lines: [String] = []
                    if let text = item.manifest.text, !text.isEmpty {
                        lines.append(text)
                    }
                    lines.append(contentsOf: item.manifest.urls)
                    let content = lines.joined(separator: "\n")

                    guard let created = try createNote(content: content, tags: ["shared"]) else { continue }
                    // Persist the note id before touching attachments — if
                    // anything below throws, a retry must resume onto this
                    // note rather than creating a duplicate.
                    try SharedInboxStore.markImported(item.directory, noteId: created.id)
                    note = created
                }

                // By count: two files can share a name, and both have to arrive.
                var alreadyAttached = Dictionary(grouping: note.attachments, by: \.filename).mapValues(\.count)
                for file in item.manifest.files {
                    let name = AttachmentFilename.safe(file.filename)
                    if let count = alreadyAttached[name], count > 0 {
                        alreadyAttached[name] = count - 1
                        continue
                    }
                    let fileURL = item.directory.appendingPathComponent(file.storedName)
                    let data = try Data(contentsOf: fileURL)
                    try addAttachment(to: note.id, data: data, filename: file.filename, mimeType: file.mimeType)
                }
                SharedInboxStore.remove(item.directory)
            } catch {
                // Leave the item staged so a later import can retry.
                Log.debug("[SharedInbox] import failed: \(error)")
            }
        }
    }

    // MARK: - Inbox

    func loadInboxItems() async throws {
        guard let syncClient else { return }
        inboxItems = try await syncClient.listInboxItems().filter { SyncValidation.isId($0.id) }
    }

    /// Turn an inbox item into a note, then remove it from the server. The note
    /// takes the item's id, so if removing it fails, the item stays and a second
    /// try finishes the job without making another note.
    func acceptInboxItem(_ item: InboxItem) async throws {
        guard let noteRepo, let key = keyManager.masterKey, let syncClient else { return }
        if try noteRepo.getRaw(id: item.id) == nil {
            let note = try noteRepo.create(id: item.id, content: item.content, tags: item.tags, fromInbox: true, key: key)
            changeLists { notes.insert(note, at: 0) }
            scheduleAutoSync()
        }
        try await syncClient.deleteInboxItem(id: item.id)
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

        refreshNoteFromStore(id: noteId)
    }

    // MARK: - Sync

    /// Refresh notes after a background (SSE-triggered) sync completes and
    /// stamp the sync timestamp. `SyncService`'s post-sync handler runs off
    /// the MainActor (it's an actor-isolated closure), so this
    /// MainActor-isolated method is the single hop point — avoids
    /// re-entering `MainActor.run` for the `await applySyncChanges()` call,
    /// which its synchronous closure can't contain. Incremental per
    /// `SyncChanges` — see `applySyncChanges(_:)`.
    private func handlePostSyncCompletion(_ changes: SyncChanges) async {
        await applySyncChanges(changes)
        lastSyncAt = Date()
        if changes.removedByServerIds.count >= Self.suspiciousRemovalCount {
            notesRemovedByServer = Array(Set(notesRemovedByServer).union(changes.removedByServerIds))
        }
        if changes.clockLooksWrong {
            syncError = L.syncErrorClockAhead
        }
    }

    /// How many notes in use one sync can remove before the user is asked. A
    /// note normally reaches the bin first, so even a big clear-out on another
    /// device doesn't get here.
    private static let suspiciousRemovalCount = 10

    /// Set up the sync client. Pass `apiKey` directly after registration
    /// to avoid Keychain retrieval timing issues; otherwise reads from Keychain.
    func setupSync(apiKey providedKey: String? = nil) {
        guard settings.syncEnabled else {
            Log.debug("[Sync] setupSync: settings.syncEnabled is false — skipping")
            return
        }
        guard let endpoint = settings.syncEndpoint else {
            Log.debug("[Sync] setupSync: no syncEndpoint — skipping")
            return
        }
        guard let noteRepo, let syncRepo, let versionRepo, let attachmentRepo, let savedSearchRepo else {
            Log.debug("[Sync] setupSync: repos not initialised — skipping")
            return
        }
        guard let key = keyManager.masterKey else {
            Log.debug("[Sync] setupSync: no masterKey — skipping")
            return
        }

        let apiKey = providedKey ?? KeychainService.retrieveAPIKey()
        guard let apiKey, !apiKey.isEmpty else {
            Log.debug("[Sync] setupSync: no API key in Keychain")
            syncError = L.syncErrorNoApiKey
            return
        }
        Log.debug("[Sync] setupSync: OK — endpoint=\(endpoint)")
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
            await service.setPostSyncHandler { [weak self] changes in
                await self?.handlePostSyncCompletion(changes)
            }
            await service.startSSE()
            await self?.triggerSync()
        }

        // Restart SSE immediately when the network recovers
        startNetworkMonitor()
    }

    /// Debounced background sync — coalesces rapid local edits into one
    /// sync cycle a few seconds after changes stop arriving. This runs at
    /// the state layer so it fires regardless of view lifecycle (e.g. when
    /// switching notes in the split view, where onDisappear never fires).
    func scheduleAutoSync(after delay: Duration = .seconds(3)) {
        guard syncEnabled else { return }
        autoSyncTask?.cancel()
        let pause = autoSyncPause
        autoSyncTask = Task { [weak self] in
            await pause(delay)
            guard !Task.isCancelled else { return }
            await self?.triggerSync()
        }
    }

    func triggerSync() async {
        guard let syncService else {
            Log.debug("[Sync] triggerSync: syncService is nil — aborting")
            return
        }
        guard !isSyncing else {
            Log.debug("[Sync] triggerSync: sync already in progress — skipping")
            return
        }
        Log.debug("[Sync] triggerSync: starting sync cycle")
        isSyncing = true
        syncError = nil
        syncStatusMessage = L.syncStatusSyncing

        // Captured before the single sync() call below — see the mapping
        // note there for why this still yields the correct "N new notes"
        // diff even though applySyncChanges no longer runs inline here.
        let previousCount = notes.count

        do {
            // Single entry point (jottery-dggw): route through the actor's
            // sync() instead of calling push()/pull()/finalise() directly.
            // sync() holds SyncService's own `isSyncing` guard, so a
            // concurrent SSE-triggered sync() (SyncService.startSSE()'s
            // onSyncEvent also calls sync()) now correctly serialises
            // against this UI-triggered cycle instead of racing it — the
            // previous code bypassed that guard by calling push()/pull()
            // directly, so the two paths could interleave across actor
            // suspension points.
            //
            // sync() already awaits the wired postSyncHandler
            // (AppState.setupSync() → handlePostSyncCompletion →
            // applySyncChanges + lastSyncAt) before returning, so by the
            // time we resume here `notes`/`archivedNotes` already reflect
            // this cycle's changes — this method must NOT call
            // applySyncChanges again itself, or every sync would decrypt
            // the same changed rows twice.
            let changes = try await syncService.sync()
            isSyncing = false
            guard !changes.wasSkipped else {
                // Nothing ran, so nothing can be said about it. One already
                // running may have missed what prompted this, so go again after.
                syncStatusMessage = nil
                if await syncService.isSyncing { scheduleAutoSync() }
                return
            }
            lastSyncAt = Date()
            syncRetryAttempts = 0

            // Pull any conflicts detected during push
            pendingConflicts = await syncService.pendingConflicts

            let diff = notes.count - previousCount
            if diff > 0 {
                syncStatusMessage = L.syncStatusNewNotes(diff)
            } else {
                syncStatusMessage = L.syncStatusUpToDate
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
            Log.debug("[Sync] triggerSync: ERROR — \(error)")
            syncError = SyncEndpoint.describeSyncFailure(error)
            syncStatusMessage = nil
            isSyncing = false
            if let delay = SyncRetry.delay(after: error, attempt: syncRetryAttempts) {
                syncRetryAttempts += 1
                scheduleAutoSync(after: .seconds(delay))
            }
        }
    }

    /// Force a full sync — re-downloads all notes from the server.
    func forceFullSync() async {
        guard let syncService else { return }
        isSyncing = true
        syncError = nil
        syncStatusMessage = L.syncStatusFullSyncRunning

        do {
            try await syncService.forceFullSync()
            // Single reload (jottery-vn6f): SyncService.forceFullSync() used
            // to *also* fire the wired postSyncHandler with `.fullReload`,
            // which drove its own loadNotes() via handlePostSyncCompletion →
            // applySyncChanges(.fullReload) — a second, redundant decrypt of
            // the whole vault on every force-resync. That postSyncHandler
            // call has been removed from SyncService.forceFullSync() (its
            // only caller is this method), leaving this the single reload.
            // Kept as `try` (not `try?`, unlike the removed handler-driven
            // path, which always swallowed errors) so a decrypt failure here
            // surfaces via `syncError` below instead of silently vanishing.
            try await loadNotes()
            isSyncing = false
            lastSyncAt = Date()
            pendingConflicts = await syncService.pendingConflicts
            syncStatusMessage = L.syncStatusFullSyncDone

            Task { [weak self] in try? await self?.loadInboxItems() }

            let msg = syncStatusMessage
            Task { @MainActor [weak self] in
                try? await Task.sleep(for: .seconds(4))
                if self?.syncStatusMessage == msg { self?.syncStatusMessage = nil }
            }
        } catch {
            syncError = SyncEndpoint.describeSyncFailure(error)
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

    /// Start the auto-lock clock, if it isn't already running.
    func noteEnteredBackground() {
        if backgroundedAt == nil {
            backgroundedAt = .now
        }
    }

    /// Lock if the app has been away longer than the auto-lock timeout
    /// (timeout 0 means "never"). Returns true if it did. Safe to call from
    /// more than one place on the way back: the first call decides.
    @discardableResult
    func lockIfAwayTooLong() -> Bool {
        guard !isLocked, let away = backgroundedAt, keyManager.autoLockTimeout > 0,
              away.duration(to: .now) >= .seconds(keyManager.autoLockTimeout) else {
            return false
        }
        lock()
        backgroundedAt = nil
        return true
    }

    func handleScenePhaseChange(_ phase: ScenePhase) {
        switch phase {
        case .background:
            // Genuinely backgrounded — stop SSE and record timestamp for auto-lock.
            noteEnteredBackground()
            Task { await syncService?.stopSSE() }
        case .inactive:
            // Transient inactivity (Face ID prompt, notification centre, share sheet).
            // Don't stop SSE or set backgroundedAt — avoid triggering a redundant
            // SSE restart and full sync when the app returns to .active.
            break
        case .active:
            // The database can't open while the device is locked (see
            // `DatabaseManager`), which a launch in the background can run into.
            if db == nil { Task { await initialise() } }
            // A reminder fires once, so coming back is when the next is set.
            Task { await refreshBackupReminder() }
            guard !isLocked, !lockIfAwayTooLong() else { return }
            let wasBackgrounded = backgroundedAt != nil
            backgroundedAt = nil
            keyManager.recordActivity()
            importSharedInboxItems()
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

    /// Notes wiping would lose for good: every note if sync is off, or those
    /// that haven't reached the server. Needs no key, so it works while locked.
    func notesOnlyOnThisDevice() -> Int {
        guard let noteRepo else { return 0 }
        return (try? settings.syncEnabled ? noteRepo.countUnsynced() : noteRepo.countActive()) ?? 0
    }

    /// Wipe from the lock screen. Anyone holding the phone can reach that, and
    /// wrong passwords are free, so they first have to show they own the device.
    /// Returns false if they couldn't, and nothing has changed.
    @discardableResult
    func wipeAfterProvingOwnership(
        authenticate: (String) async -> Bool = DeviceOwner.authenticate
    ) async -> Bool {
        guard await authenticate(L.unlockDeleteAuthReason) else { return false }
        wipeAllData()
        return true
    }

    /// Delete everything — database, Keychain, in-memory state — so the user
    /// can start fresh as if the app was just installed.
    func wipeAllData(sharedInboxRoot: URL? = nil) {
        // Clear Keychain
        KeychainService.deleteAPIKey()
        KeychainService.deleteClientId()
        KeychainService.deleteBiometricKey()

        teardownSync()
        removeTemporaryFiles()
        // Nothing here depends on it, so nothing waits for it.
        let reminders = backupReminders
        Task.detached { await reminders.cancel() }
        SharedInboxStore.removeAll(in: sharedInboxRoot)

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
        filenameCleanup?.cancel()
        filenameCleanup = nil
        attachmentRepo = nil
        savedSearchRepo = nil
        pendingEditorNote = nil
        lastPublishedTicket = nextLoadTicket()
        searchTask?.cancel()
        notes = []
        archivedNotes = []
        filteredNotes = []
        savedSearches = []
        noteColours = NoteColourSettings()
        pendingConflicts = []
        pendingAccountKeyMerge = nil
        accountKeyNotice = nil
        notesRemovedByServer = []
        inboxItems = []
        showArchive = false
        selectedNoteId = nil
        searchQuery = ""
        recentSearches.clear()
        presentedSheet = nil
        isSyncing = false
        lastSyncAt = nil
        syncError = nil
        syncStatusMessage = nil
        settings = .defaults
        keyManager.lock()
        isLocked = true
        isFirstLaunch = true

        // Re-initialise so the DB is recreated fresh
        isDatabaseReady = false
        Task { await initialise() }
    }

    // MARK: - Password Change

    /// Change the vault password. With envelope encryption, only re-wraps the master key (fast).
    /// For legacy vaults, re-encrypts all notes, attachments, versions, and the verification token.
    func changePassword(currentPassword: String, newPassword: String) async throws {
        guard PasswordPolicy.violation(newNotesPassword: newPassword) == nil else {
            throw AppStateError.passwordTooShort
        }
        guard let encryptionRepo else {
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
            let currentDeviceKey = try CryptoService.deriveKey(
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
                guard AppState.verifyWithToken(verificationJSON, key: masterKey) else {
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

            // Background: server re-wrap. A failure is kept and retried at the
            // next unlock or sync, so the user only needs telling.
            if settings.syncEnabled, let syncRepo {
                let dataForServer = masterKeyData
                let syncClient = self.syncClient
                Task {
                    let uploaded = await EnvelopeService.changePasswordServer(
                        syncRepo: syncRepo,
                        syncClient: syncClient,
                        newPassword: newPassword,
                        masterKeyData: dataForServer
                    )
                    if !uploaded { reportError(L.changePasswordServerKeyPending, duration: .seconds(8)) }
                }
            }
        } else {
            // LEGACY PATH: full re-encryption
            guard let saltData = metadata.saltData else {
                throw AppStateError.invalidSalt
            }

            // Verify current password
            let currentIterations = UInt32(metadata.iterations)
            let currentKey = try await Task.detached(priority: .userInitiated) {
                try CryptoService.deriveKey(password: currentPassword, salt: saltData, iterations: currentIterations)
            }.value
            if let verificationJSON = metadata.verification {
                guard AppState.verifyWithToken(verificationJSON, key: currentKey) else {
                    throw AppStateError.wrongPassword
                }
            } else {
                guard AppState.verifyByDecryptingNote(key: currentKey, db: db) else {
                    throw AppStateError.wrongPassword
                }
            }

            // Generate new salt and derive new key
            let newSalt = CryptoService.generateSalt()
            let iterations = UInt32(metadata.iterations)
            let newKey = try await Task.detached(priority: .userInitiated) {
                try CryptoService.deriveKey(password: newPassword, salt: newSalt, iterations: iterations)
            }.value

            // Re-encrypt everything under the new key and store the new salt
            // and verification token in one transaction. If anything cannot
            // be re-encrypted, nothing changes and the old password still works.
            guard let db else { throw AppStateError.notInitialised }
            let verificationJSON = try CryptoService.serializeEncryptedJSON(
                CryptoService.encryptText(EncryptionMetadata.verificationPlaintext, key: newKey)
            )
            var newMetadata = metadata
            newMetadata.salt = newSalt.base64EncodedString()
            newMetadata.verification = verificationJSON
            try await performReKey(to: newKey) { [newMetadata] in
                try VaultReKeyService.reKey(
                    database: db, from: currentKey, to: newKey, newMetadata: newMetadata
                )
            }

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

    // MARK: - Re-keying the Vault

    /// Move the whole vault onto `newKeyData` and make it this device's key,
    /// wrapped locally under `notesPassword`. All or nothing: if anything
    /// cannot be re-encrypted, nothing changes and this throws
    /// (`VaultReKeyService.ReKeyError`).
    ///
    /// Used to merge this device's notes into an account that already has a
    /// different key, and when a legacy vault moves onto the account's key.
    @discardableResult
    func switchVaultToMasterKey(
        _ newKeyData: Data,
        notesPassword: String
    ) async throws -> VaultReKeyService.Summary {
        guard let db, let encryptionRepo, let oldKey = keyManager.masterKey,
              let current = try encryptionRepo.get() else {
            throw AppStateError.notInitialised
        }
        let newKey = SymmetricKey(data: newKeyData)
        let newMetadata = try await Task.detached(priority: .userInitiated) {
            try EnvelopeService.envelopeMetadata(for: newKeyData, localPassword: notesPassword, current: current)
        }.value
        let summary = try await performReKey(to: newKey) {
            try VaultReKeyService.reKey(database: db, from: oldKey, to: newKey, newMetadata: newMetadata)
        }
        Log.debug("[ReKey] Vault moved to a new master key: \(summary)")
        return summary
    }

    /// Run `reKey` with sync stopped, then switch to `newKey`. A running sync
    /// service captured the old key, so anything it pushed or pulled after the
    /// re-key commits would be under the wrong key: pushing old ciphertext and
    /// clearing the "needs sync" flag the re-key just set, or writing pulled
    /// tags under the old key. Sync is restarted afterwards on the new key.
    @discardableResult
    private func performReKey(
        to newKey: SymmetricKey,
        _ reKey: @escaping @Sendable () throws -> VaultReKeyService.Summary
    ) async throws -> VaultReKeyService.Summary {
        let syncWasRunning = syncService != nil
        guard await stopSyncForKeyChange() else {
            if syncWasRunning { setupSync() }
            throw AppStateError.syncBusy
        }
        isReKeying = true
        let summary: VaultReKeyService.Summary
        do {
            // Every note and blob, so never on the main thread.
            summary = try await Task.detached(priority: .userInitiated) { [willReKey] in
                await willReKey?()
                return try reKey()
            }.value
        } catch {
            isReKeying = false
            if syncWasRunning { setupSync() }
            throw error
        }
        // Nothing can run between these two: saving reopens with the key in place.
        isReKeying = false
        await didChangeMasterKey(to: newKey, restartSync: syncWasRunning)
        return summary
    }

    /// How long a key change waits for a sync in progress.
    @ObservationIgnored var syncShutdownTimeout: Duration = .seconds(30)

    /// Runs just before re-encrypting. Tests hold a re-key here, as on a small
    /// vault it's over too quickly to do anything during it.
    @ObservationIgnored var willReKey: (@Sendable () async -> Void)?

    /// Detach the sync service so nothing new can start, and wait for a sync
    /// in progress to finish. False if one was still running at the deadline.
    private func stopSyncForKeyChange() async -> Bool {
        guard let service = syncService else { return true }
        syncService = nil
        return await service.shutDown(timeout: syncShutdownTimeout)
    }

    /// After the vault has been re-encrypted under `newKey`: use it from now
    /// on, refresh Face ID's copy, restart sync if it was running, and reload.
    ///
    /// Face ID stores the master key itself, so without the refresh the next
    /// Face ID unlock would load the old key and no note would decrypt.
    private func didChangeMasterKey(to newKey: SymmetricKey, restartSync: Bool) async {
        // It may have locked while the vault was being re-encrypted. Unlocking
        // finds the new key, so there's nothing to load, and Face ID's copy is
        // refreshed by the next password unlock.
        guard !isLocked else {
            biometricKeyNeedsRefresh = keyManager.isBiometricEnabled
            return
        }
        keyManager.masterKey = newKey
        if keyManager.isBiometricEnabled {
            try? keyManager.enableBiometricUnlock()
        }
        // An edit that was held back while this ran.
        flushPendingEditorNote()
        if restartSync {
            setupSync()
        }
        do {
            try await loadNotes()
        } catch {
            // The re-key itself succeeded; the next refresh will catch up.
            Log.debug("[ReKey] Reload after key change failed: \(error)")
        }
    }

    // MARK: - Settings

    /// The list's sort order, kept for next time.
    func setSortOrder(_ order: SortOrder) {
        var updated = settings
        updated.sort = order
        do {
            try updateSettings(updated)
        } catch {
            Log.debug("[Settings] couldn't save the sort order: \(error)")
            sortOrder = order
        }
    }

    /// Whether to offer Face ID after a password unlock. Asked once: someone who
    /// says later can still turn it on in Settings.
    static func shouldOfferFaceID(isAvailable: Bool, isEnabled: Bool, wasDeclined: Bool) -> Bool {
        isAvailable && !isEnabled && !wasDeclined
    }

    static let faceIDDeclinedKey = "faceIDOfferDeclined"

    func offerFaceIDIfUseful() {
        offersFaceID = Self.shouldOfferFaceID(
            isAvailable: keyManager.isBiometricAvailable,
            isEnabled: keyManager.isBiometricEnabled,
            wasDeclined: UserDefaults.standard.bool(forKey: Self.faceIDDeclinedKey)
        )
    }

    func declineFaceID() {
        UserDefaults.standard.set(true, forKey: Self.faceIDDeclinedKey)
    }

    func updateSettings(_ newSettings: UserSettings) throws {
        guard let settingsRepo else { return }
        settings = newSettings
        try settingsRepo.save(newSettings)
        sortOrder = newSettings.sort
        keyManager.autoLockTimeout = TimeInterval(newSettings.autoLockTimeout * 60)
    }
}

/// What `MainView` tells the user about the account's key.
enum AccountKeyNotice: Equatable {
    /// The server's copy of the key is wrapped with a password this device
    /// doesn't have, so the notes password was changed on another device.
    case passwordChangedElsewhere
    /// This vault's existing password matches the account password, so the
    /// server could open the account's key.
    case passwordMatchesAccount
}

/// The account's key, opened with the notes password, and that password,
/// held only while the user is asked whether to move the vault onto the key.
struct PendingAccountKeyMerge {
    let accountKey: Data
    let notesPassword: String
}

enum AppStateError: LocalizedError {
    case notInitialised
    case noVault
    case invalidSalt
    case wrongPassword
    /// A sync was still running when the vault's key was about to change.
    case syncBusy
    /// The vault is being re-encrypted, so nothing can be saved for the moment.
    case vaultBusy
    /// A new notes password is shorter than `PasswordPolicy` allows.
    case passwordTooShort

    var errorDescription: String? {
        switch self {
        case .notInitialised: return L.appErrorNotInitialised
        case .noVault: return L.appErrorNoVault
        case .invalidSalt: return L.appErrorInvalidSalt
        case .wrongPassword: return L.appErrorWrongPassword
        case .syncBusy: return L.appErrorSyncBusy
        case .vaultBusy: return L.vaultBusy
        case .passwordTooShort: return L.notesPasswordTooShort(PasswordPolicy.minimumNotesPasswordLength)
        }
    }
}
