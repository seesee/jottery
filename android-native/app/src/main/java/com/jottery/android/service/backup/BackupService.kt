package com.jottery.android.service.backup

import android.util.JsonReader
import android.util.JsonToken
import android.util.Log
import com.jottery.android.crypto.CryptoService
import com.jottery.android.database.repository.AttachmentRepository
import com.jottery.android.database.repository.EncryptionRepository
import com.jottery.android.database.repository.NoteRepository
import com.jottery.android.database.repository.SavedSearchRepository
import com.jottery.android.database.repository.VersionRepository
import com.jottery.android.model.AttachmentRecord
import com.jottery.android.model.AttachmentRef
import com.jottery.android.model.EncryptionMetadata
import com.jottery.android.model.NoteRecord
import com.jottery.android.model.NoteVersionRecord
import com.jottery.android.model.SavedSearch
import com.jottery.android.util.DateUtils
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.intOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.put
import java.io.InputStream
import java.io.InputStreamReader
import java.io.OutputStream
import java.util.Base64
import javax.crypto.SecretKey
import javax.crypto.spec.SecretKeySpec

/** Why a backup can't be restored. The UI maps each case to a string. */
sealed class BackupException(message: String) : Exception(message) {
    class WrongPassword : BackupException("Incorrect password for this backup")
    class NotABackup : BackupException("This isn't a Jottery backup")
    class UnsupportedVersion(val version: String) : BackupException("Unsupported backup version: $version")
    class Corrupt : BackupException("The backup is damaged")

    /** A backup made by Android before 1.4, with no key in it, on the wrong vault. */
    class KeyNotInBackup : BackupException("This backup doesn't contain its key")
    class NoVault : BackupException("No vault found")
}

/** A way to open the backup file, so it can be read twice without holding it. */
fun interface BackupSource {
    fun open(): InputStream
}

/** What a restore would do, for the user to confirm. */
data class RestorePlan(
    val notes: Int,
    val versions: Int,
    val attachments: Int,
    val savedSearches: Int,
    /** Notes here that the backup would replace with an older-or-equal copy... */
    val replaced: Int,
    /** ...and notes here that are newer than the backup's copy, so are kept. */
    val keptNewer: Int,
    /** Notes in the backup that can't be read with its own key. */
    val unreadable: Int,
    internal val backupKey: SecretKey,
    internal val header: BackupHeader,
)

/** The raw key a plan was checked with, to open the vault after a fresh restore. */
fun RestorePlan.keyBytes(): ByteArray = backupKey.encoded

data class RestoreResult(
    val notesRestored: Int,
    val keptNewer: Int,
    val unreadable: Int,
    val attachmentsRestored: Int,
)

/** The `encryption` object: how to get from the password to the key. */
@Serializable
data class BackupEncryption(
    val envelopeVersion: Int? = null,
    val deviceSalt: String? = null,
    val localWrappedMaster: String? = null,
    val wrappingKdfVersion: Int? = null,
    val salt: String? = null,
    val iterations: Int? = null,
    val createdAt: String? = null,
    val algorithm: String = "AES-256-GCM",
)

data class BackupHeader(
    val version: String,
    val encryption: BackupEncryption?,
)

/**
 * Creates and restores backups in the format shared by every client (decision
 * D6, docs/backup-format.md). A backup carries what it needs to rebuild its key
 * from the password, so it restores on a fresh install or on any other client.
 *
 * Files are streamed a batch at a time, because a vault with attachments may
 * not fit in memory. Restore reads the file twice: once to prove every batch
 * opens, then once to write, in a single transaction.
 *
 * Also reads: web's v2.1 records, iOS's batches (see [BackupCodec]), and Android
 * backups from before this format, which have no key and so only restore into
 * the vault that made them.
 */
