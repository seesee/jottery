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
import com.jottery.android.model.SortOrder
import com.jottery.android.model.UserSettings
import com.jottery.android.util.DateUtils
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

    private var searchJob: Job? = null
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
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Database init failed: ${e.message}")
            _isFirstLaunch.value = true
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

        val saltData = metadata.saltData ?: throw AppStateError.InvalidSalt

        val key = keyManager.unlock(
            password = password,
            salt = saltData,
            iterations = metadata.iterations,
        )

        // Verify password
        val verified = if (metadata.verification != null) {
            verifyWithToken(metadata.verification, key)
        } else {
            // No token (old vault) -- try decrypting a note instead
            verifyByDecryptingNote(key)
        }

        if (!verified) {
            keyManager.lock()
            _isLocked.value = true
            throw AppStateError.WrongPassword
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

        // TODO: Phase 5 -- stop SSE and release sync objects
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
        _notes.value = repo.listActive(key)
        _archivedNotes.value = repo.listArchived(key)
        _savedSearches.value = savedSearchRepo?.listActive(key) ?: emptyList()
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
        }
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
    }

    /**
     * Restore a soft-deleted note.
     */
    suspend fun restoreNote(id: String) {
        val repo = noteRepo ?: return
        repo.restore(id)
        loadNotes()
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
        }
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
        loadArchivedNotes()
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

        // Delete database file
        context.deleteDatabase("jottery.db")

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
        val saltData = metadata.saltData ?: throw AppStateError.InvalidSalt

        // Verify current password
        val currentKey = CryptoService.deriveKey(
            password = currentPassword,
            salt = saltData,
            iterations = metadata.iterations,
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
        val iterations = metadata.iterations
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
    }

    // MARK: - Sync (Phase 5 Stubs)

    /**
     * Set up the sync client. Pass apiKey directly after registration
     * to avoid Keystore retrieval timing issues; otherwise reads from Keystore.
     *
     * TODO: Phase 5 -- implement full sync service.
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

        Log.d(TAG, "[Sync] setupSync: OK -- endpoint=$endpoint")
        _syncEnabled.value = true
        _syncError.value = null

        // TODO: Phase 5 -- create SyncClient + SyncService, wire post-sync handler,
        //   start SSE, run initial sync, start network monitor.
    }

    /**
     * Trigger a sync cycle (push then pull).
     *
     * TODO: Phase 5 -- implement full sync cycle.
     */
    suspend fun triggerSync() {
        if (!_syncEnabled.value) {
            Log.d(TAG, "[Sync] triggerSync: sync not enabled -- aborting")
            return
        }
        Log.d(TAG, "[Sync] triggerSync: starting sync cycle")
        _isSyncing.value = true
        _syncError.value = null
        _syncStatusMessage.value = "Pushing changes..."

        // TODO: Phase 5 -- push, pull, finalise, reload notes, update conflicts.
        _isSyncing.value = false
        _syncStatusMessage.value = "Synced -- up to date"

        viewModelScope.launch {
            delay(4_000)
            if (_syncStatusMessage.value == "Synced -- up to date") {
                _syncStatusMessage.value = null
            }
        }
    }

    /**
     * Force a full sync -- re-downloads all notes from the server.
     *
     * TODO: Phase 5 -- implement full sync reset and re-download.
     */
    suspend fun forceFullSync() {
        if (!_syncEnabled.value) return
        _isSyncing.value = true
        _syncError.value = null
        _syncStatusMessage.value = "Full sync in progress..."

        // TODO: Phase 5 -- force full sync, reload notes, update conflicts.
        _isSyncing.value = false
        _syncStatusMessage.value = "Full sync complete"

        viewModelScope.launch {
            delay(4_000)
            if (_syncStatusMessage.value == "Full sync complete") {
                _syncStatusMessage.value = null
            }
        }
    }

    /**
     * Resolve a sync conflict for a specific note.
     *
     * TODO: Phase 5 -- delegate to SyncService.resolveConflict, reload notes, trigger sync.
     */
    suspend fun resolveConflict(noteId: String, strategy: ConflictResolutionStrategy) {
        Log.d(TAG, "[Sync] resolveConflict: noteId=$noteId, strategy=$strategy")
        // TODO: Phase 5 -- resolve via SyncService, update pendingConflicts, loadNotes, triggerSync.
        _pendingConflicts.value = _pendingConflicts.value.filter { it.id != noteId }
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
            // TODO: Phase 5 -- stop SSE
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

            // Only restart SSE and sync when returning from genuine background
            if (wasBackgrounded && _syncEnabled.value) {
                viewModelScope.launch {
                    // TODO: Phase 5 -- restart SSE, trigger sync
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
