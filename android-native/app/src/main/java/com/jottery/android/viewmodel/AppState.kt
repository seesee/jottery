package com.jottery.android.viewmodel

import android.app.Application
import android.util.Log
import com.jottery.android.R
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import androidx.room.withTransaction
import com.jottery.android.crypto.CryptoService
import com.jottery.android.crypto.KeyManager
import com.jottery.android.crypto.KeyWipedException
import com.jottery.android.crypto.KeystoreService
import com.jottery.android.database.JotteryDatabase
import com.jottery.android.database.repository.AttachmentRepository
import com.jottery.android.database.repository.EncryptionRepository
import com.jottery.android.database.repository.NoteRepository
import com.jottery.android.database.repository.SavedSearchRepository
import com.jottery.android.database.repository.SettingsRepository
import com.jottery.android.database.repository.SyncRepository
import com.jottery.android.database.repository.VersionRepository
import com.jottery.android.model.AccountCredentials
import com.jottery.android.model.AttachmentRecord
import com.jottery.android.model.AttachmentRef
import com.jottery.android.model.ConflictInfo
import com.jottery.android.model.ConflictResolutionStrategy
import com.jottery.android.model.DecryptedNote
import com.jottery.android.model.DecryptedSavedSearch
import com.jottery.android.model.EncryptionMetadata
import com.jottery.android.model.InboxItem
import com.jottery.android.model.NoteCategories
import com.jottery.android.model.NoteRecord
import com.jottery.android.model.NoteVersionRecord
import com.jottery.android.model.CloneDeviceRequest
import com.jottery.android.model.CredentialPayload
import com.jottery.android.model.LegacyCredentialPayload
import com.jottery.android.model.RegisterDeviceRequest
import com.jottery.android.model.SortOrder
import com.jottery.android.model.UserAccountInfo
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
import com.jottery.android.service.AccountKeyService
import com.jottery.android.service.EnvelopeException
import com.jottery.android.service.EnvelopeService
import com.jottery.android.service.VaultReKeyService
import com.jottery.android.service.ExportService
import com.jottery.android.service.ImportService
import com.jottery.android.service.ImportStrategy
import com.jottery.android.service.BackupReminder
import com.jottery.android.service.BlobRepair
import com.jottery.android.service.NotePurger
import com.jottery.android.service.VaultHealthService
import com.jottery.android.service.SearchService
import com.jottery.android.service.SyncPhase
import com.jottery.android.service.SyncRetryPolicy
import com.jottery.android.service.SyncValidator
import com.jottery.android.service.SyncService
import javax.crypto.spec.SecretKeySpec
import com.jottery.android.util.AttachmentFiles
import com.jottery.android.util.BiometricHelper
import com.jottery.android.util.ConnectivityMonitor
import com.jottery.android.util.DateUtils
import com.jottery.android.util.NotePrinter
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.coroutines.Job
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.delay
import com.jottery.android.util.KeyboardCommand
import com.jottery.android.util.QuickAction
import com.jottery.android.util.SearchSuggestions
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
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

    /**
     * Blank the plaintext filenames earlier builds stored, a few rows at a time. Each
     * row's attachment data is rewritten with it, so this keeps out of the way of
     * everything else, and picks up at the next unlock if it doesn't finish.
     */
    private suspend fun clearOldFilenames() {
        val repository = attachmentRepo ?: return
        try {
            while (repository.clearOldFilenames(limit = 5) > 0) delay(100)
        } catch (e: Exception) {
            Log.w(TAG, "[Filenames] Failed: ${e.javaClass.simpleName}")
        }
    }

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

    // MARK: - New Notes

    private val _defaultNoteType = MutableStateFlow(
        application.getSharedPreferences(PREFS_NAME, 0)
            .getString(KEY_DEFAULT_NOTE_TYPE, DEFAULT_NOTE_TYPE) ?: DEFAULT_NOTE_TYPE
    )
    /** Per device, like the web's, and read for every note made here. */
    val defaultNoteType: StateFlow<String> = _defaultNoteType.asStateFlow()

    fun setDefaultNoteType(type: String) {
        _defaultNoteType.value = type
        getApplication<Application>()
            .getSharedPreferences(PREFS_NAME, 0)
            .edit()
            .putString(KEY_DEFAULT_NOTE_TYPE, type)
            .apply()
    }

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

    /** A note just deleted by swipe, which the user can still undo. */
    private val _undoableDelete = MutableStateFlow<String?>(null)
    val undoableDelete: StateFlow<String?> = _undoableDelete.asStateFlow()

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
    private var syncRetryJob: Job? = null
    private var periodicSyncJob: Job? = null
    private var syncRetryAttempt = 0
    private var connectivityMonitor: ConnectivityMonitor? = null
    private var isBackgrounded = false

    /** The user hears once per session that a password change hasn't reached the server. */
    private var rewrapWarningShown = false

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
        get() = resolveSelectedNote(_selectedNoteId.value, _notes.value, _archivedNotes.value)

    /** All unique tags across all notes, sorted alphabetically. */
    val allTags: List<String>
        get() {
            val tagSets = _notes.value.flatMap { it.tags } + _archivedNotes.value.flatMap { it.tags }
            return tagSets.toSet().sorted()
        }

    // MARK: - Initialisation

    init {
        initialise()
        viewModelScope.launch {
            _isLocked.collect { locked ->
                if (!locked) {
                    applyPendingShare()
                    applyPendingQuickAction()
                }
            }
        }
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
                    Log.e(TAG, "Initialisation error: ${e.javaClass.simpleName}")
                    _isFirstLaunch.value = true
                } finally {
                    _isInitialised.value = true
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Database init failed: ${e.javaClass.simpleName}")
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

            key = keyManager.unlockWithKeyData(masterKeyData)
            masterKeyData.fill(0)

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

        finishUnlock(key, metadata, password)
        offerBiometricIfUseful()
        return true
    }

    /**
     * Everything after the key is proved. A failure here is never a wrong password,
     * so nothing here may be mistaken for one or leave the key loaded on the lock
     * screen. Sync is optional: its problems are reported and the vault still opens.
     *
     * @param notesPassword null for an unlock that doesn't use it, such as a
     *   biometric one, which therefore can't reconcile the account's wrapped key.
     */
    private suspend fun finishUnlock(key: SecretKey, metadata: EncryptionMetadata, notesPassword: String?) {
        try {
            loadNotes()
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: Exception) {
            keyManager.lock()
            throw e
        }

        // Auto-purge deleted notes older than 30 days
        try {
            val purged = purger()?.purgeExpired() ?: 0
            if (purged > 0) {
                Log.i(TAG, "[Purge] Removed $purged deleted note(s) older than 30 days")
            }
        } catch (e: Exception) {
            Log.w(TAG, "[Purge] Failed: ${e.javaClass.simpleName}")
        }

        viewModelScope.launch(Dispatchers.IO) { clearOldFilenames() }

        Log.d(TAG, "[Sync] unlock: calling setupSync()")
        try {
            setupSync()
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: Exception) {
            Log.w(TAG, "[Sync] Setup failed: ${e.javaClass.simpleName}")
            _syncError.value = text(R.string.sync_error_setup)
        }

        // Upgrade old vaults: store a verification token if missing. Only a key that
        // has proved itself may be written down as the right one, or a mistyped
        // password on an empty vault would lock out the real one for good.
        if (metadata.verification == null) {
            try {
                val proved = metadata.envelopeVersion != null || opensANote(noteRepo?.listAllRaw().orEmpty(), key)
                if (proved) upgradeVaultVerification(key)
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
        if (notesPassword != null && _settings.value.syncEnabled) {
            viewModelScope.launch { reconcileAccountKey(notesPassword) }
        }

        // Last: this leaves the unlock screen, which cancels the caller's scope
        _isLocked.value = false
    }

    // MARK: - Biometric Unlock

    private val _biometricEnabled = MutableStateFlow(KeystoreService.hasBiometricKey(application))
    val biometricEnabled: StateFlow<Boolean> = _biometricEnabled.asStateFlow()

    /** Asked once, after a password unlock on a device that could use it. */
    private val _offerBiometric = MutableStateFlow(false)
    val offerBiometric: StateFlow<Boolean> = _offerBiometric.asStateFlow()

    fun dismissBiometricOffer() {
        _offerBiometric.value = false
        getApplication<Application>().getSharedPreferences(PREFS_NAME, 0)
            .edit().putBoolean(KEY_BIOMETRIC_OFFERED, true).apply()
    }

    private fun offerBiometricIfUseful() {
        val app = getApplication<Application>()
        val offered = app.getSharedPreferences(PREFS_NAME, 0).getBoolean(KEY_BIOMETRIC_OFFERED, false)
        if (!offered && !_biometricEnabled.value && BiometricHelper.isBiometricAvailable(app)) {
            _offerBiometric.value = true
        }
    }

    /** Store the open vault's key behind a biometric. [cipher] came through BiometricPrompt. */
    fun enableBiometric(cipher: javax.crypto.Cipher) {
        val key = keyManager.masterKey ?: return
        val bytes = key.encoded
        try {
            KeystoreService.storeBiometricKey(getApplication(), cipher, bytes)
            _biometricEnabled.value = true
        } finally {
            bytes.fill(0)
        }
    }

    fun disableBiometric() {
        KeystoreService.deleteBiometricKey(getApplication())
        _biometricEnabled.value = false
    }

    /**
     * The cipher to unlock with, or null if there's no usable key. Null after
     * enrolments changed, in which case the stored key has gone with the Keystore's.
     */
    fun biometricUnlockCipher(): javax.crypto.Cipher? {
        val cipher = try {
            KeystoreService.getBiometricDecryptCipher(getApplication())
        } catch (e: Exception) {
            Log.w(TAG, "[Biometric] Key unavailable: ${e.javaClass.simpleName}")
            null
        }
        _biometricEnabled.value = KeystoreService.hasBiometricKey(getApplication())
        return cipher
    }

    /** Unlock with a cipher BiometricPrompt has authorised. */
    suspend fun unlockWithBiometric(cipher: javax.crypto.Cipher) {
        val repo = encryptionRepo ?: throw AppStateError.NotInitialised
        val metadata = repo.get() ?: throw AppStateError.NoVault
        val bytes = KeystoreService.decryptBiometricKey(getApplication(), cipher)
            ?: throw AppStateError.BiometricUnavailable
        val key = keyManager.unlockWithKeyData(bytes)
        bytes.fill(0)

        // A key from before the vault moved to another can't be used
        if (metadata.verification != null && !verifyWithToken(metadata.verification, key)) {
            keyManager.lock()
            disableBiometric()
            throw AppStateError.BiometricUnavailable
        }
        finishUnlock(key, metadata, notesPassword = null)
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
        val records = noteRepo?.listAllRaw() ?: return false
        return records.isEmpty() || opensANote(records, key)
    }

    /**
     * AES-GCM can't decrypt under the wrong key, so one note opening proves the key.
     * It takes only one: a damaged note mustn't lock out the right password. This reads
     * rows directly, because listActive skips what it can't read, and so proves nothing.
     */
    private fun opensANote(records: List<NoteRecord>, key: SecretKey): Boolean = records.any { record ->
        try {
            CryptoService.decryptText(CryptoService.parseEncryptedJSON(record.content), key)
            true
        } catch (_: Exception) {
            false
        }
    }

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

        // The key leaves KeyManager now, so nothing new can start with it, but is
        // kept until the last of the typing has been written.
        val pending = _pendingEditorNote.value
        _pendingEditorNote.value = null
        val key = keyManager.release()
        val repo = noteRepo

        stopSync()
        searchJob?.cancel()
        syncDebounceJob?.cancel()

        AttachmentFiles.clear(getApplication())
        NotePrinter.release()
        _isLocked.value = true
        _notes.value = emptyList()
        _archivedNotes.value = emptyList()
        _filteredNotes.value = emptyList()
        _recentSearches.value = emptyList()
        _otherViewMatches.value = 0
        _savedSearches.value = emptyList()
        _pendingConflicts.value = emptyList()
        _inboxItems.value = emptyList()
        _selectedNoteId.value = null
        _searchQuery.value = ""
        _userMessage.value = null

        viewModelScope.launch(NonCancellable) {
            try {
                if (pending != null && key != null) repo?.update(pending, key)
            } catch (e: Exception) {
                Log.w(TAG, "[Lock] Couldn't save the last edit: ${e.javaClass.simpleName}")
            } finally {
                key?.destroy()
            }
        }
    }

    /** Stop everything sync runs, leaving what's stored as it is. */
    private fun stopSync() {
        sseClientInstance?.stop()
        syncClientInstance = null
        syncServiceInstance = null
        sseClientInstance = null
        syncRetryJob?.cancel()
        periodicSyncJob?.cancel()
        connectivityMonitor?.stop()
        connectivityMonitor = null
        syncRetryAttempt = 0
        _syncEnabled.value = false
    }

    /** The activity is finishing for good, so anything running goes with it. */
    override fun onCleared() {
        val pending = _pendingEditorNote.value
        val key = keyManager.release()
        stopSync()
        try {
            // viewModelScope is about to go, and this is the user's last keystrokes
            if (pending != null && key != null) {
                kotlinx.coroutines.runBlocking { noteRepo?.update(pending, key) }
            }
        } catch (e: Exception) {
            Log.w(TAG, "[Lock] Couldn't save the last edit: ${e.javaClass.simpleName}")
        } finally {
            key?.destroy()
        }
        super.onCleared()
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
            _syncError.value = getApplication<Application>().resources.getQuantityString(
                R.plurals.notes_all_unreadable, result.failedCount, result.failedCount,
            )
        } else if (result.failedCount > 0) {
            _syncError.value = text(R.string.notes_some_unreadable, result.failedCount, result.totalRecords)
        }

        scheduleSearch()
    }

    /**
     * Create a new note with the given content and tags.
     * Returns the created DecryptedNote, or null on failure.
     */
    suspend fun createNote(
        content: String = "",
        tags: List<String> = emptyList(),
        syntaxLanguage: String = _defaultNoteType.value,
    ): DecryptedNote? {
        val repo = noteRepo ?: return null
        val key = keyManager.masterKey ?: return null
        val note = repo.create(content = content, tags = tags, key = key, syntaxLanguage = syntaxLanguage)
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
        try {
            if (!repo.update(note, key)) return
        } catch (_: KeyWipedException) {
            // Locked mid-save. The lock wrote this same text from the pending note.
            return
        }
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
        // What's on screen is what the user means to copy
        flushPendingEditor()
        val original = repo.get(id = id, key = key) ?: return
        val created = repo.create(
            content = original.content,
            tags = original.tags,
            key = key,
            syntaxLanguage = original.syntaxLanguage,
            wordWrap = original.wordWrap,
            showPreview = original.showPreview,
            color = original.color,
        )
        // Both notes point at the same blobs, which stay until nothing refers to them
        val refs = repo.rawAttachmentRefs(id)
        if (refs.isNotEmpty()) repo.setAttachments(created.id, refs, key)
        val duplicate = repo.get(id = created.id, key = key) ?: created
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
     * Delete a note from a gesture, which is easy to make by accident. Pinned and
     * locked notes are protected: returns false, changing nothing, for those.
     */
    suspend fun deleteNoteWithUndo(note: DecryptedNote): Boolean {
        if (note.pinned || note.locked) return false
        deleteNote(note.id)
        _undoableDelete.value = note.id
        return true
    }

    fun clearUndoableDelete() {
        _undoableDelete.value = null
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
        val categories = NoteCategories.from(_settings.value)
        val otherView = if (_showArchive.value) _notes.value else _archivedNotes.value

        searchJob = viewModelScope.launch {
            // Debounce while typing. With no query there's nothing to wait for.
            if (query.isNotBlank()) delay(150)

            _filteredNotes.value = withContext(Dispatchers.Default) {
                val matches = if (query.isBlank()) source else SearchService.filter(source, query, categories).notes
                SearchService.sort(matches, order)
            }
            // An archive search already covers both views
            _otherViewMatches.value = if (query.isBlank() || source.size > displayedNotes.size) {
                0
            } else {
                withContext(Dispatchers.Default) { SearchService.filter(otherView, query, categories).matchCount }
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

    /** Typing not yet saved, which a lock or a sync must write first. */
    fun setPendingEditorNote(note: DecryptedNote?) {
        _pendingEditorNote.value = note
    }

    /** The editor has saved [note], unless it has been typing since. */
    fun clearPendingEditorNote(note: DecryptedNote) {
        if (_pendingEditorNote.value === note) _pendingEditorNote.value = null
    }

    /**
     * Write unsaved typing so a pull can see it. Otherwise a remote change would
     * land in the row first, and the save after it would silently replace that
     * change rather than raise the conflict it is.
     */
    private suspend fun flushPendingEditor() {
        val pending = _pendingEditorNote.value ?: return
        saveNote(pending)
        clearPendingEditorNote(pending)
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
     * Permanently delete notes: no recovery here, and a tombstone for every other
     * device. Everything that went with them goes too.
     */
    suspend fun permanentlyDeleteNotes(ids: Collection<String>) {
        val service = purger() ?: return
        service.deletePermanently(ids)
        scheduleDebouncedSync()
    }

    private fun purger(): NotePurger? {
        val database = db ?: return null
        return NotePurger(
            noteRepo = noteRepo ?: return null,
            versionRepo = versionRepo ?: return null,
            attachmentRepo = attachmentRepo ?: return null,
            syncRepo = syncRepo ?: return null,
            transaction = { block -> database.withTransaction { block() } },
        )
    }

    // MARK: - Saved Searches

    /**
     * Save a new search query with a name.
     */
    suspend fun saveSearch(name: String, query: String) {
        val repo = savedSearchRepo ?: return
        val key = keyManager.masterKey ?: return
        val order = (_savedSearches.value.maxOfOrNull { it.displayOrder } ?: -1) + 1
        val search = repo.create(name = name, query = query, key = key, order = order)
        _savedSearches.value = _savedSearches.value + search
        scheduleDebouncedSync()
    }

    /** Give a saved search a new name. */
    suspend fun renameSavedSearch(id: String, name: String) {
        val repo = savedSearchRepo ?: return
        val key = keyManager.masterKey ?: return
        repo.rename(id, name, key)
        _savedSearches.value = _savedSearches.value.map { if (it.id == id) it.copy(name = name) else it }
        scheduleDebouncedSync()
    }

    /** Move a saved search up or down the list. */
    suspend fun moveSavedSearch(id: String, offset: Int) {
        val repo = savedSearchRepo ?: return
        val searches = _savedSearches.value.toMutableList()
        val from = searches.indexOfFirst { it.id == id }
        val to = from + offset
        if (from < 0 || to !in searches.indices) return
        searches.add(to, searches.removeAt(from))
        repo.setOrder(searches.map { it.id })
        _savedSearches.value = searches.mapIndexed { index, search -> search.copy(displayOrder = index) }
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
        noteRepository.addAttachment(noteId = noteId, ref = ref, key = key)

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
        noteRepository.removeAttachment(noteId = noteId, attachmentId = attachmentId, key = key)

        // The blob goes only if nothing else needs it: restoring a version might
        try {
            purger()?.deleteUnreferencedBlobs(setOf(ref.data))
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
            Log.w("AppState", "Failed to resolve attachment $attachmentId: ${e.javaClass.simpleName}")
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
            Log.w(TAG, "Failed to decrypt version content: ${e.javaClass.simpleName}")
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

    /** Name a colour, or clear its name with a blank one. */
    suspend fun setCategoryName(colour: String, name: String) = changeCategories { it.named(colour, name) }

    /** Give a tag a colour, or take it away with null. */
    suspend fun setTagColour(tag: String, colour: String?) = changeCategories { it.withTag(tag, colour) }

    private suspend fun changeCategories(change: (NoteCategories) -> NoteCategories) {
        val settings = _settings.value
        updateSettings(change(NoteCategories.from(settings)).appliedTo(settings))
        scheduleSearch()
    }

    // MARK: - Wipe & Re-onboard

    /**
     * Delete everything -- database, Keystore, in-memory state -- so the user
     * can start fresh as if the app was just installed.
     */
    fun wipeAllData() {
        val context = getApplication<Application>()
        BackupReminder.forget(context)

        // Clear secure storage. Failing here can't be allowed to stop a wipe.
        try {
            KeystoreService.deleteAPIKey(context)
            KeystoreService.deleteClientId(context)
        } catch (e: Exception) {
            Log.w(TAG, "[Wipe] Secure preferences: ${e.javaClass.simpleName}")
        }
        KeystoreService.deleteBiometricKey(context)

        // Close and clear the database singleton before deleting the file
        JotteryDatabase.destroyInstance()

        // Delete database file
        context.deleteDatabase("jottery.db")

        // Stop sync and SSE
        stopSync()

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
        _recentSearches.value = emptyList()
        _otherViewMatches.value = 0
        _savedSearches.value = emptyList()
        _pendingConflicts.value = emptyList()
        _inboxItems.value = emptyList()
        _showArchive.value = false
        _selectedNoteId.value = null
        _searchQuery.value = ""
        _biometricEnabled.value = false
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
        val database = db ?: throw AppStateError.NotInitialised
        val metadata = encRepo.get() ?: throw AppStateError.NoVault

        val masterKeyData = unwrapVaultKey(metadata, currentPassword)
            ?: throw AppStateError.WrongPassword

        // The account's blob has to follow the new password. Only this device can
        // tell that the server's copy is now out of date, so the rewrap is written
        // with the password change and kept until the server has it.
        val userId = syncRepo?.getSyncMetadata()?.userId
        val rewrapped = if (userId.isNullOrEmpty()) null else {
            EnvelopeService.wrapForAccount(userId, newPassword, masterKeyData)
        }
        val envelope = EnvelopeService.localEnvelope(masterKeyData, newPassword)
        database.withTransaction {
            encRepo.store(EnvelopeService.metadataFor(envelope, metadata))
            if (rewrapped != null) syncRepo?.setPendingWrappedKey(rewrapped)
        }

        // Biometric unlock is unaffected: it holds the master key, which hasn't changed

        if (rewrapped != null) uploadPendingRewrap()
    }

    /** Sends any rewrap still owed to the server, telling the user if it has to wait. */
    private suspend fun uploadPendingRewrap() {
        val service = accountKeyService() ?: return
        if (!service.uploadPending() && !rewrapWarningShown) {
            rewrapWarningShown = true
            showMessage(text(R.string.account_key_update_pending))
        }
    }

    private fun accountKeyService(): AccountKeyService? {
        val client = syncClientInstance ?: return null
        return AccountKeyService(client, syncRepo ?: return null, encryptionRepo ?: return null)
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

    /**
     * For a password that already exists, which can't be refused: that would stop
     * these users adding a device. They should still know, and change it.
     */
    private fun warnIfMatchingPasswords(notesPassword: String, accountPassword: String?) {
        if (accountPassword != null && notesPassword == accountPassword) {
            showMessage(text(R.string.password_matches_account_warning))
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

    /**
     * Sync starts only here, once this device holds the account's key.
     *
     * @param openedBlob this device opened or uploaded the account's wrapped key
     *   while joining. False only when the account has none yet.
     */
    private suspend fun finishConnection(connection: PendingConnection, openedBlob: Boolean = true) {
        val context = getApplication<Application>()
        KeystoreService.setAPIKey(context, connection.apiKey)
        KeystoreService.setClientId(context, connection.clientId)

        syncRepo?.let { repo ->
            val syncMeta = repo.getSyncMetadata() ?: com.jottery.android.model.SyncMetadata()
            // What this device knew about any earlier connection doesn't carry over
            repo.saveSyncMetadata(
                syncMeta.copy(
                    userId = connection.userId,
                    lastSyncAt = null,
                    pullCursor = null,
                    cursorAdopted = false,
                    wrappedKeyOpened = openedBlob,
                    pendingWrappedKey = null,
                )
            )
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
            val client = SyncClient(connection.endpoint, connection.apiKey, connection.clientId)
            var master = EnvelopeService.fetchServerKey(client, connection.userId, notesPassword)
            if (master == null) {
                // First device on the account: its key becomes the account's, and
                // is published before anything else, so no second device can
                // start on a key of its own in the meantime.
                refuseMatchingPasswords(notesPassword, connection.accountPassword)
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
                master = try {
                    CryptoService.generateSalt().also {
                        EnvelopeService.publishKey(
                            client, connection.userId, notesPassword, it, EnvelopeService.KeyWrite.FirstUpload,
                        )
                    }
                } catch (e: SyncClientException) {
                    if (e.statusCode != HTTP_CONFLICT) throw e
                    // Another device got there first, so its key is the account's
                    EnvelopeService.fetchServerKey(client, connection.userId, notesPassword) ?: throw e
                }
            } else {
                warnIfMatchingPasswords(notesPassword, connection.accountPassword)
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
            // This vault's password already exists, so it can't be refused here
            warnIfMatchingPasswords(notesPassword, connection.accountPassword)
            joinAccount(connection, notesPassword)
        }
    }

    private suspend fun joinAccount(
        connection: PendingConnection,
        notesPassword: String,
        readAgain: Boolean = true,
    ) {
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
                try {
                    EnvelopeService.publishKey(
                        client, connection.userId, notesPassword, localKey, EnvelopeService.KeyWrite.FirstUpload,
                    )
                } catch (e: SyncClientException) {
                    // Another device got there first: compare with what it wrote
                    if (e.statusCode == HTTP_CONFLICT && readAgain) {
                        joinAccount(connection, notesPassword, readAgain = false)
                        return
                    }
                    revokeQuietly(connection)
                    throw e
                }
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
        // The stored key is the old one now
        disableBiometric()
        loadNotes()
    }

    /**
     * Bring the vault and the account's wrapped key into line after an unlock. See
     * [EnvelopeService.reconcile]. Any failure leaves things as they are, and the
     * next unlock tries again.
     */
    private suspend fun reconcileAccountKey(notesPassword: String) {
        val service = accountKeyService() ?: return
        val localKey = keyManager.masterKey ?: return

        try {
            val outcome = service.settle(notesPassword, localKey.encoded) {
                syncRepo?.getSyncMetadata()?.lastSyncAt != null &&
                    noteRepo?.allNotesOpenWith(localKey) == true
            }
            when (outcome) {
                AccountKeyService.Outcome.InStep -> Unit
                is AccountKeyService.Outcome.MoveToAccountKey -> {
                    switchVaultToMasterKey(outcome.accountKey, notesPassword)
                    triggerSync()
                }
                AccountKeyService.Outcome.Diverged -> showMessage(text(R.string.connect_key_diverged))
                AccountKeyService.Outcome.PasswordChangedElsewhere ->
                    showMessage(text(R.string.account_password_changed_elsewhere))
                AccountKeyService.Outcome.CannotHeal -> showMessage(text(R.string.account_key_cannot_heal))
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
            _syncError.value = text(R.string.sync_error_device_removed)
            return
        }
        val clientId = KeystoreService.getClientId(context)
        if (clientId.isNullOrEmpty()) {
            Log.d(TAG, "[Sync] setupSync: no client ID in Keystore")
            _syncError.value = text(R.string.sync_error_device_removed)
            return
        }


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

        service.onSyncPhaseChanged = { phase ->
            _syncStatusMessage.value = when (phase) {
                SyncPhase.PUSHING -> text(R.string.sync_pushing)
                SyncPhase.PULLING -> text(R.string.sync_pulling)
                null -> null
            }
        }
        service.onNotesQuarantined = { count ->
            _syncError.value = getApplication<Application>().resources
                .getQuantityString(R.plurals.sync_notes_quarantined, count, count)
        }
        service.onConflictDetected = { conflict ->
            _pendingConflicts.value = _pendingConflicts.value.filter { it.id != conflict.id } + conflict
        }
        service.onClockLooksWrong = {
            showMessage(getApplication<Application>().getString(R.string.sync_clock_ahead))
        }
        service.onManyNotesBinned = { count ->
            showMessage(
                getApplication<Application>().resources
                    .getQuantityString(R.plurals.sync_notes_binned, count, count)
            )
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

        startSyncTriggers(sse)

        // Start SSE and run initial sync
        viewModelScope.launch {
            // Conflicts are kept across lock and restart
            try {
                val stored = withContext(Dispatchers.Default) { service.loadConflicts() }
                _pendingConflicts.value = stored
            } catch (e: Exception) {
                Log.w(TAG, "[Sync] Couldn't load conflicts: ${e.javaClass.simpleName}")
            }
            service.startSSE()
            try { triggerSync() } catch (e: Exception) {
                Log.w(TAG, "[Sync] Initial sync failed: ${e.javaClass.simpleName}")
            }
        }
    }

    /**
     * What keeps changes moving when nobody edits: the network returning, and a
     * periodic sync while the app is open, for when SSE is down. Both stop on lock.
     * With the vault locked nothing can sync, so there's no background job.
     */
    private fun startSyncTriggers(sse: SSEClient) {
        connectivityMonitor?.stop()
        connectivityMonitor = ConnectivityMonitor(getApplication()).also { monitor ->
            monitor.onConnected = {
                viewModelScope.launch {
                    if (!isBackgrounded) {
                        sse.reconnectNow()
                        triggerSync()
                    }
                }
            }
            monitor.start()
        }

        periodicSyncJob?.cancel()
        periodicSyncJob = viewModelScope.launch {
            val minutes = syncRepo?.getSyncMetadata()?.autoSyncInterval?.takeIf { it > 0 } ?: DEFAULT_SYNC_MINUTES
            while (true) {
                delay(minutes * 60_000L)
                if (!isBackgrounded) triggerSync()
            }
        }
    }

    /** Try a failed sync again after a while, if that could help. */
    private fun scheduleSyncRetry(error: Exception) {
        val seconds = SyncRetryPolicy.delaySeconds(error, syncRetryAttempt) ?: return
        syncRetryAttempt++
        syncRetryJob?.cancel()
        syncRetryJob = viewModelScope.launch {
            delay(seconds * 1000)
            triggerSync()
        }
    }

    /** Why a sync failed. Never raw server text, which isn't trusted. */
    private fun syncFailureText(e: Exception): String = when {
        e is SyncClientException && e.statusCode == 401 -> text(R.string.sync_error_device_removed)
        e is SyncClientException && e.statusCode == 403 -> text(R.string.sync_error_account_disabled)
        e is SyncClientException && e.statusCode == 507 -> text(R.string.sync_error_storage_full)
        e is SyncClientException && (e.statusCode == 429 || e.statusCode == 503) ->
            text(R.string.sync_error_rate_limited)
        e is java.io.IOException -> text(R.string.sync_error_offline)
        else -> text(R.string.sync_error_failed)
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
            flushPendingEditor()
            uploadPendingRewrap()
            val newCount = withContext(Dispatchers.IO) { service.fullSync() }
            _isSyncing.value = false
            _lastSyncAt.value = Instant.now()
            syncRetryAttempt = 0
            syncRetryJob?.cancel()
            _syncStatusMessage.value = if (newCount > 0) {
                getApplication<Application>().resources
                    .getQuantityString(R.plurals.sync_done_new_notes, newCount, newCount)
            } else {
                text(R.string.sync_done)
            }
            loadNotes()
            // A pull can move the server's side of a conflict on
            val conflicts = withContext(Dispatchers.Default) { service.loadConflicts() }
            if (syncServiceInstance === service) _pendingConflicts.value = conflicts
            loadInboxItems()
        } catch (e: kotlinx.coroutines.CancellationException) {
            _isSyncing.value = false
            throw e
        } catch (e: Exception) {
            Log.e(TAG, "[Sync] triggerSync failed: ${e.javaClass.simpleName}")
            _isSyncing.value = false
            _syncStatusMessage.value = null
            // A lock during a sync isn't a failure worth reporting
            if (syncServiceInstance === service) {
                _syncError.value = syncFailureText(e)
                scheduleSyncRetry(e)
            }
        }

        clearStatusAfterAWhile(text(R.string.sync_done))
    }

    private fun clearStatusAfterAWhile(message: String) {
        viewModelScope.launch {
            delay(4_000)
            if (_syncStatusMessage.value?.startsWith(message) == true) {
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

        syncMutex.withLock {
            _isSyncing.value = true
            _syncError.value = null
            _syncStatusMessage.value = text(R.string.sync_full_in_progress)

            try {
                flushPendingEditor()
                withContext(Dispatchers.IO) { service.fullSync(fullPull = true) }
                _isSyncing.value = false
                _lastSyncAt.value = Instant.now()
                _syncStatusMessage.value = text(R.string.sync_full_done)
                loadNotes()
            } catch (e: kotlinx.coroutines.CancellationException) {
                _isSyncing.value = false
                throw e
            } catch (e: Exception) {
                Log.e(TAG, "[Sync] forceFullSync failed: ${e.javaClass.simpleName}")
                _isSyncing.value = false
                _syncError.value = syncFailureText(e)
                _syncStatusMessage.value = null
            }
        }

        clearStatusAfterAWhile(text(R.string.sync_full_done))
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

    /**
     * Register this device, in the view model's scope. A rotation mustn't cancel a
     * request the server may already have acted on, or the user would register again
     * and leave a second device on their account.
     */
    fun beginRegistration(
        serverUrl: String,
        email: String,
        password: String,
        deviceName: String,
        onRegistered: (PendingConnection) -> Unit = {},
    ) {
        _connectState.value = ConnectState.Working
        viewModelScope.launch {
            registerDevice(serverUrl, email, password, deviceName)
                .onSuccess {
                    _connectState.value = ConnectState.Idle
                    onRegistered(it)
                }
                .onFailure { _connectState.value = ConnectState.Failed(describeAccountFailure(it)) }
        }
    }

    /** Give up on a registered device that never finished joining. */
    fun abandonPendingConnection() {
        val connection = _pendingConnection.value ?: return
        _pendingConnection.value = null
        viewModelScope.launch { revokeQuietly(connection) }
    }

    // MARK: - Sharing In

    private class PendingShare(val text: String?, val files: List<AttachmentFiles.Picked>)

    /** Held in memory only, until the vault is open. */
    private var pendingShare: PendingShare? = null

    /** Something arrived through the share sheet: it becomes a note once unlocked. */
    fun receiveShare(text: String?, files: List<AttachmentFiles.Picked>) {
        if (text == null && files.isEmpty()) return
        pendingShare = PendingShare(text, files)
        if (!_isLocked.value) viewModelScope.launch { applyPendingShare() }
    }

    private suspend fun applyPendingShare() {
        val shared = pendingShare ?: return
        pendingShare = null
        // Text from another app is text, whatever new notes here usually are
        val note = createNote(shared.text.orEmpty(), syntaxLanguage = DEFAULT_NOTE_TYPE) ?: return
        var failed = 0
        for (file in shared.files) {
            if (addAttachment(note.id, file.data, file.name, file.mimeType) == null) failed++
        }
        showMessage(text(if (failed == 0) R.string.share_saved else R.string.share_file_failed))
    }

    // MARK: - Search Help

    private val _otherViewMatches = MutableStateFlow(0)
    /** How many notes match in the view not showing, so a search can't quietly miss them. */
    val otherViewMatches: StateFlow<Int> = _otherViewMatches.asStateFlow()

    private val _recentSearches = MutableStateFlow<List<String>>(emptyList())
    /** In memory only, and gone on lock: what someone searched for says a lot about them. */
    val recentSearches: StateFlow<List<String>> = _recentSearches.asStateFlow()

    fun rememberSearch(query: String) {
        _recentSearches.value = SearchSuggestions.remember(_recentSearches.value, query)
    }

    // MARK: - Quick Actions

    /** Held until the vault is open, like a share. */
    private var pendingQuickAction: QuickAction? = null

    private val _searchRequested = MutableStateFlow(false)
    /** Stays set until the screen has taken it up, which may not exist yet. */
    val searchRequested: StateFlow<Boolean> = _searchRequested.asStateFlow()

    private val _showNotesRequests = MutableSharedFlow<Unit>(extraBufferCapacity = 1)
    /** A quick action can arrive with any screen showing. */
    val showNotesRequests: SharedFlow<Unit> = _showNotesRequests.asSharedFlow()

    fun receiveQuickAction(action: QuickAction) {
        pendingQuickAction = action
        if (!_isLocked.value) viewModelScope.launch { applyPendingQuickAction() }
    }

    private suspend fun applyPendingQuickAction() {
        val action = pendingQuickAction ?: return
        pendingQuickAction = null
        _showNotesRequests.tryEmit(Unit)
        when (action) {
            QuickAction.NEW_NOTE -> createNote()
            QuickAction.SEARCH -> requestSearch()
        }
    }

    private val _findRequested = MutableStateFlow(false)
    /** Stays set until the editor has opened its find bar. */
    val findRequested: StateFlow<Boolean> = _findRequested.asStateFlow()

    fun findRequestHandled() {
        _findRequested.value = false
    }

    /** Returns false for anything that doesn't apply yet, so the key goes on as usual. */
    fun perform(command: KeyboardCommand): Boolean {
        if (_isLocked.value) return false
        when (command) {
            KeyboardCommand.NEW_NOTE -> receiveQuickAction(QuickAction.NEW_NOTE)
            KeyboardCommand.SEARCH -> receiveQuickAction(QuickAction.SEARCH)
            KeyboardCommand.SAVE -> viewModelScope.launch { flushPendingEditor() }
            KeyboardCommand.FIND -> {
                if (_selectedNoteId.value == null) return false
                _findRequested.value = true
            }
            KeyboardCommand.LOCK -> lock()
        }
        return true
    }

    /** Leave the open note and put the cursor in the search bar. */
    fun requestSearch() {
        _selectedNoteId.value = null
        _searchRequested.value = true
    }

    fun searchRequestHandled() {
        _searchRequested.value = false
    }

    // MARK: - Leaving and Managing an Account

    /**
     * Unlink this device and keep its notes. The server is told first, while the
     * key still works, but going ahead regardless: a user who is offline, or whose
     * server predates the call, must still be able to disconnect. Returns false if
     * the device is still listed on the server, so they can be told to remove it.
     */
    suspend fun disconnectSync(): Boolean {
        val client = syncClientInstance
        val revoked = client == null || try {
            withContext(Dispatchers.IO) { client.revokeThisDevice() }
            true
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: Exception) {
            Log.w(TAG, "[Sync] Couldn't revoke this device: ${e.javaClass.simpleName}")
            false
        }
        forgetConnection()
        return revoked
    }

    /** Clear what belongs to the connection. The notes stay. */
    private suspend fun forgetConnection() {
        stopSync()
        val context = getApplication<Application>()
        try {
            KeystoreService.deleteAPIKey(context)
            KeystoreService.deleteClientId(context)
        } catch (e: Exception) {
            Log.w(TAG, "[Sync] Secure preferences: ${e.javaClass.simpleName}")
        }
        syncRepo?.let { repo ->
            val meta = repo.getSyncMetadata() ?: com.jottery.android.model.SyncMetadata()
            repo.saveSyncMetadata(
                meta.copy(
                    userId = null,
                    lastSyncAt = null,
                    pullCursor = null,
                    cursorAdopted = false,
                    wrappedKeyOpened = false,
                    pendingWrappedKey = null,
                )
            )
        }
        updateSettings(_settings.value.copy(syncEnabled = false, syncEndpoint = null))
        _syncError.value = null
        _syncStatusMessage.value = null
        _lastSyncAt.value = null
        _inboxItems.value = emptyList()
        _pendingConflicts.value = emptyList()
    }

    /** The name the server has for this device. */
    suspend fun syncDeviceName(): String? {
        val client = syncClientInstance ?: return null
        return try {
            withContext(Dispatchers.IO) { client.status() }.deviceName.take(MAX_DEVICE_NAME)
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: Exception) {
            null
        }
    }

    /** Create an account. It can't be used until an administrator approves it. */
    suspend fun createAccount(serverUrl: String, email: String, password: String) {
        withContext(Dispatchers.IO) {
            SyncClient(serverUrl, "", "").registerUser(AccountCredentials(email, password))
        }
    }

    suspend fun isAccountApproved(serverUrl: String, email: String, password: String): Boolean {
        val status = withContext(Dispatchers.IO) {
            SyncClient(serverUrl, "", "").userStatus(AccountCredentials(email, password))
        }
        return status.isApproved && status.isActive
    }

    /** Sign in for account actions, returning the session. Kept by the caller only. */
    suspend fun signInToAccount(email: String, password: String): String {
        val client = syncClientInstance ?: throw AppStateError.NotInitialised
        val response = withContext(Dispatchers.IO) { client.logIn(AccountCredentials(email, password)) }
        return response.sessionId ?: throw SyncClientException("No session")
    }

    suspend fun accountInfo(sessionId: String): UserAccountInfo {
        val client = syncClientInstance ?: throw AppStateError.NotInitialised
        return withContext(Dispatchers.IO) { client.accountInfo(sessionId) }
    }

    /** A token for sending things to the inbox. Shown once: the server keeps a hash. */
    suspend fun generateInboxToken(sessionId: String): String {
        val client = syncClientInstance ?: throw AppStateError.NotInitialised
        return withContext(Dispatchers.IO) { client.generateInboxToken(sessionId) }.token
    }

    suspend fun revokeInboxToken(sessionId: String) {
        val client = syncClientInstance ?: throw AppStateError.NotInitialised
        withContext(Dispatchers.IO) { client.revokeInboxToken(sessionId) }
    }

    suspend fun deleteServerNotes(sessionId: String) {
        val client = syncClientInstance ?: throw AppStateError.NotInitialised
        withContext(Dispatchers.IO) { client.deleteServerNotes(sessionId) }
    }

    /**
     * Deactivate or delete the account this device syncs with. The notes on this
     * device stay: closing a sync account isn't deleting notes. No revoke is sent,
     * because the account's devices are already gone.
     */
    suspend fun closeAccount(sessionId: String, mode: AccountDeletionMode) {
        val client = syncClientInstance ?: throw AppStateError.NotInitialised
        withContext(Dispatchers.IO) { client.deleteAccount(sessionId, mode.wireValue) }
        forgetConnection()
    }

    /** Why an account action failed, as something the user can act on. */
    fun describeAccountFailure(e: Throwable): String = when {
        e is SyncClientException && e.statusCode == 401 -> text(R.string.account_error_credentials)
        e is SyncClientException && e.statusCode == 409 -> text(R.string.account_error_exists)
        // 403 and 429 carry the server's reason: not approved, deactivated, or how long to wait
        e is SyncClientException && e.serverMessage != null && (e.statusCode == 403 || e.statusCode == 429) ->
            e.serverMessage
        e is SyncClientException -> text(R.string.account_error_server)
        else -> text(R.string.connect_error_network)
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
            finishConnection(connection, openedBlob = master != null)
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
        val summary = withContext(Dispatchers.IO) { backupService().write(out, key) }
        BackupReminder.recordBackup(getApplication())
        return summary
    }

    /** Look for what can't be read, or isn't here. */
    suspend fun scanVaultHealth(): VaultHealthService.Report {
        val notes = noteRepo ?: return VaultHealthService.Report()
        val attachments = attachmentRepo ?: return VaultHealthService.Report()
        val key = keyManager.masterKey ?: return VaultHealthService.Report()
        return withContext(Dispatchers.Default) { VaultHealthService(notes, attachments).scan(key) }
    }

    /** Ask the server for a blob this device doesn't have. */
    suspend fun repairAttachment(blobId: String): BlobRepair =
        syncServiceInstance?.repairBlob(blobId) ?: BlobRepair.FAILED

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
        afterBulkChange()
    }

    /** Set or clear the colour of several notes. Locked ones are left alone. */
    suspend fun setColourOnNotes(ids: Set<String>, colour: String?) {
        val repo = noteRepo ?: return
        val key = keyManager.masterKey ?: return
        for (id in ids) {
            val note = repo.get(id, key) ?: continue
            if (!note.locked && note.color != colour) repo.update(note.copy(color = colour), key)
        }
        afterBulkChange()
    }

    /** Archive several notes. Returns how many were left because they're locked. */
    suspend fun archiveNotes(ids: Set<String>): Int {
        val repo = noteRepo ?: return 0
        val key = keyManager.masterKey ?: return 0
        var skipped = 0
        for (id in ids) {
            val note = repo.get(id, key) ?: continue
            if (note.locked) skipped++ else if (!note.archived) repo.archive(id)
        }
        if (_selectedNoteId.value in ids) _selectedNoteId.value = null
        afterBulkChange()
        return skipped
    }

    /**
     * Delete several notes to the bin, as web does: pinned and locked ones stay.
     * Returns how many were left.
     */
    suspend fun deleteNotes(ids: Set<String>): Int {
        val repo = noteRepo ?: return 0
        val key = keyManager.masterKey ?: return 0
        var skipped = 0
        for (id in ids) {
            val note = repo.get(id, key) ?: continue
            if (note.pinned || note.locked) skipped++ else repo.softDelete(id)
        }
        if (_selectedNoteId.value in ids) _selectedNoteId.value = null
        afterBulkChange()
        return skipped
    }

    /**
     * Combine notes into one, oldest first, and move the originals to the bin.
     * Locked notes take no part. Returns the new note, or null if fewer than two
     * notes could be used, having changed nothing.
     */
    suspend fun combineNotes(ids: Set<String>): DecryptedNote? {
        val repo = noteRepo ?: return null
        val key = keyManager.masterKey ?: return null
        val sources = ids.mapNotNull { repo.get(it, key) }
            .filter { !it.locked }
            .sortedBy { it.createdAt }
        if (sources.size < 2) return null

        val combined = repo.create(
            content = sources.joinToString(COMBINED_NOTE_SEPARATOR) { it.content },
            tags = sources.flatMap { it.tags }.distinct(),
            key = key,
        )
        val refs = sources.flatMap { repo.rawAttachmentRefs(it.id) }.distinctBy { it.id }
        if (refs.isNotEmpty()) repo.setAttachments(combined.id, refs, key)
        sources.forEach { repo.softDelete(it.id) }

        _selectedNoteId.value = combined.id
        afterBulkChange()
        return combined
    }

    /** One reload for the whole batch, rather than one per note. */
    private suspend fun afterBulkChange() {
        notesRevision.incrementAndGet()
        loadNotes()
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
                syncServiceInstance?.startSSE()
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
        private const val KEY_DEFAULT_NOTE_TYPE = "defaultNoteType"
        private const val DEFAULT_NOTE_TYPE = "markdown"
        private const val KEY_SCREEN_SECURITY = "screenSecurity"
        private const val KEY_BIOMETRIC_OFFERED = "biometricOffered"
        private const val MAX_LOAD_ATTEMPTS = 3
        private const val DEFAULT_SYNC_MINUTES = 5
        private const val HTTP_CONFLICT = 409
        private const val MAX_DEVICE_NAME = 100

        /** What web puts between notes it combines. */
        private const val COMBINED_NOTE_SEPARATOR = "\n\n---\n\n"
    }
}

/** The note a selection refers to, wherever it is and whatever the search shows. */
fun resolveSelectedNote(
    id: String?,
    notes: List<DecryptedNote>,
    archived: List<DecryptedNote>,
): DecryptedNote? = id?.let { wanted ->
    notes.firstOrNull { it.id == wanted } ?: archived.firstOrNull { it.id == wanted }
}

/** What closing an account does, with the value the server expects. */
enum class AccountDeletionMode(val wireValue: String) {
    DEACTIVATE("deactivate"),
    DELETE("delete"),
}

/** A default name for an export file. */
fun defaultExportName(): String = "jottery-export-${java.time.LocalDate.now()}.json"

// MARK: - Error Types

sealed class AppStateError(message: String) : Exception(message) {
    data object NotInitialised : AppStateError("Application not initialised")
    data object NoVault : AppStateError("No vault found")
    data object InvalidSalt : AppStateError("Invalid encryption salt")
    data object WrongPassword : AppStateError("Incorrect password")
    data object BiometricUnavailable : AppStateError("Biometric unlock isn't available")
}