class BackupService(
    private val noteRepo: NoteRepository,
    private val versionRepo: VersionRepository,
    private val attachmentRepo: AttachmentRepository,
    private val savedSearchRepo: SavedSearchRepository,
    private val encryptionRepo: EncryptionRepository,
    private val transaction: suspend (suspend () -> Unit) -> Unit,
) {
    companion object {
        private const val TAG = "BackupService"
        private const val FORMAT_VERSION = "3.0"
        private const val FORMAT_TYPE = "jottery-encrypted-backup"
        private const val NOTE_BATCH = 250
        private const val VERSION_BATCH = 250
        private const val ATTACHMENT_BATCH = 50

        /** Keeps memory bounded: web's count limit alone allows 500 MB a batch. */
        private const val BATCH_BYTES = 16L * 1024 * 1024

        /** A larger value in a header can only be a way to hang the app. */
        private const val MAX_ITERATIONS = 10_000_000
    }

    private val json = Json { ignoreUnknownKeys = true; encodeDefaults = true; explicitNulls = false }

    // MARK: - Create

    /** Write a backup of everything in the vault. Returns the summary counts. */
    suspend fun write(out: OutputStream, key: SecretKey): Summary {
        val metadata = encryptionRepo.get() ?: throw BackupException.NoVault()
        val writer = out.bufferedWriter(Charsets.UTF_8)
        val encryption = headerFor(metadata)

        // These keys in this order, and a batch to a line: web reads large files
        // in pieces and finds the header at the start and the summary at the end.
        writer.write("{\n\"version\":\"$FORMAT_VERSION\",\n\"type\":\"$FORMAT_TYPE\",\n")
        writer.write("\"createdAt\":${JsonPrimitive(DateUtils.nowISO8601())},\n")
        writer.write("\"encryption\":${json.encodeToString(encryption)},\n")
        writer.write("\"batches\":[\n")

        var first = true
        val indexes = mutableMapOf<String, Int>()
        fun emit(type: String, items: List<JsonElement>) {
            if (items.isEmpty()) return
            val index = indexes.getOrDefault(type, 0)
            indexes[type] = index + 1
            val jwe = BackupCodec.encryptBatch(JsonArray(items).toString(), key)
            if (!first) writer.write(",\n")
            first = false
            writer.write("{\"type\":\"$type\",\"index\":$index,\"count\":${items.size},\"data\":\"$jwe\"}")
        }

        val notes = noteRepo.listAllRaw()
        notes.chunked(NOTE_BATCH).forEach { batch -> emit("notes", batch.map(::noteToJson)) }

        var attachments = 0
        val pending = mutableListOf<JsonElement>()
        var pendingBytes = 0L
        for (id in attachmentRepo.listBlobIds()) {
            val blob = attachmentRepo.getBlob(id) ?: continue
            attachments++
            pending += buildJsonObject {
                put("id", id)
                put("blob", Base64.getEncoder().encodeToString(blob))
            }
            pendingBytes += blob.size
            if (pending.size >= ATTACHMENT_BATCH || pendingBytes >= BATCH_BYTES) {
                emit("attachments", pending.toList())
                pending.clear()
                pendingBytes = 0
            }
        }
        emit("attachments", pending.toList())
        pending.clear()

        var versions = 0
        pendingBytes = 0
        for (note in notes) {
            for (version in versionRepo.getVersions(note.id)) {
                versions++
                pending += versionToJson(version)
                pendingBytes += version.content.length
                if (pending.size >= VERSION_BATCH || pendingBytes >= BATCH_BYTES) {
                    emit("versions", pending.toList())
                    pending.clear()
                    pendingBytes = 0
                }
            }
        }
        emit("versions", pending.toList())

        val searches = savedSearchRepo.listAllRaw()
        emit("saved_searches", searches.map(::searchToJson))

        val summary = Summary(notes.size, attachments, versions, searches.size)
        writer.write("\n],\n\"summary\":${json.encodeToString(summary)}\n}")
        writer.flush()
        return summary
    }

    @Serializable
    data class Summary(
        val notes: Int,
        val attachments: Int,
        val versions: Int,
        val savedSearches: Int,
    )

    private fun headerFor(metadata: EncryptionMetadata): BackupEncryption =
        if (metadata.envelopeVersion != null) {
            BackupEncryption(
                envelopeVersion = metadata.envelopeVersion,
                deviceSalt = metadata.deviceSalt,
                localWrappedMaster = metadata.localWrappedMaster,
                wrappingKdfVersion = metadata.wrappingKdfVersion,
                createdAt = metadata.createdAt,
                algorithm = metadata.algorithm,
            )
        } else {
            BackupEncryption(
                salt = metadata.salt,
                iterations = metadata.iterations,
                createdAt = metadata.createdAt,
                algorithm = metadata.algorithm,
            )
        }

    private fun noteToJson(note: NoteRecord): JsonObject = buildJsonObject {
        put("id", note.id)
        put("createdAt", note.createdAt)
        put("modifiedAt", note.modifiedAt)
        note.syncedAt?.let { put("syncedAt", it) }
        put("content", note.content)
        // Web stores the single encrypted tag blob as a one-element array
        put("tags", JsonArray(listOf(JsonPrimitive(note.tags))))
        put("attachments", refsToJson(note.attachments))
        put("pinned", note.pinned)
        put("archived", note.archived)
        note.archivedAt?.let { put("archivedAt", it) }
        put("locked", note.locked)
        note.lockedAt?.let { put("lockedAt", it) }
        put("deleted", note.deleted)
        note.deletedAt?.let { put("deletedAt", it) }
        put("version", note.version)
        put("wordWrap", note.wordWrap)
        put("syntaxLanguage", note.syntaxLanguage)
        put("showPreview", note.showPreview)
        note.color?.let { put("color", it) }
        note.contentHash?.let { put("contentHash", it) }
        note.parentHash?.let { put("parentHash", it) }
        note.hashChain?.let { put("hashChain", parseOrEmptyArray(it)) }
    }

    private fun versionToJson(version: NoteVersionRecord): JsonObject = buildJsonObject {
        put("versionKey", version.versionKey)
        put("noteId", version.noteId)
        put("version", version.version)
        put("createdAt", version.createdAt)
        put("syncedAt", version.syncedAt)
        put("content", version.content)
        put("tags", JsonArray(listOf(JsonPrimitive(version.tags))))
        put("attachments", refsToJson(version.attachments))
        version.syntaxLanguage?.let { put("syntaxLanguage", it) }
        version.wordWrap?.let { put("wordWrap", it) }
        version.showPreview?.let { put("showPreview", it) }
        version.color?.let { put("color", it) }
        put("reason", version.reason)
        version.contentHash?.let { put("contentHash", it) }
        version.parentHash?.let { put("parentHash", it) }
    }

    private fun searchToJson(search: SavedSearch): JsonObject = buildJsonObject {
        put("id", search.id)
        put("name", search.name)
        put("query", search.query)
        put("order", search.displayOrder)
        put("createdAt", search.createdAt)
        put("modifiedAt", search.modifiedAt)
        search.syncedAt?.let { put("syncedAt", it) }
        put("deleted", search.deleted)
        search.deletedAt?.let { put("deletedAt", it) }
        put("version", search.version)
    }

    private fun refsToJson(attachments: String): JsonElement = parseOrEmptyArray(attachments)

    private fun parseOrEmptyArray(text: String): JsonElement = try {
        json.parseToJsonElement(text)
    } catch (_: Exception) {
        JsonArray(emptyList())
    }

    // MARK: - Inspect

    /**
     * Establish the backup's key and prove every batch opens, changing nothing.
     *
     * @param currentKey the open vault's key, or null when there is no vault yet.
     */
    suspend fun inspect(source: BackupSource, password: String, currentKey: SecretKey?): RestorePlan {
        val header = readHeader(source)
        val backupKey = keyFromHeader(header, password, currentKey)

        var notes = 0
        var versions = 0
        var attachments = 0
        var searches = 0
        var replaced = 0
        var keptNewer = 0
        var unreadable = 0

        forEachItem(source, backupKey) { type, item ->
            when (type) {
                ItemType.NOTE -> {
                    val readable = isReadable(item, backupKey)
                    if (!readable) {
                        unreadable++
                    } else {
                        notes++
                        val local = item.string("id")?.let { noteRepo.getRaw(it) }
                        if (local != null) {
                            if (shouldKeepLocal(local, item)) keptNewer++ else replaced++
                        }
                    }
                }
                ItemType.VERSION -> versions++
                ItemType.ATTACHMENT -> attachments++
                ItemType.SEARCH -> searches++
            }
        }

        // No note opened under the header's key, so nothing here is usable
        if (notes == 0 && unreadable > 0) {
            throw if (header.encryption == null) BackupException.KeyNotInBackup() else BackupException.WrongPassword()
        }

        return RestorePlan(
            notes = notes,
            versions = versions,
            attachments = attachments,
            savedSearches = searches,
            replaced = replaced,
            keptNewer = keptNewer,
            unreadable = unreadable,
            backupKey = backupKey,
            header = header,
        )
    }

    private fun isReadable(item: JsonObject, key: SecretKey): Boolean = try {
        decryptField(item.getValue("content"), key)
        true
    } catch (_: Exception) {
        false
    }

    private fun shouldKeepLocal(local: NoteRecord, item: JsonObject): Boolean {
        val localTime = DateUtils.parseISO8601(local.modifiedAt) ?: return false
        val backupTime = item.string("modifiedAt")?.let(DateUtils::parseISO8601) ?: return true
        return !backupTime.isAfter(localTime)
    }

    private fun keyFromHeader(header: BackupHeader, password: String, currentKey: SecretKey?): SecretKey {
        val encryption = header.encryption
            ?: return currentKey ?: throw BackupException.KeyNotInBackup()

        if (encryption.envelopeVersion != null) {
            val saltB64 = encryption.deviceSalt ?: throw BackupException.Corrupt()
            val wrappedJSON = encryption.localWrappedMaster ?: throw BackupException.Corrupt()
            val salt = decodeSalt(saltB64)
            val deviceKey = CryptoService.deriveKey(password, salt, CryptoService.DEFAULT_ITERATIONS)
            val wrapped = try {
                CryptoService.parseEncryptedJSON(wrappedJSON)
            } catch (_: Exception) {
                throw BackupException.Corrupt()
            }
            val master = try {
                CryptoService.unwrapMasterKey(wrapped, deviceKey)
            } catch (_: Exception) {
                throw BackupException.WrongPassword()
            }
            if (master.size != CryptoService.KEY_LENGTH) throw BackupException.Corrupt()
            return SecretKeySpec(master, "AES")
        }

        val salt = encryption.salt
        val iterations = encryption.iterations
        if (salt.isNullOrEmpty() || iterations == null || iterations <= 0) {
            // Early iOS backups of envelope vaults: the vault's key is the only hope
            return currentKey ?: throw BackupException.KeyNotInBackup()
        }
        if (iterations > MAX_ITERATIONS) throw BackupException.Corrupt()
        return CryptoService.deriveKey(password, decodeSalt(salt), iterations)
    }

    private fun decodeSalt(salt: String): ByteArray = try {
        Base64.getDecoder().decode(salt)
    } catch (_: IllegalArgumentException) {
        throw BackupException.Corrupt()
    }

    // MARK: - Restore

    /** Merge a backup into the open vault. Newer local notes are kept. */
    suspend fun restore(source: BackupSource, plan: RestorePlan, currentKey: SecretKey): RestoreResult =
        apply(source, plan, currentKey, metadata = null)

    /**
     * Fresh install: the vault is built from the backup itself, using the
     * backup's key and header, in the same transaction as the data.
     */
    suspend fun restoreAsNewVault(source: BackupSource, plan: RestorePlan): RestoreResult {
        val encryption = plan.header.encryption ?: throw BackupException.KeyNotInBackup()
        val verification = CryptoService.serializeEncryptedJSON(
            CryptoService.encryptText(EncryptionMetadata.VERIFICATION_PLAINTEXT, plan.backupKey)
        )
        val metadata = EncryptionMetadata(
            salt = encryption.salt,
            iterations = encryption.iterations,
            createdAt = encryption.createdAt ?: DateUtils.nowISO8601(),
            verification = verification,
            envelopeVersion = encryption.envelopeVersion,
            deviceSalt = encryption.deviceSalt,
            localWrappedMaster = encryption.localWrappedMaster,
            wrappingKdfVersion = encryption.wrappingKdfVersion,
        )
        return apply(source, plan, plan.backupKey, metadata)
    }

    private suspend fun apply(
        source: BackupSource,
        plan: RestorePlan,
        targetKey: SecretKey,
        metadata: EncryptionMetadata?,
    ): RestoreResult {
        val backupKey = plan.backupKey
        var notes = 0
        var keptNewer = 0
        var unreadable = 0
        var attachments = 0

        transaction {
            metadata?.let { encryptionRepo.store(it) }
            forEachItem(source, backupKey) { type, item ->
                when (type) {
                    ItemType.NOTE -> {
                        if (!isReadable(item, backupKey)) {
                            unreadable++
                            return@forEachItem
                        }
                        val record = noteFrom(item, backupKey, targetKey)
                        val local = noteRepo.getRaw(record.id)
                        if (local != null) {
                            if (shouldKeepLocal(local, item)) {
                                keptNewer++
                                return@forEachItem
                            }
                            versionRepo.insertOrReplace(NoteVersionRecord.safetySnapshot(local, "pre-restore"))
                        }
                        noteRepo.insertOrReplace(record)
                        notes++
                    }
                    ItemType.ATTACHMENT -> {
                        if (restoreBlob(item, backupKey, targetKey)) attachments++
                    }
                    ItemType.VERSION -> restoreVersion(item, backupKey, targetKey)
                    ItemType.SEARCH -> restoreSearch(item, backupKey, targetKey)
                }
            }
        }

        return RestoreResult(notes, keptNewer, unreadable, attachments)
    }

    private fun noteFrom(item: JsonObject, backupKey: SecretKey, targetKey: SecretKey): NoteRecord {
        val id = item.string("id") ?: throw BackupException.Corrupt()
        val now = DateUtils.nowISO8601()
        return NoteRecord(
            id = id,
            createdAt = item.string("createdAt") ?: now,
            modifiedAt = item.string("modifiedAt") ?: now,
            syncedAt = item.string("syncedAt"),
            content = reEncrypt(item.getValue("content"), backupKey, targetKey),
            tags = tagsFrom(item["tags"], backupKey, targetKey),
            attachments = refsFrom(item["attachments"], backupKey, targetKey),
            pinned = item.bool("pinned") ?: false,
            archived = item.bool("archived") ?: false,
            archivedAt = item.string("archivedAt"),
            locked = item.bool("locked") ?: false,
            lockedAt = item.string("lockedAt"),
            deleted = item.bool("deleted") ?: false,
            deletedAt = item.string("deletedAt"),
            version = item.int("version") ?: 1,
            wordWrap = item.bool("wordWrap") ?: true,
            syntaxLanguage = item.string("syntaxLanguage") ?: "markdown",
            showPreview = item.bool("showPreview") ?: false,
            color = item.string("color"),
            contentHash = item.string("contentHash"),
            parentHash = item.string("parentHash"),
            hashChain = hashChainFrom(item["hashChain"]),
            // Pushed on the next sync, so every client converges on what's here
            needsSync = true,
            // A restore mustn't hand the purge a date from long ago
            localDeletedAt = if (item.bool("deleted") == true) now else null,
        )
    }

    private suspend fun restoreVersion(item: JsonObject, backupKey: SecretKey, targetKey: SecretKey) {
        val versionKey = item.string("versionKey") ?: return
        if (versionRepo.getByKey(versionKey) != null) return
        try {
            versionRepo.insertOrReplace(
                NoteVersionRecord(
                    versionKey = versionKey,
                    noteId = item.string("noteId") ?: return,
                    version = item.int("version") ?: 0,
                    createdAt = item.string("createdAt") ?: DateUtils.nowISO8601(),
                    syncedAt = item.string("syncedAt") ?: DateUtils.nowISO8601(),
                    content = reEncrypt(item.getValue("content"), backupKey, targetKey),
                    tags = tagsFrom(item["tags"], backupKey, targetKey),
                    attachments = refsFrom(item["attachments"], backupKey, targetKey),
                    syntaxLanguage = item.string("syntaxLanguage"),
                    wordWrap = item.bool("wordWrap"),
                    showPreview = item.bool("showPreview"),
                    color = item.string("color"),
                    reason = item.string("reason") ?: "backup-restore",
                    contentHash = item.string("contentHash"),
                    parentHash = item.string("parentHash"),
                )
            )
        } catch (e: BackupException) {
            throw e
        } catch (e: Exception) {
            Log.w(TAG, "Skipping unreadable version $versionKey")
        }
    }

    private suspend fun restoreBlob(item: JsonObject, backupKey: SecretKey, targetKey: SecretKey): Boolean {
        val id = item.string("id") ?: return false
        // Web and iOS call the field "blob"; Android's older format called it "data"
        val encoded = item.string("blob") ?: item.string("data") ?: return false
        val blob = try {
            Base64.getDecoder().decode(encoded)
        } catch (_: IllegalArgumentException) {
            return false
        }

        val stored = if (sameKey(backupKey, targetKey)) {
            blob
        } else {
            val plain = try {
                CryptoService.decrypt(CryptoService.parseEncryptedJSON(String(blob, Charsets.UTF_8)), backupKey)
            } catch (e: Exception) {
                Log.w(TAG, "Skipping unreadable attachment $id")
                return false
            }
            CryptoService.serializeEncryptedJSON(CryptoService.encrypt(plain, targetKey))
                .toByteArray(Charsets.UTF_8)
        }
        attachmentRepo.insertOrReplace(
            AttachmentRecord(id = id, filename = "", mimeType = "", size = stored.size, data = stored)
        )
        return true
    }

    private suspend fun restoreSearch(item: JsonObject, backupKey: SecretKey, targetKey: SecretKey) {
        val id = item.string("id") ?: return
        val modifiedAt = item.string("modifiedAt") ?: item.string("createdAt") ?: DateUtils.nowISO8601()
        val local = savedSearchRepo.listAllRaw().firstOrNull { it.id == id }
        if (local != null) {
            val localTime = DateUtils.parseISO8601(local.modifiedAt)
            val backupTime = DateUtils.parseISO8601(modifiedAt)
            if (localTime != null && backupTime != null && !backupTime.isAfter(localTime)) return
        }
        try {
            savedSearchRepo.insertOrReplace(
                SavedSearch(
                    id = id,
                    name = reEncrypt(item.getValue("name"), backupKey, targetKey),
                    query = reEncrypt(item.getValue("query"), backupKey, targetKey),
                    displayOrder = item.int("order") ?: 0,
                    createdAt = item.string("createdAt") ?: modifiedAt,
                    modifiedAt = modifiedAt,
                    deleted = item.bool("deleted") ?: false,
                    deletedAt = item.string("deletedAt"),
                    version = item.int("version") ?: 1,
                    needsSync = true,
                )
            )
        } catch (e: BackupException) {
            throw e
        } catch (e: Exception) {
            Log.w(TAG, "Skipping unreadable saved search $id")
        }
    }

    // MARK: - Field Conversion

    private fun sameKey(a: SecretKey, b: SecretKey) = a.encoded.contentEquals(b.encoded)

    /** A field is a JSON string holding {ciphertext, iv}, or that object itself. */
    private fun encryptedJson(element: JsonElement): String =
        (element as? JsonPrimitive)?.contentOrNull ?: element.toString()

    private fun decryptField(element: JsonElement, key: SecretKey): ByteArray =
        CryptoService.decrypt(CryptoService.parseEncryptedJSON(encryptedJson(element)), key)

    private fun reEncrypt(element: JsonElement, from: SecretKey, to: SecretKey): String {
        if (sameKey(from, to)) return encryptedJson(element)
        return CryptoService.serializeEncryptedJSON(CryptoService.encrypt(decryptField(element, from), to))
    }

    /**
     * Web stores a one-element array holding the tag blob, and older web builds
     * one ciphertext per tag. iOS and old Android store the blob as a string.
     * Always ends as Android's storage format: one blob under the target key.
     */
    private fun tagsFrom(element: JsonElement?, from: SecretKey, to: SecretKey): String {
        val values = when (element) {
            null, JsonNull -> emptyList()
            is JsonArray -> element.map { encryptedJson(it) }
            else -> listOf(encryptedJson(element))
        }.filter { it != "[]" }

        val tags = values.flatMap { value ->
            val text = CryptoService.decryptText(CryptoService.parseEncryptedJSON(value), from)
            val decoded = try {
                json.parseToJsonElement(text)
            } catch (_: Exception) {
                null
            }
            when {
                decoded is JsonArray -> decoded.map { (it as JsonPrimitive).content }
                decoded is JsonPrimitive && decoded.isString -> listOf(decoded.content)
                text.isEmpty() -> emptyList()
                else -> listOf(text)
            }
        }
        return CryptoService.serializeEncryptedJSON(CryptoService.encryptStringArray(tags, to))
    }

    private fun refsFrom(element: JsonElement?, from: SecretKey, to: SecretKey): String {
        val array = when (element) {
            null, JsonNull -> return "[]"
            is JsonArray -> element
            else -> try {
                json.parseToJsonElement((element as JsonPrimitive).content).jsonArray
            } catch (_: Exception) {
                return "[]"
            }
        }
        val refs = array.map { json.decodeFromJsonElement(AttachmentRef.serializer(), it) }
            .map { ref -> ref.copy(filename = reEncryptFilename(ref.filename, from, to)) }
        return json.encodeToString(refs)
    }

    /** Older builds left some filenames in plaintext: encrypt those on the way in. */
    private fun reEncryptFilename(filename: String, from: SecretKey, to: SecretKey): String {
        val encrypted = try {
            CryptoService.parseEncryptedJSON(filename)
        } catch (_: Exception) {
            return CryptoService.serializeEncryptedJSON(CryptoService.encryptText(filename, to))
        }
        if (sameKey(from, to)) return filename
        return CryptoService.serializeEncryptedJSON(
            CryptoService.encrypt(CryptoService.decrypt(encrypted, from), to)
        )
    }

    private fun hashChainFrom(element: JsonElement?): String? = when (element) {
        null, JsonNull -> null
        is JsonArray -> element.toString()
        else -> (element as JsonPrimitive).contentOrNull
    }

    // MARK: - Reading the File

    private enum class ItemType { NOTE, VERSION, ATTACHMENT, SEARCH }

    private fun readHeader(source: BackupSource): BackupHeader {
        var version: String? = null
        var type: String? = null
        var encryption: BackupEncryption? = null

        source.open().use { stream ->
            val reader = JsonReader(InputStreamReader(stream, Charsets.UTF_8))
            try {
                reader.beginObject()
                while (reader.hasNext()) {
                    when (reader.nextName()) {
                        "version" -> version = reader.nextString() // Reads numbers too
                        "type" -> type = reader.nextString()
                        "encryption" -> encryption = readEncryption(reader)
                        else -> reader.skipValue()
                    }
                    if (version != null && type != null && encryption != null) break
                }
            } catch (e: BackupException) {
                throw e
            } catch (_: Exception) {
                throw BackupException.NotABackup()
            }
        }

        val found = version ?: throw BackupException.NotABackup()
        // Android's own format from before D6: an integer version and no header
        if (type == null && found == "3") return BackupHeader(found, null)
        if (type != FORMAT_TYPE) throw BackupException.NotABackup()
        val major = found.substringBefore('.').toIntOrNull() ?: throw BackupException.UnsupportedVersion(found)
        if (major !in 2..3 || (major == 3 && found != FORMAT_VERSION)) {
            throw BackupException.UnsupportedVersion(found)
        }
        return BackupHeader(found, encryption ?: throw BackupException.Corrupt())
    }

    private fun readEncryption(reader: JsonReader): BackupEncryption {
        var envelopeVersion: Int? = null
        var deviceSalt: String? = null
        var localWrappedMaster: String? = null
        var wrappingKdfVersion: Int? = null
        var salt: String? = null
        var iterations: Int? = null
        var createdAt: String? = null
        var algorithm = "AES-256-GCM"

        reader.beginObject()
        while (reader.hasNext()) {
            val name = reader.nextName()
            if (reader.peek() == JsonToken.NULL) {
                reader.nextNull()
                continue
            }
            when (name) {
                "envelopeVersion" -> envelopeVersion = reader.nextInt()
                "deviceSalt" -> deviceSalt = reader.nextString()
                "localWrappedMaster" -> localWrappedMaster = reader.nextString()
                "wrappingKdfVersion" -> wrappingKdfVersion = reader.nextInt()
                "salt" -> salt = reader.nextString()
                "iterations" -> iterations = reader.nextInt()
                "createdAt" -> createdAt = reader.nextString()
                "algorithm" -> algorithm = reader.nextString()
                else -> reader.skipValue()
            }
        }
        reader.endObject()
        return BackupEncryption(
            envelopeVersion, deviceSalt, localWrappedMaster, wrappingKdfVersion,
            salt, iterations, createdAt, algorithm,
        )
    }

    /** Stream every item in the file, one batch in memory at a time. */
    private suspend fun forEachItem(
        source: BackupSource,
        key: SecretKey,
        handle: suspend (ItemType, JsonObject) -> Unit,
    ) {
        source.open().use { stream ->
            val reader = JsonReader(InputStreamReader(stream, Charsets.UTF_8))
            reader.isLenient = false
            try {
                reader.beginObject()
                while (reader.hasNext()) {
                    when (reader.nextName()) {
                        "batches" -> readBatches(reader, key, handle)
                        "data" -> readRecords(reader, key, handle)
                        // Android's format from before D6 stored flat arrays
                        "notes" -> readFlat(reader, ItemType.NOTE, handle)
                        "versions" -> readFlat(reader, ItemType.VERSION, handle)
                        "attachments" -> readFlat(reader, ItemType.ATTACHMENT, handle)
                        "savedSearches" -> readFlat(reader, ItemType.SEARCH, handle)
                        else -> reader.skipValue()
                    }
                }
            } catch (e: BackupException) {
                throw e
            } catch (e: java.io.IOException) {
                throw e
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (e: Exception) {
                Log.w(TAG, "Backup is malformed: ${e.javaClass.simpleName}")
                throw BackupException.Corrupt()
            }
        }
    }

    private suspend fun readBatches(
        reader: JsonReader,
        key: SecretKey,
        handle: suspend (ItemType, JsonObject) -> Unit,
    ) {
        reader.beginArray()
        while (reader.hasNext()) {
            var type: String? = null
            var data: String? = null
            reader.beginObject()
            while (reader.hasNext()) {
                when (reader.nextName()) {
                    "type" -> type = reader.nextString()
                    "data" -> data = reader.nextString()
                    else -> reader.skipValue()
                }
            }
            reader.endObject()

            val itemType = when (type) {
                "notes" -> ItemType.NOTE
                "versions" -> ItemType.VERSION
                "attachments" -> ItemType.ATTACHMENT
                "saved_searches" -> ItemType.SEARCH
                else -> null // Settings and sync_metadata are never adopted from a file
            }
            val jwe = data ?: throw BackupException.Corrupt()
            // Open every batch, including the ones skipped, to prove it's whole
            val items = BackupCodec.decryptBatch(jwe, key)
            if (itemType != null) {
                json.parseToJsonElement(items).jsonArray.forEach { handle(itemType, it.jsonObject) }
            }
        }
        reader.endArray()
    }

    /** Web's v2.1 layout: every record is its own JWE of {type, data}. */
    private suspend fun readRecords(
        reader: JsonReader,
        key: SecretKey,
        handle: suspend (ItemType, JsonObject) -> Unit,
    ) {
        reader.beginArray()
        while (reader.hasNext()) {
            val record = json.parseToJsonElement(BackupCodec.decryptRecord(reader.nextString(), key)).jsonObject
            val itemType = when (record.string("type")) {
                "note" -> ItemType.NOTE
                "version" -> ItemType.VERSION
                "attachment" -> ItemType.ATTACHMENT
                "saved_search" -> ItemType.SEARCH
                else -> null
            }
            if (itemType != null) handle(itemType, record.getValue("data").jsonObject)
        }
        reader.endArray()
    }

    private suspend fun readFlat(
        reader: JsonReader,
        type: ItemType,
        handle: suspend (ItemType, JsonObject) -> Unit,
    ) {
        reader.beginArray()
        while (reader.hasNext()) {
            handle(type, readObject(reader))
        }
        reader.endArray()
    }

    private fun readObject(reader: JsonReader): JsonObject {
        val map = linkedMapOf<String, JsonElement>()
        reader.beginObject()
        while (reader.hasNext()) {
            val name = reader.nextName()
            map[name] = when (reader.peek()) {
                JsonToken.NULL -> {
                    reader.nextNull()
                    JsonNull
                }
                JsonToken.BOOLEAN -> JsonPrimitive(reader.nextBoolean())
                JsonToken.NUMBER -> JsonPrimitive(reader.nextLong())
                JsonToken.STRING -> JsonPrimitive(reader.nextString())
                else -> {
                    reader.skipValue()
                    JsonNull
                }
            }
        }
        reader.endObject()
        return JsonObject(map)
    }

    private fun JsonObject.string(name: String): String? = (this[name] as? JsonPrimitive)?.contentOrNull
    private fun JsonObject.bool(name: String): Boolean? = (this[name] as? JsonPrimitive)?.booleanOrNull
    private fun JsonObject.int(name: String): Int? = (this[name] as? JsonPrimitive)?.intOrNull
}
