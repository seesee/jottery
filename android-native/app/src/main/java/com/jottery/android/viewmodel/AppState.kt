package com.jottery.android.viewmodel

import android.app.Application
import android.util.Log
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
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
import com.jottery.android.crypto.EncryptedData
import com.jottery.android.service.BackupService
import com.jottery.android.service.EnvelopeException
import com.jottery.android.service.EnvelopeService
import com.jottery.android.service.ImportService
import com.jottery.android.service.SyncService
import javax.crypto.spec.SecretKeySpec
import com.jottery.android.util.DateUtils
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import java.time.Instant
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

    // MARK: - Settings

    private val _settings = MutableStateFlow(UserSettings.defaults)
    val settings: StateFlow<UserSettings> = _settings.asStateFlow()

    // MARK: - Pending Editor Note

    private val _pendingEditorNote = MutableStateFlow<DecryptedNote?>(null)
    val pendingEditorNote: StateFlow<DecryptedNote?> = _pendingEditorNote.asStateFlow()

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
    private var backgroundedAt: Instant? = null

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

        // Wire auto-lock so the timer triggers a full app lock (UI + key wipe)
        keyManager.onAutoLock = {
            lock()
        }

        try {
            val database = JotteryDatabase.getInstance(context)
            this.db = database
            val verRepo = VersionRepository(database.noteVersionDao())
            this.versionRepo = verRepo
            this.noteRepo = NoteRepository(database.noteDao(), database.noteVersionDao())
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
        val key = keyManager.unlock(password, salt, iterations)

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

            val deviceKey = CryptoService.deriveKey(password, deviceSalt, CryptoService.DEFAULT_ITERATIONS)
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

            key = keyManager.unlock(
                password = password,
                salt = saltData,
                iterations = metadata.iterations ?: CryptoService.DEFAULT_ITERATIONS,
            )

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
        _isLocked.value = false

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

        // Auto-migrate legacy vaults to envelope in background (non-fatal)
        if (metadata.envelopeVersion == null && _settings.value.syncEnabled) {
            val syncRepository = syncRepo
            val encRepo = encryptionRepo
            val client = syncClient
            if (syncRepository != null && encRepo != null && client != null) {
                val unlockPassword = password
                viewModelScope.launch(Dispatchers.IO) {
                    EnvelopeService.tryEnvelopeSetup(
                        this@AppState, syncRepository, encRepo, client, unlockPassword, key,
                    )
                }
            }
        }

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

        val result = repo.listActiveWithStats(key)
        _notes.value = result.notes
        _archivedNotes.value = repo.listArchived(key)
        _savedSearches.value = savedSearchRepo?.listActive(key) ?: emptyList()

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
        repo.update(note, key)

        // Update in-memory
        val currentNotes = _notes.value.toMutableList()
        val index = currentNotes.indexOfFirst { it.id == note.id }
        if (index >= 0) {
            val updated = note.copy(modifiedAt = Instant.now())
            currentNotes[index] = updated
            _notes.value = currentNotes
            scheduleSearch()
        }
        scheduleDebouncedSync()
    }

    /**
     * Duplicate a note by ID.
     */
    suspend fun duplicateNote(id: String) {
        val repo = noteRepo ?: return
        val key = keyManager.masterKey ?: return
        val original = repo.get(id = id, key = key) ?: return
        val duplicate = repo.create(content = original.content, tags = original.tags, key = key)
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
        _notes.value = _notes.value.filter { it.id != id }
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
        val currentNotes = _notes.value.toMutableList()
        val index = currentNotes.indexOfFirst { it.id == id }
        if (index >= 0) {
            val note = currentNotes[index]
            currentNotes[index] = note.copy(pinned = !note.pinned)
            _notes.value = currentNotes
            scheduleSearch()
        }
        scheduleDebouncedSync()
    }

    /**
     * Archive a note by ID.
     */
    suspend fun archiveNote(id: String) {
        val repo = noteRepo ?: return
        repo.archive(id)
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

        val currentNotes = _notes.value.toMutableList()
        val noteIndex = currentNotes.indexOfFirst { it.id == id }
        if (noteIndex >= 0) {
            val note = currentNotes[noteIndex]
            val newLocked = !note.locked
            currentNotes[noteIndex] = note.copy(
                locked = newLocked,
                lockedAt = if (newLocked) Instant.now() else null,
            )
            _notes.value = currentNotes
        }

        val currentArchived = _archivedNotes.value.toMutableList()
        val archivedIndex = currentArchived.indexOfFirst { it.id == id }
        if (archivedIndex >= 0) {
            val note = currentArchived[archivedIndex]
            val newLocked = !note.locked
            currentArchived[archivedIndex] = note.copy(
                locked = newLocked,
                lockedAt = if (newLocked) Instant.now() else null,
            )
            _archivedNotes.value = currentArchived
        }
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
        val source = displayedNotes
        val order = _sortOrder.value

        // No debounce needed when query is empty -- just sort (cheap)
        if (query.isBlank()) {
            _filteredNotes.value = sortNotes(source, order)
            return
        }

        searchJob = viewModelScope.launch {
            // Debounce while typing
            delay(150)

            // Run expensive filter + sort
            val filtered = filterNotes(source, query)
            val results = sortNotes(filtered, order)
            _filteredNotes.value = results
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

    /**
     * Filter notes by a search query string.
     * Supports tags (#tagname), negation (-term), exact phrases ("..."),
     * and plain text matching.
     */
    private fun filterNotes(notes: List<DecryptedNote>, query: String): List<DecryptedNote> {
        val trimmed = query.trim()
        if (trimmed.isEmpty()) return notes

        // Parse query into tokens
        val tokens = mutableListOf<SearchToken>()
        var remaining = trimmed

        while (remaining.isNotEmpty()) {
            remaining = remaining.trimStart()
            if (remaining.isEmpty()) break

            when {
                // Tag filter: #tagname
                remaining.startsWith("#") -> {
                    val tagEnd = remaining.indexOfFirst { it == ' ' || it == '|' }.let {
                        if (it < 0) remaining.length else it
                    }
                    val tag = remaining.substring(1, tagEnd)
                    if (tag.isNotEmpty()) {
                        tokens.add(SearchToken.Tag(tag))
                    }
                    remaining = remaining.substring(tagEnd)
                }
                // Negation: -term
                remaining.startsWith("-") && remaining.length > 1 && remaining[1] != ' ' -> {
                    val termEnd = remaining.indexOf(' ').let {
                        if (it < 0) remaining.length else it
                    }
                    val term = remaining.substring(1, termEnd)
                    if (term.isNotEmpty()) {
                        tokens.add(SearchToken.Negation(term))
                    }
                    remaining = remaining.substring(termEnd)
                }
                // Exact phrase: "..."
                remaining.startsWith("\"") -> {
                    val closeIdx = remaining.indexOf('"', 1)
                    if (closeIdx > 0) {
                        val phrase = remaining.substring(1, closeIdx)
                        if (phrase.isNotEmpty()) {
                            tokens.add(SearchToken.Phrase(phrase))
                        }
                        remaining = remaining.substring(closeIdx + 1)
                    } else {
                        // No closing quote -- treat rest as phrase
                        val phrase = remaining.substring(1)
                        if (phrase.isNotEmpty()) {
                            tokens.add(SearchToken.Phrase(phrase))
                        }
                        remaining = ""
                    }
                }
                // OR operator
                remaining.startsWith("|") -> {
                    tokens.add(SearchToken.Or)
                    remaining = remaining.substring(1)
                }
                // Has modifier
                remaining.startsWith("has:") -> {
                    val modEnd = remaining.indexOf(' ').let {
                        if (it < 0) remaining.length else it
                    }
                    val modifier = remaining.substring(4, modEnd)
                    tokens.add(SearchToken.HasModifier(modifier))
                    remaining = remaining.substring(modEnd)
                }
                // Plain word
                else -> {
                    val wordEnd = remaining.indexOf(' ').let {
                        if (it < 0) remaining.length else it
                    }
                    val word = remaining.substring(0, wordEnd)
                    if (word.isNotEmpty()) {
                        tokens.add(SearchToken.Word(word))
                    }
                    remaining = remaining.substring(wordEnd)
                }
            }
        }

        return notes.filter { note ->
            matchesTokens(note, tokens)
        }
    }

    /**
     * Check if a note matches the parsed search tokens.
     */
    private fun matchesTokens(note: DecryptedNote, tokens: List<SearchToken>): Boolean {
        val contentLower = note.content.lowercase()
        val tagsLower = note.tags.map { it.lowercase() }

        // Group tokens by OR boundaries
        var i = 0
        while (i < tokens.size) {
            val token = tokens[i]

            // Look ahead for OR
            if (i + 2 < tokens.size && tokens[i + 1] is SearchToken.Or) {
                val left = matchesSingleToken(note, token, contentLower, tagsLower)
                val right = matchesSingleToken(note, tokens[i + 2], contentLower, tagsLower)
                if (!left && !right) return false
                i += 3
                continue
            }

            if (token is SearchToken.Or) {
                i++
                continue
            }

            if (!matchesSingleToken(note, token, contentLower, tagsLower)) {
                return false
            }
            i++
        }
        return true
    }

    /**
     * Check if a note matches a single search token.
     */
    private fun matchesSingleToken(
        note: DecryptedNote,
        token: SearchToken,
        contentLower: String,
        tagsLower: List<String>,
    ): Boolean {
        return when (token) {
            is SearchToken.Tag -> tagsLower.any { it == token.name.lowercase() }
            is SearchToken.Negation -> !contentLower.contains(token.term.lowercase())
            is SearchToken.Phrase -> contentLower.contains(token.text.lowercase())
            is SearchToken.Word -> {
                val lower = token.text.lowercase()
                when {
                    lower.startsWith("*") && lower.endsWith("*") -> {
                        val inner = lower.trim('*')
                        contentLower.contains(inner) || tagsLower.any { it.contains(inner) }
                    }
                    lower.startsWith("*") -> {
                        val suffix = lower.removePrefix("*")
                        contentLower.contains(suffix) || tagsLower.any { it.endsWith(suffix) }
                    }
                    lower.endsWith("*") -> {
                        val prefix = lower.removeSuffix("*")
                        contentLower.contains(prefix) || tagsLower.any { it.startsWith(prefix) }
                    }
                    else -> contentLower.contains(lower) || tagsLower.any { it.contains(lower) }
                }
            }
            is SearchToken.HasModifier -> {
                when (token.modifier.lowercase()) {
                    "attachment" -> note.attachments.isNotEmpty()
                    else -> true
                }
            }
            is SearchToken.Or -> true
        }
    }

    /**
     * Sort notes by the given order, respecting pinned-first.
     */
    private fun sortNotes(notes: List<DecryptedNote>, order: SortOrder): List<DecryptedNote> {
        val pinned = notes.filter { it.pinned }
        val unpinned = notes.filter { !it.pinned }

        val sortedPinned = when (order) {
            SortOrder.RECENT -> pinned.sortedByDescending { it.modifiedAt }
            SortOrder.OLDEST -> pinned.sortedBy { it.modifiedAt }
            SortOrder.ALPHA -> pinned.sortedBy { it.title.lowercase() }
            SortOrder.CREATED -> pinned.sortedByDescending { it.createdAt }
        }

        val sortedUnpinned = when (order) {
            SortOrder.RECENT -> unpinned.sortedByDescending { it.modifiedAt }
            SortOrder.OLDEST -> unpinned.sortedBy { it.modifiedAt }
            SortOrder.ALPHA -> unpinned.sortedBy { it.title.lowercase() }
            SortOrder.CREATED -> unpinned.sortedByDescending { it.createdAt }
        }

        return sortedPinned + sortedUnpinned
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

    /**
     * Export selected notes to JSON.
     * TODO: Phase 5 -- implement full export with attachments and share intent.
     */
    suspend fun exportNotes(ids: Set<String>) {
        Log.i(TAG, "[Export] Export requested for ${ids.size} notes")
        // TODO: Implement export to JSON file via share intent
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
    }

    /**
     * Delete a saved search by ID.
     */
    suspend fun deleteSavedSearch(id: String) {
        val repo = savedSearchRepo ?: return
        repo.delete(id)
        _savedSearches.value = _savedSearches.value.filter { it.id != id }
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
     * Add an attachment to a note.
     */
    suspend fun addAttachment(
        noteId: String,
        data: ByteArray,
        filename: String,
        mimeType: String,
    ) {
        val noteRepository = noteRepo ?: return
        val attachmentRepository = attachmentRepo ?: return
        val key = keyManager.masterKey ?: return
        val note = noteRepository.get(id = noteId, key = key) ?: return

        // Encrypt the file data
        val encrypted = CryptoService.encrypt(data, key)
        val encryptedJSON = CryptoService.serializeEncryptedJSON(encrypted)
        val blobData = encryptedJSON.toByteArray(Charsets.UTF_8)

        // Single ID for both metadata and blob
        val attachmentId = CryptoService.generateUUID()

        // Store encrypted blob
        val record = AttachmentRecord(
            id = attachmentId,
            filename = filename,
            mimeType = mimeType,
            size = data.size,
            data = blobData,
        )
        attachmentRepository.insertOrReplace(record)

        // Encrypt the filename for storage
        val encFilename = CryptoService.encryptText(filename, key)
        val encFilenameJSON = CryptoService.serializeEncryptedJSON(encFilename)

        // Create attachment ref (data = attachmentId so blob lookup matches)
        val ref = AttachmentRef(
            id = attachmentId,
            filename = encFilenameJSON,
            mimeType = mimeType,
            size = data.size,
            data = attachmentId,
        )

        // Update note with encrypted filename in the raw record
        noteRepository.addAttachment(noteId = noteId, ref = ref)

        // Update in-memory (with plaintext filename for display)
        val currentNotes = _notes.value.toMutableList()
        val index = currentNotes.indexOfFirst { it.id == noteId }
        if (index >= 0) {
            val currentNote = currentNotes[index]
            val updatedAttachments = currentNote.attachments + AttachmentRef(
                id = attachmentId,
                filename = filename,
                mimeType = mimeType,
                size = data.size,
                data = attachmentId,
            )
            currentNotes[index] = currentNote.copy(attachments = updatedAttachments)
            _notes.value = currentNotes
        }
        scheduleDebouncedSync()
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

        // Update in-memory
        val currentNotes = _notes.value.toMutableList()
        val index = currentNotes.indexOfFirst { it.id == noteId }
        if (index >= 0) {
            val currentNote = currentNotes[index]
            currentNotes[index] = currentNote.copy(
                attachments = currentNote.attachments.filter { it.id != attachmentId }
            )
            _notes.value = currentNotes
        }
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
     * Restore a note to a previous version. Creates a snapshot of the current state first.
     */
    suspend fun restoreVersion(noteId: String, version: NoteVersionRecord) {
        val noteRepository = noteRepo ?: return
        val versionRepository = versionRepo ?: return
        val key = keyManager.masterKey ?: return

        // Snapshot current state before restoring
        val currentRecord = noteRepository.getRaw(noteId)
        if (currentRecord != null) {
            versionRepository.insertOrReplace(
                NoteVersionRecord.from(currentRecord, "pre-restore")
            )
        }

        // Decrypt the version's content and tags
        val encContent = CryptoService.parseEncryptedJSON(version.content)
        val content = CryptoService.decryptText(encContent, key)

        val encTags = CryptoService.parseEncryptedJSON(version.tags)
        val tagsText = CryptoService.decryptText(encTags, key)
        val tags: List<String> = when {
            tagsText.isEmpty() -> emptyList()
            tagsText.startsWith("[") -> {
                try {
                    json.decodeFromString(tagsText)
                } catch (_: Exception) {
                    emptyList()
                }
            }
            else -> {
                tagsText.split(",")
                    .map { it.trim() }
                    .filter { it.isNotEmpty() }
            }
        }

        // Get current decrypted note and update with restored content
        val note = noteRepository.get(id = noteId, key = key) ?: return
        val restoredNote = note.copy(
            content = content,
            tags = tags,
            syntaxLanguage = version.syntaxLanguage ?: note.syntaxLanguage,
            wordWrap = version.wordWrap ?: note.wordWrap,
            showPreview = version.showPreview ?: note.showPreview,
            color = version.color,
        )
        noteRepository.update(restoredNote, key)

        // Update in-memory
        val currentNotes = _notes.value.toMutableList()
        val index = currentNotes.indexOfFirst { it.id == noteId }
        if (index >= 0) {
            val refreshed = noteRepository.get(id = noteId, key = key)
            if (refreshed != null) {
                currentNotes[index] = refreshed
                _notes.value = currentNotes
            }
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
     * Change the vault password. Re-derives a new key from the new password with a new salt,
     * then re-encrypts all notes, attachments, versions, and the verification token.
     */
    suspend fun changePassword(currentPassword: String, newPassword: String) {
        val encRepo = encryptionRepo ?: throw AppStateError.NotInitialised
        val noteRepository = noteRepo ?: throw AppStateError.NotInitialised
        val versionRepository = versionRepo ?: throw AppStateError.NotInitialised

        val metadata = encRepo.get() ?: throw AppStateError.NoVault

        if (metadata.envelopeVersion != null) {
            // FAST PATH: envelope active — re-wrap only, no note re-encryption
            val deviceSaltB64 = metadata.deviceSalt ?: throw AppStateError.InvalidSalt
            val deviceSalt = android.util.Base64.decode(deviceSaltB64, android.util.Base64.NO_WRAP)
            val wrappedJSON = metadata.localWrappedMaster ?: throw AppStateError.InvalidSalt

            // Verify current password by unwrapping the master key
            val currentDeviceKey = CryptoService.deriveKey(currentPassword, deviceSalt, CryptoService.DEFAULT_ITERATIONS)
            val wrapped = CryptoService.parseEncryptedJSON(wrappedJSON)
            val masterKeyData: ByteArray
            try {
                masterKeyData = CryptoService.unwrapMasterKey(wrapped, currentDeviceKey)
            } catch (_: Exception) {
                throw AppStateError.WrongPassword
            }

            // Verify with token as extra check
            if (metadata.verification != null) {
                val masterKey = SecretKeySpec(masterKeyData, "AES")
                if (!verifyWithToken(metadata.verification, masterKey)) {
                    throw AppStateError.WrongPassword
                }
            }

            // Synchronous: local re-wrap (throws on failure)
            EnvelopeService.changePasswordLocal(encRepo, newPassword, masterKeyData)

            // Synchronous: update biometric key if enabled (master key unchanged)
            if (KeystoreService.hasBiometricKey(getApplication())) {
                try {
                    KeystoreService.storeBiometricKey(getApplication(), masterKeyData)
                } catch (_: Exception) { /* ignore */ }
            }

            // Background: server re-wrap (non-fatal)
            val syncRepository = syncRepo
            val client = syncClientInstance
            if (syncRepository != null && client != null) {
                val dataForServer = masterKeyData.copyOf()
                viewModelScope.launch(Dispatchers.IO) {
                    EnvelopeService.changePasswordServer(syncRepository, client, newPassword, dataForServer)
                }
            }
            return
        }

        // LEGACY PATH: full re-encryption
        val saltData = metadata.saltData ?: throw AppStateError.InvalidSalt

        // Verify current password
        val currentKey = CryptoService.deriveKey(
            password = currentPassword,
            salt = saltData,
            iterations = metadata.iterations ?: CryptoService.DEFAULT_ITERATIONS,
        )
        if (metadata.verification != null) {
            if (!verifyWithToken(metadata.verification, currentKey)) {
                throw AppStateError.WrongPassword
            }
        } else {
            if (!verifyByDecryptingNote(currentKey)) {
                throw AppStateError.WrongPassword
            }
        }

        // Generate new salt and derive new key
        val newSalt = CryptoService.generateSalt()
        val iterations = metadata.iterations ?: CryptoService.DEFAULT_ITERATIONS
        val newKey = CryptoService.deriveKey(
            password = newPassword,
            salt = newSalt,
            iterations = iterations,
        )

        // Re-encrypt all active, deleted, and archived notes
        val allNotes = noteRepository.listActive(currentKey) +
            noteRepository.listDeleted(currentKey) +
            noteRepository.listArchived(currentKey)

        for (note in allNotes) {
            val encContent = CryptoService.encryptText(note.content, newKey)
            val encTags = CryptoService.encryptStringArray(note.tags, newKey)
            val contentJSON = CryptoService.serializeEncryptedJSON(encContent)
            val tagsJSON = CryptoService.serializeEncryptedJSON(encTags)

            // Re-encrypt attachment filenames
            val reEncryptedAttachments = note.attachments.map { ref ->
                val encFilename = CryptoService.encryptText(ref.filename, newKey)
                ref.copy(filename = CryptoService.serializeEncryptedJSON(encFilename))
            }
            val attachmentsString = json.encodeToString(reEncryptedAttachments)

            val record = noteRepository.getRaw(note.id) ?: continue
            val updatedRecord = record.copy(
                content = contentJSON,
                tags = tagsJSON,
                attachments = attachmentsString,
                needsSync = true,
            )
            noteRepository.updateRaw(updatedRecord)
        }

        // Re-encrypt attachment blobs
        val attachmentRepository = attachmentRepo
        if (attachmentRepository != null) {
            val blobIds = attachmentRepository.listBlobIds()
            for (blobId in blobIds) {
                val blobData = attachmentRepository.getBlob(blobId) ?: continue
                val blobStr = String(blobData, Charsets.UTF_8)
                if (blobStr.isEmpty()) continue
                try {
                    val encrypted = CryptoService.parseEncryptedJSON(blobStr)
                    val plainData = CryptoService.decrypt(encrypted, currentKey)
                    val reEncrypted = CryptoService.encrypt(plainData, newKey)
                    val reEncryptedJSON = CryptoService.serializeEncryptedJSON(reEncrypted)
                    attachmentRepository.updateBlobData(blobId, reEncryptedJSON.toByteArray(Charsets.UTF_8))
                } catch (e: Exception) {
                    Log.w(TAG, "[ChangePassword] Skipping blob $blobId: ${e.message}")
                }
            }
        }

        // Re-encrypt version snapshots
        for (noteId in allNotes.map { it.id }.toSet()) {
            val versions = versionRepository.getVersions(noteId)
            for (ver in versions) {
                try {
                    val encVerContent = CryptoService.parseEncryptedJSON(ver.content)
                    val plainContent = CryptoService.decryptText(encVerContent, currentKey)
                    val newEncContent = CryptoService.encryptText(plainContent, newKey)

                    val encVerTags = CryptoService.parseEncryptedJSON(ver.tags)
                    val plainTagsStr = CryptoService.decryptText(encVerTags, currentKey)
                    val newEncTags = CryptoService.encryptText(plainTagsStr, newKey)

                    val updated = ver.copy(
                        content = CryptoService.serializeEncryptedJSON(newEncContent),
                        tags = CryptoService.serializeEncryptedJSON(newEncTags),
                    )
                    versionRepository.insertOrReplace(updated)
                } catch (e: Exception) {
                    Log.w(TAG, "[ChangePassword] Skipping version ${ver.versionKey}: ${e.message}")
                }
            }
        }

        // Re-encrypt saved searches
        val searchRepo = savedSearchRepo
        if (searchRepo != null) {
            val searches = searchRepo.listAll(currentKey)
            for (search in searches) {
                val encName = CryptoService.encryptText(search.name, newKey)
                val encQuery = CryptoService.encryptText(search.query, newKey)
                searchRepo.updateEncrypted(
                    id = search.id,
                    name = CryptoService.serializeEncryptedJSON(encName),
                    query = CryptoService.serializeEncryptedJSON(encQuery),
                )
            }
        }

        // Update encryption metadata with new salt and verification token
        val verificationEncrypted = CryptoService.encryptText(
            EncryptionMetadata.VERIFICATION_PLAINTEXT, newKey
        )
        val verificationJSON = CryptoService.serializeEncryptedJSON(verificationEncrypted)

        val newMetadata = metadata.copy(
            salt = android.util.Base64.encodeToString(newSalt, android.util.Base64.NO_WRAP),
            verification = verificationJSON,
        )
        encRepo.store(newMetadata)

        // Update in-memory master key
        keyManager.masterKey = newKey

        // Update biometric key if enabled
        if (KeystoreService.hasBiometricKey(getApplication())) {
            try {
                KeystoreService.storeBiometricKey(getApplication(), newKey.encoded)
            } catch (_: Exception) { /* ignore */ }
        }

        // Reload notes with new key
        loadNotes()

        // Try envelope migration after legacy password change
        if (_settings.value.syncEnabled) {
            val syncRepository = syncRepo
            val encRepo = encryptionRepo
            val client = syncClientInstance
            if (syncRepository != null && encRepo != null && client != null) {
                viewModelScope.launch(Dispatchers.IO) {
                    EnvelopeService.tryEnvelopeSetup(
                        this@AppState, syncRepository, encRepo, client, newPassword, newKey,
                    )
                }
            }
        }
    }

    // MARK: - Re-encryption

    /**
     * Re-encrypt all notes, attachments, versions, and saved searches from one
     * key to another. Used when onboarding from the server discovers a different master key.
     */
    suspend fun reEncryptAllData(oldKey: javax.crypto.SecretKey, newKey: javax.crypto.SecretKey) {
        val noteRepository = noteRepo ?: throw AppStateError.NotInitialised
        val versionRepository = versionRepo ?: throw AppStateError.NotInitialised

        val allNotes = noteRepository.listActive(oldKey) +
            noteRepository.listDeleted(oldKey) +
            noteRepository.listArchived(oldKey)

        for (note in allNotes) {
            val encContent = CryptoService.encryptText(note.content, newKey)
            val encTags = CryptoService.encryptStringArray(note.tags, newKey)
            val contentJSON = CryptoService.serializeEncryptedJSON(encContent)
            val tagsJSON = CryptoService.serializeEncryptedJSON(encTags)

            val reEncryptedAttachments = note.attachments.map { ref ->
                val encFilename = CryptoService.encryptText(ref.filename, newKey)
                ref.copy(filename = CryptoService.serializeEncryptedJSON(encFilename))
            }
            val attachmentsString = json.encodeToString(reEncryptedAttachments)

            val record = noteRepository.getRaw(note.id) ?: continue
            val updatedRecord = record.copy(
                content = contentJSON,
                tags = tagsJSON,
                attachments = attachmentsString,
                needsSync = true,
            )
            noteRepository.updateRaw(updatedRecord)
        }

        // Re-encrypt attachment blobs
        val attachmentRepository = attachmentRepo
        if (attachmentRepository != null) {
            val blobIds = attachmentRepository.listBlobIds()
            for (blobId in blobIds) {
                val blobData = attachmentRepository.getBlob(blobId) ?: continue
                val blobStr = String(blobData, Charsets.UTF_8)
                if (blobStr.isEmpty()) continue
                try {
                    val encrypted = CryptoService.parseEncryptedJSON(blobStr)
                    val plainData = CryptoService.decrypt(encrypted, oldKey)
                    val reEncrypted = CryptoService.encrypt(plainData, newKey)
                    val reEncryptedJSON = CryptoService.serializeEncryptedJSON(reEncrypted)
                    attachmentRepository.updateBlobData(blobId, reEncryptedJSON.toByteArray(Charsets.UTF_8))
                } catch (e: Exception) {
                    Log.w(TAG, "[ReEncrypt] Skipping blob $blobId: ${e.message}")
                }
            }
        }

        // Re-encrypt version snapshots
        for (noteId in allNotes.map { it.id }.toSet()) {
            val versions = versionRepository.getVersions(noteId)
            for (ver in versions) {
                try {
                    val encContent = CryptoService.parseEncryptedJSON(ver.content)
                    val plainContent = CryptoService.decryptText(encContent, oldKey)
                    val newEncContent = CryptoService.encryptText(plainContent, newKey)

                    val encTags = CryptoService.parseEncryptedJSON(ver.tags)
                    val plainTagsStr = CryptoService.decryptText(encTags, oldKey)
                    val newEncTags = CryptoService.encryptText(plainTagsStr, newKey)

                    val updated = ver.copy(
                        content = CryptoService.serializeEncryptedJSON(newEncContent),
                        tags = CryptoService.serializeEncryptedJSON(newEncTags),
                    )
                    versionRepository.insertOrReplace(updated)
                } catch (e: Exception) {
                    Log.w(TAG, "[ReEncrypt] Skipping version ${ver.versionKey}: ${e.message}")
                }
            }
        }

        // Re-encrypt saved searches
        val searchRepo = savedSearchRepo
        if (searchRepo != null) {
            val searches = searchRepo.listAll(oldKey)
            for (search in searches) {
                val encName = CryptoService.encryptText(search.name, newKey)
                val encQuery = CryptoService.encryptText(search.query, newKey)
                searchRepo.updateEncrypted(
                    id = search.id,
                    name = CryptoService.serializeEncryptedJSON(encName),
                    query = CryptoService.serializeEncryptedJSON(encQuery),
                )
            }
        }

        Log.i(TAG, "[ReEncrypt] All data re-encrypted successfully")
    }

    // MARK: - Sync (Phase 5 Stubs)

    /**
     * Attempt envelope onboarding from the server.
     * If another device has already uploaded a wrapped master key, fetch and
     * unwrap it so this device uses the same encryption key. Must be called
     * after createVault and before setupSync.
     */
    /**
     * Create vault, onboard envelope, and start sync.
     * Runs in viewModelScope so it survives Compose navigation changes.
     */
    fun unlockAndSync(encryptionPassword: String) {
        viewModelScope.launch {
            try {
                createVault(encryptionPassword)
                tryEnvelopeOnboarding(encryptionPassword)
                setupSync()
            } catch (e: Exception) {
                Log.e(TAG, "[Setup] unlockAndSync failed: ${e.message}", e)
                _syncError.value = e.message
            }
        }
    }

    suspend fun tryEnvelopeOnboarding(encryptionPassword: String) {
        Log.d(TAG, "[Envelope] tryEnvelopeOnboarding called")
        val encRepo = encryptionRepo
        if (encRepo == null) { Log.w(TAG, "[Envelope] encryptionRepo is null"); return }
        val syncRepository = syncRepo
        if (syncRepository == null) { Log.w(TAG, "[Envelope] syncRepo is null"); return }
        val currentSettings = _settings.value
        val endpoint = currentSettings.syncEndpoint
        if (endpoint.isNullOrEmpty()) { Log.w(TAG, "[Envelope] no syncEndpoint"); return }
        val context = getApplication<Application>()
        val apiKey = KeystoreService.getAPIKey(context)
        if (apiKey.isNullOrEmpty()) { Log.w(TAG, "[Envelope] no API key in Keystore"); return }
        val clientId = KeystoreService.getClientId(context)
        if (clientId.isNullOrEmpty()) { Log.w(TAG, "[Envelope] no clientId in Keystore"); return }

        val syncMeta = syncRepository.getSyncMetadata()
        val userId = syncMeta?.userId
        if (userId.isNullOrEmpty()) {
            Log.w(TAG, "[Envelope] Skipped onboarding — no userId (syncMeta=$syncMeta)")
            return
        }

        val client = SyncClient(endpoint, apiKey, clientId)
        Log.d(TAG, "[Envelope] Attempting onboard from server, userId=$userId, endpoint=$endpoint")
        try {
            val serverKey = withContext(Dispatchers.IO) {
                EnvelopeService.onboardFromServer(client, encRepo, encryptionPassword, userId)
            }
            val serverKeyData = serverKey.encoded
            keyManager.unlockWithKeyData(serverKeyData)
            Log.i(TAG, "[Envelope] Onboarded from server — master key replaced (${serverKeyData.size} bytes)")
        } catch (_: EnvelopeException) {
            Log.d(TAG, "[Envelope] No server key — this device is the first")
        } catch (e: Exception) {
            Log.e(TAG, "[Envelope] Onboarding failed: ${e.message}", e)
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
            key = key,
            scope = viewModelScope,
        )

        service.onSyncStatusChanged = { status ->
            _syncStatusMessage.value = status
        }
        service.onConflictDetected = { conflict ->
            _pendingConflicts.value = _pendingConflicts.value + conflict
        }
        service.onSyncComplete = { _ ->
            viewModelScope.launch { loadNotes() }
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
            val newCount = withContext(Dispatchers.IO) { service.fullSync() }
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
     * Register this device with a Jottery sync server.
     * Stores the returned API key and client ID, enables sync.
     */
    suspend fun registerDevice(
        serverUrl: String,
        email: String,
        password: String,
        deviceName: String,
    ): Result<String> {
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

            val context = getApplication<Application>()
            KeystoreService.setAPIKey(context, response.apiKey)
            KeystoreService.setClientId(context, response.clientId)

            // Store userId for envelope encryption
            val syncRepository = syncRepo
            if (syncRepository != null) {
                val syncMeta = syncRepository.getSyncMetadata()
                    ?: com.jottery.android.model.SyncMetadata()
                syncRepository.saveSyncMetadata(syncMeta.copy(userId = response.userId))
            }

            val newSettings = _settings.value.copy(
                syncEnabled = true,
                syncEndpoint = serverUrl,
            )
            updateSettings(newSettings)

            Result.success("Device registered successfully.")
        } catch (e: Exception) {
            Log.e(TAG, "[Sync] registerDevice failed: ${e.message}", e)
            Result.failure(e)
        }
    }

    /**
     * Import sync credentials from another device.
     * Stores the API key and client ID, enables sync.
     */
    suspend fun importCredentials(
        serverUrl: String,
        apiKey: String,
        clientId: String,
    ): Result<String> {
        return try {
            val context = getApplication<Application>()
            KeystoreService.setAPIKey(context, apiKey)
            KeystoreService.setClientId(context, clientId)

            val newSettings = _settings.value.copy(
                syncEnabled = true,
                syncEndpoint = serverUrl,
            )
            updateSettings(newSettings)
            setupSync(apiKey)

            Result.success("Credentials imported successfully.")
        } catch (e: Exception) {
            Log.e(TAG, "[Sync] importCredentials failed: ${e.message}", e)
            Result.failure(e)
        }
    }

    // MARK: - Credential Import (multi-device setup)

    /**
     * Import credentials from another device.
     * Supports both encrypted (jottery:v1:) and legacy (base64 JSON) formats.
     *
     * This is the main entry point called from SetupScreen's Import mode.
     */
    suspend fun importFromDevice(
        credentialString: String,
        password: String,
        deviceName: String,
    ): Result<String> {
        return try {
            val trimmed = credentialString.trim()

            if (trimmed.startsWith("jottery:v1:")) {
                importEncryptedCredentials(trimmed, password, deviceName)
            } else {
                importLegacyCredentials(trimmed, password, deviceName)
            }
        } catch (e: Exception) {
            Log.e(TAG, "[Import] importFromDevice failed: ${e.message}", e)
            Result.failure(e)
        }
    }

    /**
     * Handle encrypted credential format:
     * jottery:v1:<salt_b64>.<base64(JSON({"ciphertext":"...","iv":"..."}))>
     */
    private suspend fun importEncryptedCredentials(
        input: String,
        password: String,
        deviceName: String,
    ): Result<String> {
        val payload = input.removePrefix("jottery:v1:")
        val dotIndex = payload.indexOf('.')
        if (dotIndex == -1) {
            return Result.failure(Exception("Invalid credential format: missing separator"))
        }

        val saltB64 = payload.substring(0, dotIndex)
        val encryptedPayloadB64 = payload.substring(dotIndex + 1)

        // Decode salt from base64
        val salt = android.util.Base64.decode(saltB64, android.util.Base64.DEFAULT)
        if (salt.size < 32) {
            return Result.failure(Exception("Invalid credential format: salt too short"))
        }

        // Decode encrypted payload: base64 → JSON string → EncryptedData
        val payloadBytes = android.util.Base64.decode(encryptedPayloadB64, android.util.Base64.DEFAULT)
        val payloadJSON = String(payloadBytes, Charsets.UTF_8)
        val encrypted = CryptoService.parseEncryptedJSON(payloadJSON)

        // Try decrypting with common iteration counts
        // (the jottery:v1: format does not include the iteration count)
        val iterationCandidates = listOf(
            CryptoService.DEFAULT_ITERATIONS,  // 600,000
            CryptoService.MIN_ITERATIONS,      // 100,000
        )

        var decryptedJSON: String? = null
        var matchedIterations = CryptoService.DEFAULT_ITERATIONS

        for (candidate in iterationCandidates) {
            try {
                val key = withContext(Dispatchers.Default) {
                    CryptoService.deriveKey(password, salt, candidate)
                }
                decryptedJSON = CryptoService.decryptText(encrypted, key)
                matchedIterations = candidate
                break
            } catch (_: Exception) {
                // Try next iteration count
            }
        }

        if (decryptedJSON == null) {
            return Result.failure(Exception("Wrong password or corrupted credentials"))
        }

        // Parse decrypted payload: {"endpoint":"...","apiKey":"...","clientId":"..."}
        val jsonParser = Json { ignoreUnknownKeys = true }
        val creds = jsonParser.decodeFromString<CredentialPayload>(decryptedJSON)

        if (creds.endpoint.isBlank() || creds.apiKey.isBlank()) {
            return Result.failure(Exception("Invalid credential payload: missing endpoint or API key"))
        }

        val endpoint = creds.endpoint.trimEnd('/')
        return finishImport(endpoint, creds.apiKey, deviceName, password, salt, matchedIterations)
    }

    /**
     * Handle legacy credential format: base64(JSON with endpoint, apiKey, clientId, salt).
     */
    private suspend fun importLegacyCredentials(
        input: String,
        password: String,
        deviceName: String,
    ): Result<String> {
        val jsonStr = try {
            val bytes = android.util.Base64.decode(input, android.util.Base64.DEFAULT)
            String(bytes, Charsets.UTF_8)
        } catch (e: Exception) {
            return Result.failure(Exception("Invalid credential format: not valid base64"))
        }

        val jsonParser = Json { ignoreUnknownKeys = true }
        val creds = try {
            jsonParser.decodeFromString<LegacyCredentialPayload>(jsonStr)
        } catch (e: Exception) {
            return Result.failure(Exception("Invalid credential format: could not parse JSON"))
        }

        if (creds.endpoint.isBlank() || creds.apiKey.isBlank()) {
            return Result.failure(Exception("Invalid credentials: missing required fields"))
        }

        if (creds.salt.isNullOrBlank()) {
            return Result.failure(Exception("Invalid credentials: missing salt"))
        }

        val salt = android.util.Base64.decode(creds.salt, android.util.Base64.DEFAULT)
        val endpoint = creds.endpoint.trimEnd('/')

        return finishImport(endpoint, creds.apiKey, deviceName, password, salt, CryptoService.DEFAULT_ITERATIONS)
    }

    /**
     * Common finishing logic: clone device, create vault with imported salt, setup sync.
     */
    private suspend fun finishImport(
        endpoint: String,
        sourceApiKey: String,
        deviceName: String,
        password: String,
        salt: ByteArray,
        iterations: Int,
    ): Result<String> {
        // Clone device to get a fresh API key for this device
        val tempClient = SyncClient(endpoint, "", "")
        val response = withContext(Dispatchers.IO) {
            tempClient.cloneDevice(
                CloneDeviceRequest(
                    apiKey = sourceApiKey,
                    deviceName = deviceName,
                )
            )
        }

        // Store API key and client ID
        val context = getApplication<Application>()
        KeystoreService.setAPIKey(context, response.apiKey)
        KeystoreService.setClientId(context, response.clientId)

        // Store userId for envelope encryption
        syncRepo?.let { repo ->
            val syncMeta = repo.getSyncMetadata() ?: com.jottery.android.model.SyncMetadata()
            repo.saveSyncMetadata(syncMeta.copy(userId = response.userId))
        }

        // Create vault with the imported salt so the same password derives the same key
        createVault(password, existingSalt = salt, existingIterations = iterations)

        // Enable sync
        val newSettings = _settings.value.copy(
            syncEnabled = true,
            syncEndpoint = endpoint,
        )
        updateSettings(newSettings)

        // Envelope onboarding: if the server has a wrapped master key
        // (from a device using envelope encryption), fetch and unwrap it
        // so this device uses the same key. Must happen before setupSync.
        val encRepo = encryptionRepo
        val syncRepository = syncRepo
        if (encRepo != null && syncRepository != null) {
            val importClient = SyncClient(endpoint, response.apiKey, response.clientId)
            val userId = response.userId
            if (userId.isNotEmpty()) {
                try {
                    val serverKey = EnvelopeService.onboardFromServer(
                        importClient, encRepo, password, userId,
                    )
                    val serverKeyData = serverKey.encoded
                    keyManager.unlockWithKeyData(serverKeyData)
                } catch (_: EnvelopeException) {
                    Log.d(TAG, "[Envelope] No server key — this device is the first")
                } catch (e: Exception) {
                    Log.w(TAG, "[Envelope] Onboarding failed: ${e.message}")
                }
            }
        }

        setupSync(response.apiKey)

        // Trigger initial sync in the background
        viewModelScope.launch {
            try {
                _isSyncing.value = true
                _syncStatusMessage.value = "Pulling notes\u2026"
                val service = syncServiceInstance ?: return@launch
                withContext(Dispatchers.IO) { service.fullSync() }
                _isSyncing.value = false
                _lastSyncAt.value = Instant.now()
                loadNotes()
                val count = _notes.value.size
                _syncStatusMessage.value = "Synced \u2014 $count note${if (count == 1) "" else "s"}"
                delay(4_000)
                _syncStatusMessage.value = null
            } catch (e: Exception) {
                Log.e(TAG, "[Import] Initial sync failed: ${e.message}", e)
                _isSyncing.value = false
                _syncError.value = "Initial sync failed: ${e.message}"
                _syncStatusMessage.value = null
            }
        }

        return Result.success("Credentials imported successfully.")
    }

    // MARK: - Backup & Import

    /**
     * Create an encrypted backup of all notes.
     */
    suspend fun createBackup(): String {
        val nRepo = noteRepo ?: throw AppStateError.NotInitialised
        val vRepo = versionRepo ?: throw AppStateError.NotInitialised
        val aRepo = attachmentRepo ?: throw AppStateError.NotInitialised
        val service = BackupService(nRepo, vRepo, aRepo, savedSearchRepo)
        return withContext(Dispatchers.IO) { service.createBackup() }
    }

    /**
     * Restore notes from an encrypted backup.
     * Returns the number of notes restored.
     */
    suspend fun restoreBackup(backupJson: String): Int {
        val nRepo = noteRepo ?: throw AppStateError.NotInitialised
        val vRepo = versionRepo ?: throw AppStateError.NotInitialised
        val aRepo = attachmentRepo ?: throw AppStateError.NotInitialised
        val service = BackupService(nRepo, vRepo, aRepo, savedSearchRepo)
        val count = withContext(Dispatchers.IO) { service.restoreBackup(backupJson) }
        loadNotes()
        return count
    }

    /**
     * Import notes from a JSON export file.
     * Returns the number of notes imported.
     */
    suspend fun importNotes(jsonString: String): Int {
        val nRepo = noteRepo ?: throw AppStateError.NotInitialised
        val aRepo = attachmentRepo ?: throw AppStateError.NotInitialised
        val key = keyManager.masterKey ?: throw AppStateError.NotInitialised
        val service = ImportService(nRepo, aRepo)
        val count = withContext(Dispatchers.IO) { service.`import`(jsonString, key) }
        loadNotes()
        return count
    }

    /**
     * Resolve a sync conflict for a specific note.
     */
    suspend fun resolveConflict(noteId: String, strategy: ConflictResolutionStrategy) {
        Log.d(TAG, "[Sync] resolveConflict: noteId=$noteId, strategy=$strategy")

        val conflict = _pendingConflicts.value.firstOrNull { it.id == noteId } ?: return
        val service = syncServiceInstance

        if (service != null) {
            try {
                withContext(Dispatchers.IO) {
                    service.resolveConflict(conflict, strategy)
                }
            } catch (e: Exception) {
                Log.e(TAG, "[Sync] resolveConflict failed: ${e.message}", e)
            }
        }

        _pendingConflicts.value = _pendingConflicts.value.filter { it.id != noteId }
        loadNotes()
        triggerSync()
    }

    // MARK: - Inbox (Phase 5 Stubs)

    /**
     * Load inbox items from the server.
     *
     * TODO: Phase 5 -- fetch from SyncClient.
     */
    suspend fun loadInboxItems() {
        // TODO: Phase 5 -- syncClient.listInboxItems()
        Log.d(TAG, "[Sync] loadInboxItems: stub")
    }

    /**
     * Accept an inbox item -- create a local note from it and delete from server.
     *
     * TODO: Phase 5 -- create note from item, delete from server.
     */
    suspend fun acceptInboxItem(item: InboxItem) {
        val repo = noteRepo ?: return
        val key = keyManager.masterKey ?: return

        // Create a new encrypted note from the inbox item
        val note = repo.create(content = item.content, tags = item.tags, key = key)
        _notes.value = listOf(note) + _notes.value

        // TODO: Phase 5 -- syncClient.deleteInboxItem(id = item.id)
        _inboxItems.value = _inboxItems.value.filter { it.id != item.id }
    }

    /**
     * Delete a single inbox item from the server.
     *
     * TODO: Phase 5 -- delete from SyncClient.
     */
    suspend fun deleteInboxItem(item: InboxItem) {
        // TODO: Phase 5 -- syncClient.deleteInboxItem(id = item.id)
        _inboxItems.value = _inboxItems.value.filter { it.id != item.id }
    }

    /**
     * Delete all inbox items from the server.
     *
     * TODO: Phase 5 -- delete all from SyncClient.
     */
    suspend fun deleteAllInboxItems() {
        // TODO: Phase 5 -- syncClient.deleteAllInboxItems()
        _inboxItems.value = emptyList()
    }

    // MARK: - Lifecycle

    /**
     * Handle foreground/background transitions.
     * Call with started=true when app comes to foreground, false when going to background.
     */
    fun handleLifecycleEvent(started: Boolean) {
        if (!started) {
            // Going to background -- record timestamp for auto-lock
            if (backgroundedAt == null) {
                backgroundedAt = Instant.now()
            }
            sseClientInstance?.stop()
        } else {
            // Coming to foreground
            if (_isLocked.value) return

            // Check auto-lock (timeout 0 means "never")
            val bg = backgroundedAt
            if (bg != null && keyManager.autoLockTimeout > 0) {
                val elapsedSeconds = java.time.Duration.between(bg, Instant.now()).seconds
                if (elapsedSeconds >= keyManager.autoLockTimeout) {
                    lock()
                    backgroundedAt = null
                    return
                }
            }
            val wasBackgrounded = backgroundedAt != null
            backgroundedAt = null
            keyManager.recordActivity()

            // Restart SSE and sync when returning from genuine background
            if (wasBackgrounded && _syncEnabled.value) {
                viewModelScope.launch {
                    try { syncServiceInstance?.startSSE() } catch (_: Exception) {}
                    triggerSync()
                }
            }
        }
    }

    companion object {
        private const val TAG = "AppState"
        private const val PREFS_NAME = "jottery_prefs"
        private const val KEY_EDITOR_FONT_SCALE = "editorFontScale"
    }
}

// MARK: - Search Token Types

private sealed class SearchToken {
    data class Tag(val name: String) : SearchToken()
    data class Negation(val term: String) : SearchToken()
    data class Phrase(val text: String) : SearchToken()
    data class Word(val text: String) : SearchToken()
    data class HasModifier(val modifier: String) : SearchToken()
    data object Or : SearchToken()
}

// MARK: - Error Types

sealed class AppStateError(message: String) : Exception(message) {
    data object NotInitialised : AppStateError("Application not initialised")
    data object NoVault : AppStateError("No vault found")
    data object InvalidSalt : AppStateError("Invalid encryption salt")
    data object WrongPassword : AppStateError("Incorrect password")
}
