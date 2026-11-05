package com.jottery.android.testing

import androidx.room.withTransaction
import androidx.test.core.app.ApplicationProvider
import com.jottery.android.crypto.CryptoService
import com.jottery.android.database.JotteryDatabase
import com.jottery.android.database.repository.AttachmentRepository
import com.jottery.android.database.repository.NoteRepository
import com.jottery.android.database.repository.SavedSearchRepository
import com.jottery.android.database.repository.SyncRepository
import com.jottery.android.database.repository.VersionRepository
import com.jottery.android.model.NoteRecord
import com.jottery.android.model.SyncAttachment
import com.jottery.android.model.SyncNote
import com.jottery.android.model.SyncPullResponse
import com.jottery.android.model.SyncPushResponse
import com.jottery.android.model.SyncRejected
import com.jottery.android.network.SSEClient
import com.jottery.android.network.SyncClient
import com.jottery.android.service.SyncService
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import okhttp3.mockwebserver.Dispatcher
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okhttp3.mockwebserver.RecordedRequest
import javax.crypto.spec.SecretKeySpec

/**
 * Real SyncService, real Room (in memory), real OkHttp, stub HTTP server.
 * Each test sets [onPush] and [onPull] to script the server.
 */
class SyncHarness(
    /** Talk to a real server rather than the stub: its address and this device's key. */
    private val realServer: RealServer? = null,
) {
    class RealServer(val endpoint: String, val apiKey: String, val clientId: String)

    val key = SecretKeySpec(ByteArray(32) { it.toByte() }, "AES")
    val db: JotteryDatabase = JotteryDatabase.buildInMemory(ApplicationProvider.getApplicationContext())
    val noteRepo = NoteRepository(db.noteDao(), db.noteVersionDao()) { block -> db.withTransaction { block() } }
    val versionRepo = VersionRepository(db.noteVersionDao())
    val attachmentRepo = AttachmentRepository(db.attachmentDao())
    val syncRepo = SyncRepository(db.syncMetadataDao(), db.noteSyncMetadataDao(), db.noteDeletionDao())
    val savedSearchRepo = SavedSearchRepository(db.savedSearchDao())

    private val json = Json { ignoreUnknownKeys = true; encodeDefaults = true }

    var onPush: (String) -> SyncPushResponse = { SyncPushResponse(emptyList(), emptyList()) }
    var onPull: (String) -> SyncPullResponse = { emptyPull() }

    /** Blobs the server will hand over one at a time, by id. */
    val serverBlobs = mutableMapOf<String, String>()
    val blobRequests = mutableListOf<String>()
    val pushBodies = mutableListOf<String>()
    val pullBodies = mutableListOf<String>()

    val server = MockWebServer().apply {
        dispatcher = object : Dispatcher() {
            override fun dispatch(request: RecordedRequest): MockResponse {
                val body = request.body.readUtf8()
                return when (request.path) {
                    "/api/v1/sync/push" -> {
                        pushBodies += body
                        try {
                            MockResponse().setBody(json.encodeToString(onPush(body)))
                        } catch (refusal: HttpRefusal) {
                            MockResponse().setResponseCode(refusal.status).setBody(refusal.body)
                        }
                    }
                    "/api/v1/sync/pull" -> {
                        pullBodies += body
                        MockResponse().setBody(json.encodeToString(onPull(body)))
                    }
                    else -> {
                        val id = request.path.orEmpty().substringAfter("/api/v1/sync/attachments/", "")
                        blobRequests += id
                        serverBlobs[id]?.let { data ->
                            MockResponse().setBody(json.encodeToString(SyncAttachment(id, data)))
                        } ?: MockResponse().setResponseCode(404).setBody("{}")
                    }
                }
            }
        }
        start()
    }

    /** Thrown by [onPush] to answer with an error status. */
    class HttpRefusal(val status: Int, val body: String) : Exception()

    fun storageFull() = HttpRefusal(507, """{"error":"Your account is full","code":"storage_quota_exceeded"}""")

    val endpoint: String get() = realServer?.endpoint ?: server.url("/").toString().trimEnd('/')
    val client = SyncClient(endpoint, realServer?.apiKey ?: "test-api-key", realServer?.clientId ?: "test-client")

    fun service(
        transaction: suspend (suspend () -> Unit) -> Unit = { block -> db.withTransaction { block() } },
    ): SyncService = SyncService(
        syncClient = client,
        sseClient = SSEClient(endpoint, CoroutineScope(Dispatchers.Unconfined)),
        noteRepo = noteRepo,
        versionRepo = versionRepo,
        attachmentRepo = attachmentRepo,
        syncRepo = syncRepo,
        savedSearchRepo = savedSearchRepo,
        keyProvider = { key },
        scope = CoroutineScope(Dispatchers.Unconfined),
        transaction = transaction,
    )

    fun close() {
        server.shutdown()
        db.close()
    }

    fun encrypt(text: String): String =
        CryptoService.serializeEncryptedJSON(CryptoService.encryptText(text, key))

    fun encryptTag(tag: String): String = encrypt(json.encodeToString(tag))

    fun decryptTag(ciphertext: String): String =
        json.decodeFromString(CryptoService.decryptText(CryptoService.parseEncryptedJSON(ciphertext), key))

    /** Put a note in the database in the state it has after a clean sync. */
    suspend fun seedSynced(id: String, text: String, hash: String, chain: List<String> = listOf(hash)): NoteRecord {
        val record = NoteRecord(
            id = id,
            createdAt = "2026-01-01T00:00:00.000Z",
            modifiedAt = "2026-01-01T00:00:00.000Z",
            syncedAt = "2026-01-01T00:00:01.000Z",
            content = encrypt(text),
            tags = CryptoService.serializeEncryptedJSON(CryptoService.encryptStringArray(emptyList(), key)),
            version = 1,
            contentHash = hash,
            parentHash = null,
            hashChain = json.encodeToString(chain),
            needsSync = false,
        )
        noteRepo.insertOrReplace(record)
        return record
    }

    fun remoteNote(
        id: String,
        text: String,
        hash: String,
        parent: String?,
        chain: List<String>,
        modifiedAt: String = "2026-02-01T00:00:00.000Z",
    ) = SyncNote(
        id = id,
        createdAt = "2026-01-01T00:00:00.000Z",
        modifiedAt = modifiedAt,
        content = encrypt(text),
        tags = emptyList(),
        attachments = emptyList(),
        pinned = false,
        deleted = false,
        version = 2,
        contentHash = hash,
        parentHash = parent,
        hashChain = chain,
    )

    fun rejection(note: SyncNote) = SyncRejected(
        id = note.id,
        reason = "Conflict: diverged from common ancestor",
        serverModifiedAt = note.modifiedAt,
        serverContent = note.content,
        serverTags = note.tags,
        serverVersion = note.version,
        serverAttachments = note.attachments,
        serverPinned = note.pinned,
        serverContentHash = note.contentHash,
        serverParentHash = note.parentHash,
        serverHashChain = note.hashChain,
    )

    companion object {
        fun emptyPull(vararg notes: SyncNote) = SyncPullResponse(
            notes = notes.toList(),
            attachments = emptyList(),
            versions = emptyList(),
            syncedAt = "2026-03-01T00:00:00.000Z",
            hasMore = false,
        )
    }
}
