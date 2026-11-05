package com.jottery.android.viewmodel

import android.app.Application
import android.util.Log
import com.jottery.android.R
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import androidx.room.withTransaction
import com.jottery.android.crypto.CryptoService
import com.jottery.android.crypto.KeyManager
import com.jottery.android.crypto.KeystoreService
import com.jottery.android.database.JotteryDatabase
import com.jottery.android.database.repository.AttachmentRepository
import com.jottery.android.database.repository.EncryptionRepository
import com.jottery.android.database.repository.NoteRepository
import com.jottery.android.database.repository.SavedSearchRepository
import com.jottery.android.database.repository.SettingsRepository
import com.jottery.android.database.repository.SyncRepository
import com.jottery.android.database.repository.VersionRepository
import com.jottery.android.model.AttachmentRecord
import com.jottery.android.model.AttachmentRef
import com.jottery.android.model.ConflictInfo
import com.jottery.android.model.ConflictResolutionStrategy
import com.jottery.android.model.DecryptedNote
import com.jottery.android.model.DecryptedSavedSearch
import com.jottery.android.model.EncryptionMetadata
import com.jottery.android.model.InboxItem
import com.jottery.android.model.NoteVersionRecord
import com.jottery.android.model.CloneDeviceRequest
import com.jottery.android.model.CredentialPayload
import com.jottery.android.model.LegacyCredentialPayload
import com.jottery.android.model.RegisterDeviceRequest
import com.jottery.android.model.SortOrder
import com.jottery.android.model.UserSettings
import com.jottery.android.network.SSEClient
import com.jottery.android.network.SyncClient
import com.jottery.android.network.SyncClientException
import com.jottery.android.crypto.EncryptedData
import com.jottery.android.service.backup.BackupService
import com.jottery.android.service.backup.BackupSource
import com.jottery.android.service.backup.RestorePlan
import com.jottery.android.service.backup.RestoreResult
import com.jottery.android.service.backup.keyBytes
import com.jottery.android.service.EnvelopeException
import com.jottery.android.service.EnvelopeService
import com.jottery.android.service.VaultReKeyService
import com.jottery.android.service.ExportService
import com.jottery.android.service.ImportService
import com.jottery.android.service.ImportStrategy
import com.jottery.android.service.SearchService
import com.jottery.android.service.SyncValidator
import com.jottery.android.service.SyncService
import javax.crypto.spec.SecretKeySpec
import com.jottery.android.util.AttachmentFiles
import com.jottery.android.util.DateUtils
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import java.time.Instant
import java.util.concurrent.atomic.AtomicLong
import javax.crypto.SecretKey

/**
 * Root application state -- drives the entire UI.
 * Android port of iOS AppState.swift using AndroidViewModel + StateFlow.
 */
class AppState(application: Application) : AndroidViewModel(application) {

    // MARK: - Lock State

    private val _isInitialised = MutableStateFlow(false)
    val isInitialised: StateFlow<Boolean> = _isInitialised.asStateFlow()

    private val _isFirstLaunch = MutableStateFlow(true)
    val isFirstLaunch: StateFlow<Boolean> = _isFirstLaunch.asStateFlow()

    private val _isLocked = MutableStateFlow(true)
    val isLocked: StateFlow<Boolean> = _isLocked.asStateFlow()

    // MARK: - Notes

    private val _notes = MutableStateFlow<List<DecryptedNote>>(emptyList())
    val notes: StateFlow<List<DecryptedNote>> = _notes.asStateFlow()

    private val _archivedNotes = MutableStateFlow<List<DecryptedNote>>(emptyList())
    val archivedNotes: StateFlow<List<DecryptedNote>> = _archivedNotes.asStateFlow()

    private val _showArchive = MutableStateFlow(false)
    val showArchive: StateFlow<Boolean> = _showArchive.asStateFlow()

    private val _selectedNoteId = MutableStateFlow<String?>(null)
    val selectedNoteId: StateFlow<String?> = _selectedNoteId.asStateFlow()

    private val _searchQuery = MutableStateFlow("")
    val searchQuery: StateFlow<String> = _searchQuery.asStateFlow()

    private val _sortOrder = MutableStateFlow(SortOrder.RECENT)
    val sortOrder: StateFlow<SortOrder> = _sortOrder.asStateFlow()

    // MARK: - Sync

    private val _isSyncing = MutableStateFlow(false)
    val isSyncing: StateFlow<Boolean> = _isSyncing.asStateFlow()

    private val _lastSyncAt = MutableStateFlow<Instant?>(null)
    val lastSyncAt: StateFlow<Instant?> = _lastSyncAt.asStateFlow()

    private val _syncError = MutableStateFlow<String?>(null)
    val syncError: StateFlow<String?> = _syncError.asStateFlow()

    private val _syncEnabled = MutableStateFlow(false)
    val syncEnabled: StateFlow<Boolean> = _syncEnabled.asStateFlow()

    private val _syncStatusMessage = MutableStateFlow<String?>(null)
    val syncStatusMessage: StateFlow<String?> = _syncStatusMessage.asStateFlow()

    // MARK: - Search (debounced)

    private val _filteredNotes = MutableStateFlow<List<DecryptedNote>>(emptyList())
    val filteredNotes: StateFlow<List<DecryptedNote>> = _filteredNotes.asStateFlow()

    // MARK: - Saved Searches

    private val _savedSearches = MutableStateFlow<List<DecryptedSavedSearch>>(emptyList())
    val savedSearches: StateFlow<List<DecryptedSavedSearch>> = _savedSearches.asStateFlow()

    // MARK: - Conflicts

    private val _pendingConflicts = MutableStateFlow<List<ConflictInfo>>(emptyList())
    val pendingConflicts: StateFlow<List<ConflictInfo>> = _pendingConflicts.asStateFlow()

    // MARK: - Inbox

    private val _inboxItems = MutableStateFlow<List<InboxItem>>(emptyList())
    val inboxItems: StateFlow<List<InboxItem>> = _inboxItems.asStateFlow()

    // MARK: - Editor Font

    private val _editorFontScale = MutableStateFlow(
        application.getSharedPreferences(PREFS_NAME, 0)
            .getFloat(KEY_EDITOR_FONT_SCALE, 1.0f)
    )
    val editorFontScale: StateFlow<Float> = _editorFontScale.asStateFlow()

    // MARK: - Screen Security

    private val _screenSecurity = MutableStateFlow(
        application.getSharedPreferences(PREFS_NAME, 0).getBoolean(KEY_SCREEN_SECURITY, true)
    )
    /** Keep notes out of screenshots, screen recording and the recents thumbnail. */
    val screenSecurity: StateFlow<Boolean> = _screenSecurity.asStateFlow()

    fun setScreenSecurity(enabled: Boolean) {
        _screenSecurity.value = enabled
        getApplication<Application>()
            .getSharedPreferences(PREFS_NAME, 0)
            .edit()
            .putBoolean(KEY_SCREEN_SECURITY, enabled)
            .apply()
    }

    // MARK: - Settings

    private val _settings = MutableStateFlow(UserSettings.defaults)
    val settings: StateFlow<UserSettings> = _settings.asStateFlow()

    // MARK: - Pending Editor Note

    private val _pendingEditorNote = MutableStateFlow<DecryptedNote?>(null)
    val pendingEditorNote: StateFlow<DecryptedNote?> = _pendingEditorNote.asStateFlow()

    // MARK: - User Messages

    /** A one-off message for the snackbar on the main screen. */
    private val _userMessage = MutableStateFlow<String?>(null)
    val userMessage: StateFlow<String?> = _userMessage.asStateFlow()

    fun showMessage(message: String) {
        _userMessage.value = message
    }

    fun clearMessage() {
        _userMessage.value = null
    }

    // MARK: - Internal

    private var db: JotteryDatabase? = null
    val keyManager = KeyManager(scope = viewModelScope)

    private var noteRepo: NoteRepository? = null
    private var encryptionRepo: EncryptionRepository? = null
    private var settingsRepo: SettingsRepository? = null
    private var syncRepo: SyncRepository? = null
    private var versionRepo: VersionRepository? = null
    private var attachmentRepo: AttachmentRepository? = null
    private var savedSearchRepo: SavedSearchRepository? = null

    private var syncClientInstance: SyncClient? = null
    /** Expose sync client for EnvelopeService. */
    val syncClient: SyncClient? get() = syncClientInstance
    private var syncServiceInstance: SyncService? = null
    private var sseClientInstance: SSEClient? = null

    private var searchJob: Job? = null
    private var syncDebounceJob: Job? = null
    private var isBackgrounded = false

    /** Bumped by every local change to the note lists, so a slow load can see it. */
    private val notesRevision = AtomicLong()
    private val loadTicket = AtomicLong()

    /** One sync at a time; a request during a sync runs once more afterwards. */
    private val syncMutex = Mutex()
    @Volatile private var syncRequested = false

    private val json = Json { ignoreUnknownKeys = true }

    /** Notes displayed in the list -- either active or archived depending on mode. */
    val displayedNotes: List<DecryptedNote>
        get() = if (_showArchive.value) _archivedNotes.value else _notes.value

    val noteCount: Int
        get() = displayedNotes.size

    val filteredCount: Int
        get() = _filteredNotes.value.size

    val inboxCount: Int
        get() = _inboxItems.value.size

    val selectedNote: DecryptedNote?
        get() {
            val id = _selectedNoteId.value ?: return null
            return _notes.value.firstOrNull { it.id == id }
        }

    /** All unique tags across all notes, sorted alphabetically. */
    val allTags: List<String>
        get() {
            val tagSets = _notes.value.flatMap { it.tags } + _archivedNotes.value.flatMap { it.tags }
            return tagSets.toSet().sorted()
        }

    // MARK: - Initialisation

    init {
        initialise()
    }

    /**
     * Create the database, repositories, and check if a vault exists.
     * Safe to call multiple times -- subsequent calls are no-ops.
     */
    fun initialise() {
        if (db != null) return  // Already initialised

        val context = getApplication<Application>()

        // Anything still decrypted for another app from the last session goes now
        AttachmentFiles.clear(context)

        // Wire auto-lock so the timer triggers a full app lock (UI + key wipe)
        keyManager.onAutoLock = {
            lock()
        }

        try {
            val database = JotteryDatabase.getInstance(context)
            this.db = database
            val verRepo = VersionRepository(database.noteVersionDao())
            this.versionRepo = verRepo
            this.noteRepo = NoteRepository(database.noteDao(), database.noteVersionDao()) { block ->
                database.withTransaction { block() }
            }
            this.encryptionRepo = EncryptionRepository(database.encryptionMetadataDao())
            this.settingsRepo = SettingsRepository(database.settingsDao())
            this.syncRepo = SyncRepository(
                database.syncMetadataDao(),
                database.noteSyncMetadataDao(),
                database.noteDeletionDao(),
            )
            this.attachmentRepo = AttachmentRepository(database.attachmentDao())
            this.savedSearchRepo = SavedSearchRepository(database.savedSearchDao())

            // Check vault and load settings in a coroutine (Room DAOs are suspend)
            viewModelScope.launch {
                try {
                    val metadata = encryptionRepo?.get()
                    _isFirstLaunch.value = metadata == null

                    val loaded = settingsRepo?.get()
                    if (loaded != null) {
                        _settings.value = loaded
                        _sortOrder.value = loaded.sort
                        keyManager.autoLockTimeout = loaded.autoLockTimeout.toLong() * 60L
                    }
                } catch (e: Exception) {
                    Log.e(TAG, "Initialisation error: ${e.message}")
                    _isFirstLaunch.value = true
                } finally {
                    _isInitialised.value = true
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Database init failed: ${e.message}")
            _isFirstLaunch.value = true
            _isInitialised.value = true
        }
    }

    // MARK: - Vault Setup

    /**
     * Create a new vault with the given password.
     * When importing from another device, pass the existing salt and iterations
     * so the same password derives the same encryption key.
     */
    suspend fun createVault(
        password: String,
        existingSalt: ByteArray? = null,
        existingIterations: Int? = null,
    ) {
        val repo = encryptionRepo ?: throw AppStateError.NotInitialised

        val salt = existingSalt ?: CryptoService.generateSalt()
        val iterations = existingIterations ?: CryptoService.DEFAULT_ITERATIONS

        // Derive key first so we can create the verification token
        val key = withContext(Dispatchers.Default) { keyManager.unlock(password, salt, iterations) }

        // Encrypt a known plaintext to verify the password later
        val verificationEncrypted = CryptoService.encryptText(
            EncryptionMetadata.VERIFICATION_PLAINTEXT, key
        )
        val verificationJSON = CryptoService.serializeEncryptedJSON(verificationEncrypted)

        val metadata = EncryptionMetadata.new(
            salt = salt,
            iterations = iterations,
            verification = verificationJSON,
        )
        repo.store(metadata)

        _isFirstLaunch.value = false
        _isLocked.value = false

        // Load notes (should be empty for new vault)
        loadNotes()
    }

    // MARK: - Unlock

    /**
     * Attempt to unlock with the given password.
     * Returns true if successful.
     */
    suspend fun unlock(password: String): Boolean {
        val repo = encryptionRepo ?: throw AppStateError.NotInitialised

        val metadata = repo.get() ?: throw AppStateError.NoVault

        val key: javax.crypto.SecretKey

        if (metadata.envelopeVersion != null) {
            // Envelope unlock path — derive device key, unwrap master key
            val deviceSaltB64 = metadata.deviceSalt ?: throw AppStateError.InvalidSalt
            val deviceSalt = android.util.Base64.decode(deviceSaltB64, android.util.Base64.NO_WRAP)
            val wrappedJSON = metadata.localWrappedMaster ?: throw AppStateError.InvalidSalt

            val deviceKey = withContext(Dispatchers.Default) {
                CryptoService.deriveKey(password, deviceSalt, CryptoService.DEFAULT_ITERATIONS)
            }
            val wrapped = CryptoService.parseEncryptedJSON(wrappedJSON)
            val masterKeyData: ByteArray
            try {
                masterKeyData = CryptoService.unwrapMasterKey(wrapped, deviceKey)
            } catch (_: Exception) {
                keyManager.lock()
                _isLocked.value = true
                throw AppStateError.WrongPassword
            }

            key = SecretKeySpec(masterKeyData, "AES")
            keyManager.unlockWithKeyData(masterKeyData)

            // Verify with token if available
            if (metadata.verification != null) {
                if (!verifyWithToken(metadata.verification, key)) {
                    keyManager.lock()
                    _isLocked.value = true
                    throw AppStateError.WrongPassword
                }
            }
        } else {
            // Legacy unlock path — derive key directly from password + salt
            val saltData = metadata.saltData ?: throw AppStateError.InvalidSalt

            key = withContext(Dispatchers.Default) {
                keyManager.unlock(
                    password = password,
                    salt = saltData,
                    iterations = metadata.iterations ?: CryptoService.DEFAULT_ITERATIONS,
                )
            }

            // Verify password
            val verified = if (metadata.verification != null) {
                verifyWithToken(metadata.verification, key)
            } else {
                verifyByDecryptingNote(key)
            }

            if (!verified) {
                keyManager.lock()
                _isLocked.value = true
                throw AppStateError.WrongPassword
            }
        }

        loadNotes()

        // Auto-purge deleted notes older than 30 days
        try {
            val purged = noteRepo?.purgeOldDeletedNotes() ?: 0
            if (purged > 0) {
                Log.i(TAG, "[Purge] Removed $purged deleted note(s) older than 30 days")
            }
        } catch (_: Exception) { /* ignore */ }

        Log.d(TAG, "[Sync] unlock: calling setupSync()")
        setupSync()

        // Upgrade old vaults: store a verification token if missing
        if (metadata.verification == null) {
            try {
                upgradeVaultVerification(key)
            } catch (_: Exception) { /* ignore */ }
        }

        // Encrypt any filenames an older build left in plaintext
        viewModelScope.launch {
            try {
                val repaired = noteRepo?.repairPlaintextFilenames(key) ?: 0
                if (repaired > 0) {
                    Log.i(TAG, "[Repair] Encrypted filenames on $repaired note(s)")
                    scheduleDebouncedSync()
                }
            } catch (e: Exception) {
                Log.w(TAG, "[Repair] Filename repair failed: ${e.javaClass.simpleName}")
            }
        }

        // Keep the account's wrapped key in step, in the background
        if (_settings.value.syncEnabled) {
            viewModelScope.launch { reconcileAccountKey(password) }
        }

        // Last: this leaves the unlock screen, which cancels the caller's scope
        _isLocked.value = false

        return true
    }

    /**
     * Verify the key by decrypting the stored verification token.
     */
    private fun verifyWithToken(json: String, key: SecretKey): Boolean {
        return try {
            val encrypted = CryptoService.parseEncryptedJSON(json)
            val plaintext = CryptoService.decryptText(encrypted, key)
            plaintext == EncryptionMetadata.VERIFICATION_PLAINTEXT
        } catch (_: Exception) {
            false
        }
    }

    /**
     * Verify the key by decrypting one note from the database.
     * Returns true if decryption succeeds, or if there are no notes to test against.
     */
    private suspend fun verifyByDecryptingNote(key: SecretKey): Boolean {
        val repo = noteRepo ?: return false
        return try {
            val record = repo.getRaw(
                // Try to get any non-deleted note to verify against
                id = "" // We need a different approach -- check if any active notes exist
            )
            // If getRaw returns null for empty id, try listing active notes
            val activeNotes = repo.listActive(key)
            // If listing succeeded without exception, the key is valid
            true
        } catch (_: Exception) {
            // Try counting active notes -- if there are none, empty vault is safe
            try {
                val count = repo.countActive()
                count == 0 // If no notes, can't verify but safe to enter
            } catch (_: Exception) {
                false
            }
        }
    }

    /**
     * Store a verification token on an old vault that doesn't have one.
     */
    private suspend fun upgradeVaultVerification(key: SecretKey) {
        val repo = encryptionRepo ?: return
        val metadata = repo.get() ?: return
        val encrypted = CryptoService.encryptText(
            EncryptionMetadata.VERIFICATION_PLAINTEXT, key
        )
        val updatedMetadata = metadata.copy(
            verification = CryptoService.serializeEncryptedJSON(encrypted)
        )
        repo.store(updatedMetadata)
    }

    // MARK: - Lock

    /**
     * Lock the application -- flush pending editor, wipe key, clear in-memory data.
     */
    fun lock() {
        Log.d(TAG, "[Sync] lock: clearing sync state, syncEnabled=false")

        // Flush any pending editor save while the key is still available
        val pending = _pendingEditorNote.value
        val key = keyManager.masterKey
        if (pending != null && key != null) {
            viewModelScope.launch {
                try {
                    noteRepo?.update(pending, key)
                } catch (_: Exception) { /* ignore */ }
            }
            _pendingEditorNote.value = null
        }

        // Stop SSE and release sync objects
        sseClientInstance?.stop()
        syncClientInstance = null
        syncServiceInstance = null
        sseClientInstance = null
        _syncEnabled.value = false

        AttachmentFiles.clear(getApplication())
        keyManager.lock()
        _isLocked.value = true
        _notes.value = emptyList()
        _archivedNotes.value = emptyList()
        _filteredNotes.value = emptyList()
        _pendingConflicts.value = emptyList()
        _inboxItems.value = emptyList()
        _selectedNoteId.value = null
    }

    // MARK: - Notes CRUD

    /**
     * Load all active, archived, and saved search data from the database.
     */
    suspend fun loadNotes() {
        val repo = noteRepo ?: return
        val key = keyManager.masterKey ?: return
        val ticket = loadTicket.incrementAndGet()

        // Anything saved during the read must not be overwritten by it, so a read
        // that a local write overtook starts again.
        repeat(MAX_LOAD_ATTEMPTS) {
            val revision = notesRevision.get()
            // A sync usually changes a few notes: the rest needn't be decrypted again
            val known = (_notes.value + _archivedNotes.value).associateBy { it.id }
            val loaded = withContext(Dispatchers.Default) {
                LoadedNotes(
                    active = repo.listActiveWithStats(key, known),
                    archived = repo.listArchived(key, known),
                    searches = savedSearchRepo?.listActive(key) ?: emptyList(),
                )
            }
            // A newer load owns the result, and a locked vault shows nothing
            if (ticket != loadTicket.get() || keyManager.masterKey !== key) return
            if (revision == notesRevision.get()) {
                publish(loaded)
                return
            }
        }
    }

    private class LoadedNotes(
        val active: NoteRepository.ListResult,
        val archived: List<DecryptedNote>,
        val searches: List<DecryptedSavedSearch>,
    )

    private fun publish(loaded: LoadedNotes) {
        val result = loaded.active
        _notes.value = result.notes
        _archivedNotes.value = loaded.archived
        _savedSearches.value = loaded.searches

        Log.d(TAG, "[loadNotes] ${result.notes.size} decrypted, " +
            "${result.failedCount} failed, ${result.totalRecords} total in DB")

        // Surface decryption failures so the user knows something is wrong
        if (result.failedCount > 0 && result.notes.isEmpty()) {
            _syncError.value = "${result.failedCount} notes in database could not be decrypted. " +
                "This usually means the encryption key differs from the device that created them. " +
                "Try wiping and re-importing credentials (including the encryption salt) from your other device."
        } else if (result.failedCount > 0) {
            _syncError.value = "${result.failedCount} of ${result.totalRecords} notes failed to decrypt"
        }

        scheduleSearch()
    }

    /**
     * Create a new note with the given content and tags.
     * Returns the created DecryptedNote, or null on failure.
     */
    suspend fun createNote(content: String = "", tags: List<String> = emptyList()): DecryptedNote? {
        val repo = noteRepo ?: return null
        val key = keyManager.masterKey ?: return null
        val note = repo.create(content = content, tags = tags, key = key)
        notesRevision.incrementAndGet()
        _notes.value = listOf(note) + _notes.value
        _selectedNoteId.value = note.id
        scheduleSearch()
        scheduleDebouncedSync()
        return note
    }

    /**
     * Save (update) a decrypted note back to the database.
     */
    suspend fun saveNote(note: DecryptedNote) {
        val repo = noteRepo ?: return
        val key = keyManager.masterKey ?: return
        if (!repo.update(note, key)) return
        refreshNote(note.id)
        scheduleDebouncedSync()
    }

    /**
     * Republish one note from the database. Flags and timestamps always come
     * from the row, never from a copy of an earlier in-memory note.
     */
    private suspend fun refreshNote(id: String) {
        val repo = noteRepo ?: return
        val key = keyManager.masterKey ?: return
        notesRevision.incrementAndGet()
        val fresh = repo.get(id = id, key = key) ?: return
        _notes.value = _notes.value.map { if (it.id == id) fresh else it }
        _archivedNotes.value = _archivedNotes.value.map { if (it.id == id) fresh else it }
        scheduleSearch()
    }

    /**
     * Duplicate a note by ID.
     */
    suspend fun duplicateNote(id: String) {
        val repo = noteRepo ?: return
        val key = keyManager.masterKey ?: return
        val original = repo.get(id = id, key = key) ?: return
        val duplicate = repo.create(content = original.content, tags = original.tags, key = key)
        notesRevision.incrementAndGet()
        _notes.value = listOf(duplicate) + _notes.value
        _selectedNoteId.value = duplicate.id
        scheduleSearch()
        scheduleDebouncedSync()
    }

    /**
     * Soft-delete a note by ID.
     */
    suspend fun deleteNote(id: String) {
        val repo = noteRepo ?: return
        repo.softDelete(id)
        notesRevision.incrementAndGet()
        _notes.value = _notes.value.filter { it.id != id }
        _archivedNotes.value = _archivedNotes.value.filter { it.id != id }
        if (_selectedNoteId.value == id) {
            _selectedNoteId.value = null
        }
        scheduleSearch()
        scheduleDebouncedSync()
    }

    /**
     * Restore a soft-deleted note.
     */
    suspend fun restoreNote(id: String) {
        val repo = noteRepo ?: return
        repo.restore(id)
        loadNotes()
        scheduleDebouncedSync()
    }

    /**
     * Toggle pin status on a note.
     */
    suspend fun togglePin(id: String) {
        val repo = noteRepo ?: return
        repo.togglePin(id)
        refreshNote(id)
        scheduleDebouncedSync()
    }

    /**
     * Archive a note by ID.
     */
    suspend fun archiveNote(id: String) {
        val repo = noteRepo ?: return
        repo.archive(id)
        notesRevision.incrementAndGet()
        _notes.value = _notes.value.filter { it.id != id }
        if (_selectedNoteId.value == id) {
            _selectedNoteId.value = null
        }
        scheduleSearch()
        loadArchivedNotes()
        scheduleDebouncedSync()
    }

    /**
     * Unarchive a note by ID.
     */
    suspend fun unarchiveNote(id: String) {
        val repo = noteRepo ?: return
        repo.unarchive(id)
        notesRevision.incrementAndGet()
        _archivedNotes.value = _archivedNotes.value.filter { it.id != id }
        if (_selectedNoteId.value == id) {
            _selectedNoteId.value = null
        }
        loadNotes()
        scheduleDebouncedSync()
    }

    /**
     * Toggle lock status on a note.
     */
    suspend fun toggleLock(id: String) {
        val repo = noteRepo ?: return
        repo.toggleLock(id)
        refreshNote(id)
        scheduleDebouncedSync()
    }

    /**
     * Reload archived notes from the database.
     */
    suspend fun loadArchivedNotes() {
        val repo = noteRepo ?: return
        val key = keyManager.masterKey ?: return
        _archivedNotes.value = repo.listArchived(key)
    }

    // MARK: - Search (debounced)

    /**
     * Call whenever searchQuery, notes, archivedNotes, showArchive, or sortOrder
     * changes to recompute filteredNotes off the main thread.
     * Debounces by 150 ms when a search query is active; immediate otherwise.
     */
    fun scheduleSearch() {
        searchJob?.cancel()
        val query = _searchQuery.value
        val source = if (SearchService.searchesArchive(query)) {
            _notes.value + _archivedNotes.value
        } else {
            displayedNotes
        }
        val order = _sortOrder.value

        // No debounce needed when query is empty -- just sort (cheap)
        if (query.isBlank()) {
            _filteredNotes.value = SearchService.sort(source, order)
            return
        }

        searchJob = viewModelScope.launch {
            // Debounce while typing
            delay(150)

            // Run expensive filter + sort
            _filteredNotes.value = withContext(Dispatchers.Default) {
                SearchService.sort(SearchService.filter(source, query).notes, order)
            }
        }
    }

    /**
     * Schedule a debounced sync push after a local note change.
     * Waits 3 seconds after the last call to avoid syncing on every keystroke.
     */
    private fun scheduleDebouncedSync() {
        if (!_syncEnabled.value || syncServiceInstance == null) return
        syncDebounceJob?.cancel()
        syncDebounceJob = viewModelScope.launch {
            delay(3_000)
            triggerSync()
        }
    }

    // MARK: - State Setters

    fun setSearchQuery(query: String) {
        _searchQuery.value = query
        scheduleSearch()
    }

    fun setSortOrder(order: SortOrder) {
        _sortOrder.value = order
        scheduleSearch()
    }

    fun setShowArchive(show: Boolean) {
        _showArchive.value = show
        scheduleSearch()
    }

    /** Toggle between notes and archive view. */
    fun toggleArchiveView() {
        _showArchive.value = !_showArchive.value
        _selectedNoteId.value = null
        scheduleSearch()
    }

    fun setSelectedNoteId(id: String?) {
        _selectedNoteId.value = id
    }

    /** Record user activity to reset the auto-lock countdown. */
    fun recordActivity() {
        keyManager.recordActivity()
    }

    /** Non-flow accessor for sync-enabled check (used by dispose effects). */
    val syncEnabledValue: Boolean get() = _syncEnabled.value

    /**
     * Save a note and trigger sync from viewModelScope.
     * Use this from DisposableEffect.onDispose where the composable scope
     * may already be cancelled.
     */
    fun saveNoteAndSync(note: DecryptedNote) {
        viewModelScope.launch {
            saveNote(note)
            if (_syncEnabled.value) {
                triggerSync()
            }
        }
    }

    /**
     * Trigger sync from viewModelScope (fire-and-forget).
     * Use this from DisposableEffect.onDispose.
     */
    fun triggerSyncBackground() {
        if (!_syncEnabled.value) return
        viewModelScope.launch { triggerSync() }
    }

    fun setPendingEditorNote(note: DecryptedNote?) {
        _pendingEditorNote.value = note
    }

    fun setEditorFontScale(scale: Float) {
        _editorFontScale.value = scale
        getApplication<Application>()
            .getSharedPreferences(PREFS_NAME, 0)
            .edit()
            .putFloat(KEY_EDITOR_FONT_SCALE, scale)
            .apply()
    }

    /**
     * Load deleted notes for the recycle bin.
     */
    suspend fun loadDeletedNotes(): List<DecryptedNote> {
        val repo = noteRepo ?: return emptyList()
        val key = keyManager.masterKey ?: return emptyList()
        return repo.listDeleted(key)
    }

    /**
     * Permanently delete a note (hard delete, no recovery).
     */
    suspend fun permanentlyDeleteNote(id: String) {
        val repo = noteRepo ?: return
        repo.hardDelete(id)
    }

    // MARK: - Saved Searches

    /**
     * Save a new search query with a name.
     */
    suspend fun saveSearch(name: String, query: String) {
        val repo = savedSearchRepo ?: return
        val key = keyManager.masterKey ?: return
        val search = repo.create(name = name, query = query, key = key)
        _savedSearches.value = _savedSearches.value + search
        scheduleDebouncedSync()
    }

    /**
     * Delete a saved search by ID.
     */
    suspend fun deleteSavedSearch(id: String) {
        val repo = savedSearchRepo ?: return
        repo.delete(id)
        _savedSearches.value = _savedSearches.value.filter { it.id != id }
        scheduleDebouncedSync()
    }

    /**
     * Apply a saved search: set the search query to the saved search's query.
     */
    fun applySavedSearch(id: String) {
        val search = _savedSearches.value.firstOrNull { it.id == id } ?: return
        setSearchQuery(search.query)
    }

    // MARK: - Attachments

    /**
     * Add an attachment to a note. Returns its id, or null if nothing was added.
     * The name is stored only as ciphertext, in the note's reference.
     */
    suspend fun addAttachment(
        noteId: String,
        data: ByteArray,
        filename: String,
        mimeType: String,
    ): String? {
        val noteRepository = noteRepo ?: return null
        val attachmentRepository = attachmentRepo ?: return null
        val key = keyManager.masterKey ?: return null

        val attachmentId = CryptoService.generateUUID()
        val blobData = withContext(Dispatchers.Default) {
            CryptoService.serializeEncryptedJSON(CryptoService.encrypt(data, key)).toByteArray(Charsets.UTF_8)
        }
        attachmentRepository.insertOrReplace(
            AttachmentRecord(id = attachmentId, filename = "", mimeType = mimeType, size = data.size, data = blobData)
        )

        // One ID for both metadata and blob
        val ref = AttachmentRef(
            id = attachmentId,
            filename = CryptoService.serializeEncryptedJSON(CryptoService.encryptText(filename, key)),
            mimeType = mimeType,
            size = data.size,
            data = attachmentId,
        )
        noteRepository.addAttachment(noteId = noteId, ref = ref)

        refreshNote(noteId)
        scheduleDebouncedSync()
        return attachmentId
    }

    /** An attachment's decrypted bytes, or null if it can't be read. */
    suspend fun attachmentData(ref: AttachmentRef): ByteArray? {
        val attachmentRepository = attachmentRepo ?: return null
        val key = keyManager.masterKey ?: return null
        return withContext(Dispatchers.Default) {
            try {
                val blob = attachmentRepository.getBlob(ref.data) ?: return@withContext null
                CryptoService.decrypt(CryptoService.parseEncryptedJSON(String(blob, Charsets.UTF_8)), key)
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (e: Exception) {
                Log.w(TAG, "[Attachments] Can't read ${ref.id}: ${e.javaClass.simpleName}")
                null
            }
        }
    }

    /**
     * Remove an attachment from a note.
     */
    suspend fun removeAttachment(noteId: String, attachmentId: String) {
        val noteRepository = noteRepo ?: return
        val attachmentRepository = attachmentRepo ?: return
        val key = keyManager.masterKey ?: return
        val note = noteRepository.get(id = noteId, key = key) ?: return

        // Find the attachment to get its blob ID
        val ref = note.attachments.firstOrNull { it.id == attachmentId } ?: return

        // Remove from note
        noteRepository.removeAttachment(noteId = noteId, attachmentId = attachmentId)

        // Delete blob
        try {
            attachmentRepository.delete(ref.data)
        } catch (_: Exception) { /* ignore */ }

        refreshNote(noteId)
        scheduleDebouncedSync()
    }

    /**
     * Resolve an attachment blob to a data URL for the markdown preview WebView.
     * Matches iOS's MarkdownPreviewView.resolveAttachment behaviour.
     */
    suspend fun resolveAttachmentDataUrl(
        attachmentId: String,
        attachments: List<AttachmentRef>,
    ): String? {
        val attachmentRepository = attachmentRepo ?: return null
        val key = keyManager.masterKey ?: return null

        // Find the matching attachment ref (by id or data field)
        val ref = attachments.firstOrNull { ref ->
            ref.id == attachmentId ||
                    ref.data == attachmentId ||
                    ref.id.replace("-", "").startsWith(attachmentId) ||
                    ref.data.replace("-", "").startsWith(attachmentId)
        } ?: return null

        return try {
            val blobData = attachmentRepository.getBlob(ref.data) ?: return null
            val blobString = String(blobData, Charsets.UTF_8)
            val encrypted = CryptoService.parseEncryptedJSON(blobString)
            val decryptedData = CryptoService.decrypt(encrypted, key)
            val base64 = android.util.Base64.encodeToString(decryptedData, android.util.Base64.NO_WRAP)
            "data:${ref.mimeType};base64,$base64"
        } catch (e: Exception) {
            Log.w("AppState", "Failed to resolve attachment $attachmentId: ${e.message}")
            null
        }
    }

    // MARK: - Version History

    /**
     * Get version history for a note.
     */
    suspend fun getVersionsForNote(noteId: String): List<NoteVersionRecord> {
        return versionRepo?.getVersions(noteId) ?: emptyList()
    }

    /**
     * Decrypt a version's content for preview.
     */
    fun decryptVersionContent(version: NoteVersionRecord): String? {
        val key = keyManager.masterKey ?: return null
        return try {
            val enc = CryptoService.parseEncryptedJSON(version.content)
            CryptoService.decryptText(enc, key)
        } catch (e: Exception) {
            Log.w(TAG, "Failed to decrypt version content: ${e.message}")
            null
        }
    }

    // MARK: - Version Restore

    /**
     * Restore a note to a previous version, keeping a snapshot of the current
     * state. Returns false, having changed nothing, if the version can't be read.
     */
    suspend fun restoreVersion(noteId: String, version: NoteVersionRecord): Boolean {
        val noteRepository = noteRepo ?: return false
        val versionRepository = versionRepo ?: return false
        val key = keyManager.masterKey ?: return false

        val note: DecryptedNote
        val content: String
        val tags: List<String>
        try {
            note = noteRepository.get(id = noteId, key = key) ?: return false
            content = CryptoService.decryptText(CryptoService.parseEncryptedJSON(version.content), key)
            tags = decryptVersionTags(version.tags, key)
        } catch (e: Exception) {
            Log.w(TAG, "[Versions] Can't read version ${version.versionKey}: ${e.javaClass.simpleName}")
            return false
        }

        noteRepository.getRaw(noteId)?.let {
            versionRepository.insertOrReplace(NoteVersionRecord.safetySnapshot(it, "pre-restore"))
        }
        val restored = note.copy(
            content = content,
            tags = tags,
            syntaxLanguage = version.syntaxLanguage ?: note.syntaxLanguage,
            wordWrap = version.wordWrap ?: note.wordWrap,
            showPreview = version.showPreview ?: note.showPreview,
            color = version.color,
        )
        if (!noteRepository.update(restored, key)) return false

        refreshNote(noteId)
        scheduleDebouncedSync()
        return true
    }

    /** Older builds stored '[]' for a version with no tags. */
    private fun decryptVersionTags(tags: String, key: SecretKey): List<String> {
        if (tags == "[]") return emptyList()
        val text = CryptoService.decryptText(CryptoService.parseEncryptedJSON(tags), key)
        return when {
            text.isEmpty() -> emptyList()
            text.startsWith("[") -> json.decodeFromString(text)
            text.startsWith("\"") -> listOf(json.decodeFromString<String>(text))
            else -> text.split(",").map { it.trim() }.filter { it.isNotEmpty() }
        }
    }

    // MARK: - Settings

    /**
     * Update and persist user settings.
     */
    suspend fun updateSettings(newSettings: UserSettings) {
        val repo = settingsRepo ?: return
        _settings.value = newSettings
        repo.save(newSettings)
        _sortOrder.value = newSettings.sort
        keyManager.autoLockTimeout = newSettings.autoLockTimeout.toLong() * 60L
    }

    // MARK: - Wipe & Re-onboard

    /**
     * Delete everything -- database, Keystore, in-memory state -- so the user
     * can start fresh as if the app was just installed.
     */
    fun wipeAllData() {
        val context = getApplication<Application>()

        // Clear secure storage
        KeystoreService.deleteAPIKey(context)
        KeystoreService.deleteClientId(context)
        KeystoreService.deleteBiometricKey(context)

        // Close and clear the database singleton before deleting the file
        JotteryDatabase.destroyInstance()

        // Delete database file
        context.deleteDatabase("jottery.db")

        // Stop sync and SSE
        sseClientInstance?.stop()
        syncClientInstance = null
        syncServiceInstance = null
        sseClientInstance = null

        // Reset all in-memory state
        db = null
        noteRepo = null
        encryptionRepo = null
        settingsRepo = null
        syncRepo = null
        versionRepo = null
        attachmentRepo = null
        savedSearchRepo = null
        _notes.value = emptyList()
        _archivedNotes.value = emptyList()
        _filteredNotes.value = emptyList()
        _savedSearches.value = emptyList()
        _pendingConflicts.value = emptyList()
        _inboxItems.value = emptyList()
        _showArchive.value = false
        _selectedNoteId.value = null
        _searchQuery.value = ""
        _syncEnabled.value = false
        _isSyncing.value = false
        _lastSyncAt.value = null
        _syncError.value = null
        _syncStatusMessage.value = null
        _settings.value = UserSettings.defaults
        _pendingEditorNote.value = null
        keyManager.lock()
        _isLocked.value = true
        _isFirstLaunch.value = true

        // Re-initialise so the DB is recreated fresh
        initialise()
    }

    // MARK: - Password Change

    /**
     * Change the vault password. The key stays the same and only its wrapping
     * changes, in one write: no note is touched, so a kill can't leave a mix and
     * other devices keep working. A legacy vault moves to an envelope here, which
     * replaces the full re-encryption that used to run.
     */
    suspend fun changePassword(currentPassword: String, newPassword: String) {
        val encRepo = encryptionRepo ?: throw AppStateError.NotInitialised
        val metadata = encRepo.get() ?: throw AppStateError.NoVault

        val masterKeyData = unwrapVaultKey(metadata, currentPassword)
            ?: throw AppStateError.WrongPassword

        EnvelopeService.saveLocalEnvelope(encRepo, masterKeyData, newPassword)

        // Update biometric key if enabled (master key unchanged)
        if (KeystoreService.hasBiometricKey(getApplication())) {
            try {
                KeystoreService.storeBiometricKey(getApplication(), masterKeyData)
            } catch (_: Exception) { /* ignore */ }
        }

        // The account's blob follows the new password. If this fails, the next
        // unlock heals it, because the blob then won't open with the new password.
        val client = syncClientInstance
        val userId = syncRepo?.getSyncMetadata()?.userId
        if (client != null && !userId.isNullOrEmpty()) {
            val dataForServer = masterKeyData.copyOf()
            viewModelScope.launch {
                try {
                    EnvelopeService.publishKey(client, userId, newPassword, dataForServer)
                } catch (e: Exception) {
                    Log.w(TAG, "[Envelope] Server re-wrap failed: ${e.javaClass.simpleName}")
                }
            }
        }
    }

    /** The vault's key, if [password] opens it. */
    private suspend fun unwrapVaultKey(metadata: EncryptionMetadata, password: String): ByteArray? =
        withContext(Dispatchers.Default) {
            val candidate: ByteArray? = if (metadata.envelopeVersion != null) {
                val deviceSalt = metadata.deviceSalt ?: throw AppStateError.InvalidSalt
                val wrappedJSON = metadata.localWrappedMaster ?: throw AppStateError.InvalidSalt
                val deviceKey = CryptoService.deriveKey(
                    password,
                    android.util.Base64.decode(deviceSalt, android.util.Base64.NO_WRAP),
                    CryptoService.DEFAULT_ITERATIONS,
                )
                try {
                    CryptoService.unwrapMasterKey(CryptoService.parseEncryptedJSON(wrappedJSON), deviceKey)
                } catch (_: Exception) {
                    null
                }
            } else {
                val salt = metadata.saltData ?: throw AppStateError.InvalidSalt
                CryptoService.deriveKey(
                    password, salt, metadata.iterations ?: CryptoService.DEFAULT_ITERATIONS,
                ).encoded
            }
            candidate?.takeIf {
                val key = SecretKeySpec(it, "AES")
                if (metadata.verification != null) verifyWithToken(metadata.verification, key)
                else verifyByDecryptingNote(key)
            }
        }

    // MARK: - Joining an Account

    /**
     * A device registered with a server but not yet holding the account's key.
     * Sync stays off until it does. Kept in memory only, and it survives rotation
     * here in the ViewModel, so the setup screen can't lose track of the device.
     */
    data class PendingConnection(
        val endpoint: String,
        val apiKey: String,
        val clientId: String,
        val userId: String,
        val accountPassword: String?,
    )

    /** Progress of a setup or connect action, which outlives any one screen. */
    sealed interface ConnectState {
        data object Idle : ConnectState
        data object Working : ConnectState
        data class Failed(val message: String) : ConnectState

        /** No account key yet, so this password is new: it has to be typed twice. */
        data object ConfirmNewPassword : ConnectState

        /** The account uses another key: the user chooses whether to move to it. */
        data object NeedsMerge : ConnectState
        data object Done : ConnectState
    }

    private val _pendingConnection = MutableStateFlow<PendingConnection?>(null)
    val pendingConnection: StateFlow<PendingConnection?> = _pendingConnection.asStateFlow()

    private val _connectState = MutableStateFlow<ConnectState>(ConnectState.Idle)
    val connectState: StateFlow<ConnectState> = _connectState.asStateFlow()

    /** Held while the user decides whether to move this vault onto the account's key. */
    private var pendingMerge: PendingMerge? = null

    private class PendingMerge(
        val connection: PendingConnection,
        val accountKey: ByteArray,
        val notesPassword: String,
    )

    fun clearConnectState() {
        _connectState.value = ConnectState.Idle
    }

    private fun text(id: Int, vararg args: Any): String =
        getApplication<Application>().getString(id, *args)

    /** Whatever went wrong while joining, as something the user can act on. */
    private fun connectErrorText(e: Exception): String = when (e) {
        is EnvelopeException.CannotUnwrap -> text(R.string.connect_error_cannot_unwrap)
        is ConnectRefused -> e.message.orEmpty()
        is SyncClientException, is java.io.IOException -> text(R.string.connect_error_network)
        else -> e.message ?: text(R.string.connect_error_network)
    }

    private class ConnectRefused(message: String) : Exception(message)

    /** Runs a connect action in viewModelScope, so leaving a screen can't cancel it. */
    private fun connecting(block: suspend () -> Unit) {
        _connectState.value = ConnectState.Working
        viewModelScope.launch {
            try {
                block()
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (e: Exception) {
                Log.e(TAG, "[Connect] Failed: ${e.javaClass.simpleName}")
                _connectState.value = ConnectState.Failed(connectErrorText(e))
            }
        }
    }

    private fun refuseMatchingPasswords(notesPassword: String, accountPassword: String?) {
        if (accountPassword != null && notesPassword == accountPassword) {
            throw ConnectRefused(text(R.string.password_same_as_account))
        }
    }

    private fun refuseShortNewPassword(password: String) {
        if (password.length < MIN_NOTES_PASSWORD_LENGTH) {
            throw ConnectRefused(text(R.string.password_too_short, MIN_NOTES_PASSWORD_LENGTH))
        }
    }

    /** Best effort: a failed join shouldn't leave an orphaned device on the account. */
    private suspend fun revokeQuietly(connection: PendingConnection) {
        try {
            withContext(Dispatchers.IO) {
                SyncClient(connection.endpoint, connection.apiKey, connection.clientId).revokeThisDevice()
            }
        } catch (e: Exception) {
            Log.w(TAG, "[Connect] Couldn't revoke device: ${e.javaClass.simpleName}")
        }
    }

    /** Sync starts only here, once this device holds the account's key. */
    private suspend fun finishConnection(connection: PendingConnection) {
        val context = getApplication<Application>()
        KeystoreService.setAPIKey(context, connection.apiKey)
        KeystoreService.setClientId(context, connection.clientId)

        syncRepo?.let { repo ->
            val syncMeta = repo.getSyncMetadata() ?: com.jottery.android.model.SyncMetadata()
            repo.saveSyncMetadata(syncMeta.copy(userId = connection.userId))
        }
        attachmentRepo?.markAllNotUploaded()
        updateSettings(_settings.value.copy(syncEnabled = true, syncEndpoint = connection.endpoint))

        _pendingConnection.value = null
        setupSync(connection.apiKey)
        _connectState.value = ConnectState.Done
    }

    /**
     * Setup: create a vault with no account. In viewModelScope, because the vault
     * appearing is what navigates away from the screen that asked for it.
     */
    fun createNewVault(password: String) {
        connecting {
            refuseShortNewPassword(password)
            createVault(password)
            _connectState.value = ConnectState.Idle
        }
    }

    /**
     * Setup: create this device's vault for the registered account. The server's
     * key is checked BEFORE any vault exists, so a wrong password or a network
     * failure leaves the user on the screen with nothing created.
     */
    fun connectNewVault(notesPassword: String, confirmation: String? = null) {
        val connection = _pendingConnection.value ?: return
        connecting {
            refuseMatchingPasswords(notesPassword, connection.accountPassword)

            val client = SyncClient(connection.endpoint, connection.apiKey, connection.clientId)
            var master = EnvelopeService.fetchServerKey(client, connection.userId, notesPassword)
            if (master == null) {
                // First device on the account: its key becomes the account's, and
                // is published before anything else, so no second device can
                // start on a key of its own in the meantime.
                refuseShortNewPassword(notesPassword)
                // Nothing can recover a mistyped password, so a new one is typed
                // twice. A password that opened the account's key has proved itself.
                if (confirmation == null) {
                    _connectState.value = ConnectState.ConfirmNewPassword
                    return@connecting
                }
                if (confirmation != notesPassword) {
                    throw ConnectRefused(text(R.string.password_mismatch))
                }
                master = CryptoService.generateSalt()
                EnvelopeService.publishKey(client, connection.userId, notesPassword, master)
            }
            createVaultFromMasterKey(master, notesPassword)
            finishConnection(connection)
        }
    }

    /** Create the vault directly on a master key, as an envelope. */
    private suspend fun createVaultFromMasterKey(masterKey: ByteArray, password: String) {
        val repo = encryptionRepo ?: throw AppStateError.NotInitialised
        val base = EncryptionMetadata(createdAt = DateUtils.nowISO8601())
        repo.store(EnvelopeService.metadataFor(EnvelopeService.localEnvelope(masterKey, password), base))
        keyManager.unlockWithKeyData(masterKey)
        _isFirstLaunch.value = false
        _isLocked.value = false
        loadNotes()
    }

    /**
     * Settings: join an account with the vault that's already open. Compares the
     * account's key with this vault's and only starts sync once they agree, so
     * notes are never pushed under a key other devices can't read.
     */
    fun connectExistingVault(connection: PendingConnection, notesPassword: String) {
        connecting {
            refuseMatchingPasswords(notesPassword, connection.accountPassword)
            joinAccount(connection, notesPassword)
        }
    }

    private suspend fun joinAccount(connection: PendingConnection, notesPassword: String) {
        val encRepo = encryptionRepo ?: throw AppStateError.NotInitialised
        val localKey = keyManager.masterKey?.encoded ?: throw AppStateError.NotInitialised
        val metadata = encRepo.get() ?: throw AppStateError.NoVault
        val client = SyncClient(connection.endpoint, connection.apiKey, connection.clientId)

        val status = try {
            EnvelopeService.checkAccountKey(client, connection.userId, notesPassword, localKey)
        } catch (e: Exception) {
            revokeQuietly(connection)
            throw e
        }

        when (status) {
            EnvelopeService.AccountKeyStatus.NoServerKey -> {
                // This password is about to protect the account's key, so it must
                // be the one that really opens this vault.
                if (unwrapVaultKey(metadata, notesPassword) == null) {
                    revokeQuietly(connection)
                    throw ConnectRefused(text(R.string.connect_error_wrong_notes_password))
                }
                EnvelopeService.publishKey(client, connection.userId, notesPassword, localKey)
                if (metadata.envelopeVersion == null) {
                    EnvelopeService.saveLocalEnvelope(encRepo, localKey, notesPassword)
                }
                finishConnection(connection)
            }
            EnvelopeService.AccountKeyStatus.MatchesLocalKey -> {
                if (metadata.envelopeVersion == null) {
                    EnvelopeService.saveLocalEnvelope(encRepo, localKey, notesPassword)
                }
                finishConnection(connection)
            }
            is EnvelopeService.AccountKeyStatus.Differs -> {
                pendingMerge = PendingMerge(connection, status.accountKey, notesPassword)
                _connectState.value = ConnectState.NeedsMerge
            }
        }
    }

    /** The user chose to move this vault onto the account's key. */
    fun confirmMerge() {
        val merge = pendingMerge ?: return
        pendingMerge = null
        connecting {
            try {
                switchVaultToMasterKey(merge.accountKey, merge.notesPassword)
            } catch (e: Exception) {
                revokeQuietly(merge.connection)
                throw e
            }
            finishConnection(merge.connection)
        }
    }

    /** The user chose not to merge: leave the account and stay as they were. */
    fun cancelMerge() {
        val merge = pendingMerge ?: return
        pendingMerge = null
        _connectState.value = ConnectState.Idle
        viewModelScope.launch { revokeQuietly(merge.connection) }
    }

    /**
     * Move the whole vault onto [masterKey] in one transaction, then switch to it.
     * From now on the vault opens with [notesPassword].
     */
    private suspend fun switchVaultToMasterKey(masterKey: ByteArray, notesPassword: String) {
        val encRepo = encryptionRepo ?: throw AppStateError.NotInitialised
        val oldKey = keyManager.masterKey ?: throw AppStateError.NotInitialised
        val current = encRepo.get() ?: throw AppStateError.NoVault
        val newMetadata = EnvelopeService.metadataFor(
            EnvelopeService.localEnvelope(masterKey, notesPassword), current,
        )
        val database = db ?: throw AppStateError.NotInitialised
        val service = VaultReKeyService(
            noteRepo = noteRepo ?: throw AppStateError.NotInitialised,
            versionRepo = versionRepo ?: throw AppStateError.NotInitialised,
            attachmentRepo = attachmentRepo ?: throw AppStateError.NotInitialised,
            savedSearchRepo = savedSearchRepo ?: throw AppStateError.NotInitialised,
            encryptionRepo = encRepo,
            transaction = { block -> database.withTransaction { block() } },
        )

        // No sync may read or write rows halfway through, and the key changes
        // only once the transaction has committed.
        syncMutex.withLock {
            withContext(Dispatchers.Default) {
                service.reKey(oldKey, SecretKeySpec(masterKey, "AES"), newMetadata)
            }
            keyManager.unlockWithKeyData(masterKey)
        }
        loadNotes()
    }

    /**
     * Bring the vault and the account's wrapped key into line after an unlock. See
     * [EnvelopeService.reconcile]. Any failure leaves things as they are, and the
     * next unlock tries again.
     */
    private suspend fun reconcileAccountKey(notesPassword: String) {
        val encRepo = encryptionRepo ?: return
        val client = syncClientInstance ?: return
        val syncMeta = syncRepo?.getSyncMetadata() ?: return
        val userId = syncMeta.userId
        if (userId.isNullOrEmpty()) return
        val localKey = keyManager.masterKey ?: return
        val localKeyData = localKey.encoded
        val isLegacy = encRepo.get()?.envelopeVersion == null

        try {
            val action = EnvelopeService.reconcile(
                client = client,
                userId = userId,
                notesPassword = notesPassword,
                localKey = localKeyData,
                isLegacyVault = isLegacy,
                canVouchForLocalKey = {
                    syncMeta.lastSyncAt != null && noteRepo?.allNotesOpenWith(localKey) == true
                },
            )
            when (action) {
                EnvelopeService.Reconciliation.Publish -> {
                    EnvelopeService.publishKey(client, userId, notesPassword, localKeyData)
                    if (isLegacy) EnvelopeService.saveLocalEnvelope(encRepo, localKeyData, notesPassword)
                }
                is EnvelopeService.Reconciliation.InStep -> {
                    if (action.finishMigration) {
                        EnvelopeService.saveLocalEnvelope(encRepo, localKeyData, notesPassword)
                    }
                }
                is EnvelopeService.Reconciliation.MoveToAccountKey -> {
                    switchVaultToMasterKey(action.accountKey, notesPassword)
                    triggerSync()
                }
                EnvelopeService.Reconciliation.Diverged -> showMessage(text(R.string.connect_key_diverged))
                EnvelopeService.Reconciliation.CannotHeal ->
                    Log.w(TAG, "[Envelope] Account key won't open and this device can't vouch for its own")
            }
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: Exception) {
            Log.w(TAG, "[Envelope] Reconcile failed (will retry on next unlock): ${e.javaClass.simpleName}")
        }
    }

    /**
     * Set up the sync client. Pass apiKey directly after registration
     * to avoid Keystore retrieval timing issues; otherwise reads from Keystore.
     */
    fun setupSync(apiKey: String? = null) {
        val currentSettings = _settings.value
        if (!currentSettings.syncEnabled) {
            Log.d(TAG, "[Sync] setupSync: settings.syncEnabled is false -- skipping")
            return
        }
        val endpoint = currentSettings.syncEndpoint
        if (endpoint.isNullOrEmpty()) {
            Log.d(TAG, "[Sync] setupSync: no syncEndpoint -- skipping")
            return
        }
        val key = keyManager.masterKey
        if (key == null) {
            Log.d(TAG, "[Sync] setupSync: no masterKey -- skipping")
            return
        }

        val context = getApplication<Application>()
        val resolvedApiKey = apiKey ?: KeystoreService.getAPIKey(context)
        if (resolvedApiKey.isNullOrEmpty()) {
            Log.d(TAG, "[Sync] setupSync: no API key in Keystore")
            _syncError.value = "No API key found. Please re-register the device."
            return
        }
        val clientId = KeystoreService.getClientId(context)
        if (clientId.isNullOrEmpty()) {
            Log.d(TAG, "[Sync] setupSync: no client ID in Keystore")
            _syncError.value = "No client ID found. Please re-register the device."
            return
        }

        Log.d(TAG, "[Sync] setupSync: OK -- endpoint=$endpoint")

        val noteRepository = noteRepo ?: return
        val versionRepository = versionRepo ?: return
        val attachmentRepository = attachmentRepo ?: return
        val syncRepository = syncRepo ?: return
        val savedSearchRepository = savedSearchRepo ?: return

        val client = SyncClient(endpoint, resolvedApiKey, clientId)
        val sse = SSEClient(endpoint, viewModelScope)
        val service = SyncService(
            syncClient = client,
            sseClient = sse,
            noteRepo = noteRepository,
            versionRepo = versionRepository,
            attachmentRepo = attachmentRepository,
            syncRepo = syncRepository,
            savedSearchRepo = savedSearchRepository,
            keyProvider = { keyManager.masterKey },
            scope = viewModelScope,
            transaction = { block -> db?.withTransaction { block() } },
        )

        service.onSyncStatusChanged = { status ->
            _syncStatusMessage.value = status
        }
        service.onNotesQuarantined = { count ->
            _syncError.value = getApplication<Application>().resources
                .getQuantityString(R.plurals.sync_notes_quarantined, count, count)
        }
        service.onConflictDetected = { conflict ->
            _pendingConflicts.value = _pendingConflicts.value.filter { it.id != conflict.id } + conflict
        }

        // Wire SSE to trigger sync on server-sent events
        sse.onSyncNeeded = {
            viewModelScope.launch { triggerSync() }
        }

        syncClientInstance = client
        syncServiceInstance = service
        sseClientInstance = sse
        _syncEnabled.value = true
        _syncError.value = null

        // Start SSE and run initial sync
        viewModelScope.launch {
            // Conflicts are kept across lock and restart
            try {
                val stored = withContext(Dispatchers.Default) { service.loadConflicts() }
                _pendingConflicts.value = stored
            } catch (e: Exception) {
                Log.w(TAG, "[Sync] Couldn't load conflicts: ${e.javaClass.simpleName}")
            }
            try { service.startSSE() } catch (e: Exception) {
                Log.w(TAG, "[Sync] SSE start failed: ${e.message}")
            }
            try { triggerSync() } catch (e: Exception) {
                Log.w(TAG, "[Sync] Initial sync failed: ${e.message}")
            }
        }
    }

    /**
     * Trigger a sync cycle (push then pull).
     */
    suspend fun triggerSync() {
        // Overlapping cycles interleave pushes and pulls, so a request that
        // arrives during a sync just asks for one more run afterwards.
        if (!syncMutex.tryLock()) {
            syncRequested = true
            return
        }
        try {
            do {
                syncRequested = false
                runSyncCycle()
            } while (syncRequested)
        } finally {
            syncMutex.unlock()
        }
    }

    private suspend fun runSyncCycle() {
        val service = syncServiceInstance
        if (service == null || !_syncEnabled.value) {
            Log.d(TAG, "[Sync] triggerSync: sync not enabled or no service -- aborting")
            return
        }
        Log.d(TAG, "[Sync] triggerSync: starting sync cycle")
        _isSyncing.value = true
        _syncError.value = null

        try {
            val newCount = withContext(Dispatchers.IO) { service.fullSync() }
            _isSyncing.value = false
            _lastSyncAt.value = Instant.now()
            _syncStatusMessage.value = if (newCount > 0) {
                "Synced \u2014 $newCount new note${if (newCount == 1) "" else "s"}"
            } else {
                "Synced \u2014 up to date"
            }
            loadNotes()
            // A pull can move the server's side of a conflict on
            _pendingConflicts.value = withContext(Dispatchers.Default) { service.loadConflicts() }
            loadInboxItems()
        } catch (e: Exception) {
            Log.e(TAG, "[Sync] triggerSync failed: ${e.message}", e)
            _isSyncing.value = false
            _syncError.value = "Sync failed: ${e.message}"
            _syncStatusMessage.value = null
        }

        viewModelScope.launch {
            delay(4_000)
            val current = _syncStatusMessage.value
            if (current != null && current.startsWith("Synced")) {
                _syncStatusMessage.value = null
            }
        }
    }

    /**
     * Force a full sync -- re-downloads all notes from the server.
     */
    suspend fun forceFullSync() {
        val service = syncServiceInstance
        if (service == null || !_syncEnabled.value) return
        _isSyncing.value = true
        _syncError.value = null
        _syncStatusMessage.value = "Full sync in progress\u2026"

        try {
            withContext(Dispatchers.IO) { service.fullSync(fullPull = true) }
            _isSyncing.value = false
            _lastSyncAt.value = Instant.now()
            _syncStatusMessage.value = "Full sync complete"
            loadNotes()
        } catch (e: Exception) {
            Log.e(TAG, "[Sync] forceFullSync failed: ${e.message}", e)
            _isSyncing.value = false
            _syncError.value = "Full sync failed: ${e.message}"
            _syncStatusMessage.value = null
        }

        viewModelScope.launch {
            delay(4_000)
            if (_syncStatusMessage.value == "Full sync complete") {
                _syncStatusMessage.value = null
            }
        }
    }

    /**
     * Register this device with a sync server. Nothing is enabled yet: the device
     * becomes [pendingConnection] until it holds the account's key.
     */
    suspend fun registerDevice(
        serverUrl: String,
        email: String,
        password: String,
        deviceName: String,
    ): Result<PendingConnection> {
        return try {
            val tempClient = SyncClient(serverUrl, "", "")
            val response = withContext(Dispatchers.IO) {
                tempClient.registerDevice(
                    RegisterDeviceRequest(
                        email = email,
                        password = password,
                        deviceName = deviceName,
                    )
                )
            }
            val connection = PendingConnection(
                endpoint = serverUrl,
                apiKey = response.apiKey,
                clientId = response.clientId,
                userId = response.userId,
                accountPassword = password,
            )
            _pendingConnection.value = connection
            Result.success(connection)
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: Exception) {
            Log.e(TAG, "[Sync] registerDevice failed: ${e.javaClass.simpleName}")
            Result.failure(e)
        }
    }

    /** Give up on a registered device that never finished joining. */
    fun abandonPendingConnection() {
        val connection = _pendingConnection.value ?: return
        _pendingConnection.value = null
        viewModelScope.launch { revokeQuietly(connection) }
    }

    // MARK: - Credential Import (multi-device setup)

    /** What a credential string decrypts to. */
    private class ImportedCredentials(
        val endpoint: String,
        val apiKey: String,
        val salt: ByteArray,
        val iterations: Int,
    )

    /**
     * Setup: join an account with a credential string from another device.
     * Supports encrypted (jottery:v1:) and legacy (base64 JSON) strings.
     */
    fun importFromDevice(credentialString: String, password: String, deviceName: String) {
        connecting {
            val credentials = parseCredentials(credentialString.trim(), password)
            val connection = cloneDevice(credentials, deviceName)
            val client = SyncClient(connection.endpoint, connection.apiKey, connection.clientId)

            // Any failure but an explicit 404 stops the import: carrying on would
            // leave this device on a key of its own.
            val master = try {
                EnvelopeService.fetchServerKey(client, connection.userId, password)
            } catch (e: Exception) {
                revokeQuietly(connection)
                throw e
            }

            if (master != null) {
                createVaultFromMasterKey(master, password)
            } else {
                // The account has no wrapped key: its key comes from this salt
                createVault(password, credentials.salt, credentials.iterations)
            }
            finishConnection(connection)
        }
    }

    /** Settings: join an account with a credential string, keeping this vault. */
    fun importIntoExistingVault(credentialString: String, password: String, deviceName: String) {
        connecting {
            val credentials = parseCredentials(credentialString.trim(), password)
            joinAccount(cloneDevice(credentials, deviceName), password)
        }
    }

    private suspend fun cloneDevice(credentials: ImportedCredentials, deviceName: String): PendingConnection {
        val response = withContext(Dispatchers.IO) {
            SyncClient(credentials.endpoint, "", "").cloneDevice(
                CloneDeviceRequest(apiKey = credentials.apiKey, deviceName = deviceName)
            )
        }
        return PendingConnection(
            endpoint = credentials.endpoint,
            apiKey = response.apiKey,
            clientId = response.clientId,
            userId = response.userId,
            accountPassword = null,
        )
    }

    private suspend fun parseCredentials(input: String, password: String): ImportedCredentials =
        if (input.startsWith("jottery:v1:")) {
            parseEncryptedCredentials(input, password)
        } else {
            parseLegacyCredentials(input)
        }

    /**
     * Handle encrypted credential format:
     * jottery:v1:<salt_b64>.<base64(JSON({"ciphertext":"...","iv":"..."}))>
     */
    private suspend fun parseEncryptedCredentials(input: String, password: String): ImportedCredentials {
        val payload = input.removePrefix("jottery:v1:")
        val dotIndex = payload.indexOf('.')
        if (dotIndex == -1) throw ConnectRefused(text(R.string.connect_error_bad_credentials))

        val salt: ByteArray
        val encrypted: EncryptedData
        try {
            salt = android.util.Base64.decode(payload.substring(0, dotIndex), android.util.Base64.DEFAULT)
            val payloadBytes = android.util.Base64.decode(payload.substring(dotIndex + 1), android.util.Base64.DEFAULT)
            encrypted = CryptoService.parseEncryptedJSON(String(payloadBytes, Charsets.UTF_8))
        } catch (_: Exception) {
            throw ConnectRefused(text(R.string.connect_error_bad_credentials))
        }
        if (salt.size < 32) throw ConnectRefused(text(R.string.connect_error_bad_credentials))

        // The jottery:v1: format doesn't record the iteration count
        val iterationCandidates = listOf(CryptoService.DEFAULT_ITERATIONS, CryptoService.MIN_ITERATIONS)
        for (candidate in iterationCandidates) {
            val decrypted = withContext(Dispatchers.Default) {
                try {
                    CryptoService.decryptText(encrypted, CryptoService.deriveKey(password, salt, candidate))
                } catch (_: Exception) {
                    null
                }
            } ?: continue

            val creds = try {
                json.decodeFromString<CredentialPayload>(decrypted)
            } catch (_: Exception) {
                throw ConnectRefused(text(R.string.connect_error_bad_credentials))
            }
            if (creds.endpoint.isBlank() || creds.apiKey.isBlank()) {
                throw ConnectRefused(text(R.string.connect_error_bad_credentials))
            }
            return ImportedCredentials(creds.endpoint.trimEnd('/'), creds.apiKey, salt, candidate)
        }
        throw ConnectRefused(text(R.string.connect_error_wrong_credentials_password))
    }

    /** Handle legacy credential format: base64(JSON with endpoint, apiKey, clientId, salt). */
    private fun parseLegacyCredentials(input: String): ImportedCredentials {
        val creds = try {
            val bytes = android.util.Base64.decode(input, android.util.Base64.DEFAULT)
            json.decodeFromString<LegacyCredentialPayload>(String(bytes, Charsets.UTF_8))
        } catch (_: Exception) {
            throw ConnectRefused(text(R.string.connect_error_bad_credentials))
        }
        if (creds.endpoint.isBlank() || creds.apiKey.isBlank() || creds.salt.isNullOrBlank()) {
            throw ConnectRefused(text(R.string.connect_error_bad_credentials))
        }
        return ImportedCredentials(
            endpoint = creds.endpoint.trimEnd('/'),
            apiKey = creds.apiKey,
            salt = android.util.Base64.decode(creds.salt, android.util.Base64.DEFAULT),
            iterations = CryptoService.DEFAULT_ITERATIONS,
        )
    }

    // MARK: - Backup & Import

    private fun backupService(): BackupService = BackupService(
        noteRepo = noteRepo ?: throw AppStateError.NotInitialised,
        versionRepo = versionRepo ?: throw AppStateError.NotInitialised,
        attachmentRepo = attachmentRepo ?: throw AppStateError.NotInitialised,
        savedSearchRepo = savedSearchRepo ?: throw AppStateError.NotInitialised,
        encryptionRepo = encryptionRepo ?: throw AppStateError.NotInitialised,
        transaction = { block ->
            (db ?: throw AppStateError.NotInitialised).withTransaction { block() }
        },
    )

    /** Write an encrypted backup of the whole vault to [out]. */
    suspend fun createBackup(out: java.io.OutputStream): BackupService.Summary {
        val key = keyManager.masterKey ?: throw AppStateError.NotInitialised
        return withContext(Dispatchers.IO) { backupService().write(out, key) }
    }

    /**
     * Establish a backup's key from [password] and check the whole file, changing
     * nothing, so the user can confirm what a restore will do.
     */
    suspend fun inspectBackup(source: BackupSource, password: String): RestorePlan =
        withContext(Dispatchers.Default) {
            backupService().inspect(source, password, keyManager.masterKey)
        }

    /** Merge a checked backup into the open vault. */
    suspend fun restoreBackup(source: BackupSource, plan: RestorePlan): RestoreResult {
        val key = keyManager.masterKey ?: throw AppStateError.NotInitialised
        val result = withContext(Dispatchers.Default) { backupService().restore(source, plan, key) }
        loadNotes()
        scheduleDebouncedSync()
        return result
    }

    /**
     * Fresh install: make the backup's vault this device's vault and open it.
     * Runs in viewModelScope because opening the vault leaves the setup screen.
     */
    fun restoreAsNewVault(source: BackupSource, password: String, onFailure: (Throwable) -> Unit) {
        viewModelScope.launch {
            try {
                val plan = inspectBackup(source, password)
                withContext(Dispatchers.Default) { backupService().restoreAsNewVault(source, plan) }
                keyManager.unlockWithKeyData(plan.keyBytes())
                _isFirstLaunch.value = false
                _isLocked.value = false
                loadNotes()
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (e: Exception) {
                Log.e(TAG, "[Backup] Restore into a new vault failed: ${e.javaClass.simpleName}")
                onFailure(e)
            }
        }
    }

    /** Import notes from a JSON export file. */
    suspend fun importNotes(jsonString: String, strategy: ImportStrategy): ImportService.Result {
        val nRepo = noteRepo ?: throw AppStateError.NotInitialised
        val aRepo = attachmentRepo ?: throw AppStateError.NotInitialised
        val database = db ?: throw AppStateError.NotInitialised
        val key = keyManager.masterKey ?: throw AppStateError.NotInitialised
        val service = ImportService(nRepo, aRepo) { block -> database.withTransaction { block() } }
        val result = withContext(Dispatchers.Default) { service.`import`(jsonString, key, strategy) }
        loadNotes()
        scheduleDebouncedSync()
        return result
    }

    /**
     * Export notes to a file the user chose. With no [ids] that's every note,
     * including archived ones. Returns a message to show the user.
     */
    suspend fun exportTo(context: android.content.Context, uri: android.net.Uri, ids: Set<String>?): String {
        val aRepo = attachmentRepo ?: return ""
        val key = keyManager.masterKey ?: return ""
        val all = _notes.value + _archivedNotes.value
        val notes = if (ids == null) all else all.filter { it.id in ids }
        return try {
            val result = withContext(Dispatchers.IO) {
                val stream = context.contentResolver.openOutputStream(uri) ?: throw java.io.FileNotFoundException()
                stream.use { ExportService(aRepo).export(notes, key, it) }
            }
            if (result.skippedAttachments > 0) {
                text(R.string.export_done_skipped, result.notes, result.skippedAttachments)
            } else {
                text(R.string.export_done, result.notes)
            }
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: Exception) {
            Log.e(TAG, "[Export] Failed: ${e.javaClass.simpleName}")
            text(R.string.export_failed, e.message ?: e.javaClass.simpleName)
        }
    }

    /** Add tags to several notes. */
    suspend fun addTagsToNotes(ids: Set<String>, tags: List<String>) =
        changeTags(ids) { current -> (current + tags).distinct() }

    /** Remove tags from several notes. */
    suspend fun removeTagsFromNotes(ids: Set<String>, tags: List<String>) =
        changeTags(ids) { current -> current - tags.toSet() }

    private suspend fun changeTags(ids: Set<String>, change: (List<String>) -> List<String>) {
        val repo = noteRepo ?: return
        val key = keyManager.masterKey ?: return
        ids.forEach { repo.changeTags(it, key, change) }
        ids.forEach { refreshNote(it) }
        scheduleDebouncedSync()
    }

    /**
     * Resolve a sync conflict for a specific note.
     */
    suspend fun resolveConflict(noteId: String, strategy: ConflictResolutionStrategy) {
        Log.d(TAG, "[Sync] resolveConflict: noteId=$noteId, strategy=$strategy")

        val conflict = _pendingConflicts.value.firstOrNull { it.id == noteId } ?: return
        val service = syncServiceInstance ?: return

        // On failure the conflict stays: the caller shows the error and the user
        // can choose again. Nothing has been lost.
        withContext(Dispatchers.IO) {
            service.resolveConflict(conflict, strategy)
        }

        _pendingConflicts.value = _pendingConflicts.value.filter { it.id != noteId }
        loadNotes()
        triggerSync()
    }

    // MARK: - Inbox

    /** Fetch the inbox. A failure keeps what's already shown, and is no sync error. */
    suspend fun loadInboxItems() {
        val client = syncClientInstance ?: return
        try {
            val items = withContext(Dispatchers.IO) { client.getInboxItems() }
            // The server is not trusted: ids go into a URL path on delete
            _inboxItems.value = items.filter { SyncValidator.isUuid(it.id) }
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: Exception) {
            Log.w(TAG, "[Inbox] Load failed: ${e.javaClass.simpleName}")
        }
    }

    /**
     * Turn an inbox item into a note. The server's copy is deleted only once the
     * note is saved, so a failure can't lose the item. If the delete itself fails
     * the item stays listed, and accepting it again would make a second note, so
     * the user is told.
     */
    suspend fun acceptInboxItem(item: InboxItem) {
        val repo = noteRepo ?: return
        val key = keyManager.masterKey ?: return
        val client = syncClientInstance ?: return

        val note = repo.create(content = item.content, tags = item.tags, key = key)
        notesRevision.incrementAndGet()
        _notes.value = listOf(note) + _notes.value
        scheduleSearch()
        scheduleDebouncedSync()

        try {
            withContext(Dispatchers.IO) { client.deleteInboxItem(item.id) }
            _inboxItems.value = _inboxItems.value.filter { it.id != item.id }
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: Exception) {
            showMessage(text(R.string.inbox_delete_failed))
        }
    }

    /** Delete one inbox item from the server. */
    suspend fun deleteInboxItem(item: InboxItem) {
        val client = syncClientInstance ?: return
        try {
            withContext(Dispatchers.IO) { client.deleteInboxItem(item.id) }
            _inboxItems.value = _inboxItems.value.filter { it.id != item.id }
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: Exception) {
            showMessage(text(R.string.inbox_delete_failed))
        }
    }

    /** Delete every inbox item from the server. */
    suspend fun deleteAllInboxItems() {
        val client = syncClientInstance ?: return
        try {
            withContext(Dispatchers.IO) { client.deleteAllInboxItems() }
            _inboxItems.value = emptyList()
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: Exception) {
            showMessage(text(R.string.inbox_delete_failed))
        }
    }

    // MARK: - Lifecycle

    /**
     * Handle foreground/background transitions, called from MainActivity. The
     * timeout counts from the last user activity, so time in the background and
     * idle time before it both count, and timers that froze don't matter.
     */
    fun handleLifecycleEvent(started: Boolean) {
        if (!started) {
            isBackgrounded = true
            sseClientInstance?.stop()
            return
        }

        if (!isBackgrounded) return
        isBackgrounded = false
        if (_isLocked.value) return

        if (keyManager.isIdleTimeoutExceeded()) {
            lock()
            return
        }
        keyManager.recordActivity()

        // Restart SSE and catch up on what was missed
        if (_syncEnabled.value) {
            viewModelScope.launch {
                try { syncServiceInstance?.startSSE() } catch (_: Exception) {}
                triggerSync()
            }
        }
    }

    companion object {
        /** New notes passwords only: existing short passwords must keep working. */
        const val MIN_NOTES_PASSWORD_LENGTH = 8

        private const val TAG = "AppState"
        private const val PREFS_NAME = "jottery_prefs"
        private const val KEY_EDITOR_FONT_SCALE = "editorFontScale"
        private const val KEY_SCREEN_SECURITY = "screenSecurity"
        private const val MAX_LOAD_ATTEMPTS = 3
    }
}

/** A default name for an export file. */
fun defaultExportName(): String = "jottery-export-${java.time.LocalDate.now()}.json"

// MARK: - Error Types

sealed class AppStateError(message: String) : Exception(message) {
    data object NotInitialised : AppStateError("Application not initialised")
    data object NoVault : AppStateError("No vault found")
    data object InvalidSalt : AppStateError("Invalid encryption salt")
    data object WrongPassword : AppStateError("Incorrect password")
}
