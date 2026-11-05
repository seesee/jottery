package com.jottery.android.service

import androidx.room.withTransaction
import androidx.test.core.app.ApplicationProvider
import com.jottery.android.crypto.CryptoService
import com.jottery.android.database.JotteryDatabase
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
import com.jottery.android.service.backup.BackupCodec
import com.jottery.android.service.backup.BackupException
import com.jottery.android.service.backup.BackupService
import com.jottery.android.service.backup.BackupSource
import com.jottery.android.service.backup.keyBytes
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.util.Base64
import java.util.UUID
import java.util.zip.Deflater
import javax.crypto.Cipher
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.SecretKeySpec

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class BackupServiceTest {
    private val parser = Json { ignoreUnknownKeys = true }
    private val password = "a-backup-password-${UUID.randomUUID()}"
    private val noteId = "22222222-2222-4222-8222-222222222222"
    private val vaults = mutableListOf<Vault>()

    private inner class Vault {
        val db: JotteryDatabase = JotteryDatabase.buildInMemory(ApplicationProvider.getApplicationContext())
        val notes = NoteRepository(db.noteDao(), db.noteVersionDao())
        val versions = VersionRepository(db.noteVersionDao())
        val attachments = AttachmentRepository(db.attachmentDao())
        val searches = SavedSearchRepository(db.savedSearchDao())
        val encryption = EncryptionRepository(db.encryptionMetadataDao())
        val backup = BackupService(notes, versions, attachments, searches, encryption) { block ->
            db.withTransaction { block() }
        }

        init { vaults += this }

        suspend fun seedLegacy(): SecretKey {
            val salt = CryptoService.generateSalt()
            val key = CryptoService.deriveKey(password, salt, CryptoService.MIN_ITERATIONS)
            encryption.store(
                EncryptionMetadata.new(salt, CryptoService.MIN_ITERATIONS, encrypt("x", key))
            )
            return key
        }

        suspend fun seedEnvelope(): SecretKey {
            val master = CryptoService.generateSalt()
            val deviceSalt = CryptoService.generateSalt()
            val deviceKey = CryptoService.deriveKey(password, deviceSalt, CryptoService.DEFAULT_ITERATIONS)
            encryption.store(
                EncryptionMetadata(
                    createdAt = "2026-01-01T00:00:00.000Z",
                    envelopeVersion = 1,
                    deviceSalt = Base64.getEncoder().encodeToString(deviceSalt),
                    localWrappedMaster = CryptoService.serializeEncryptedJSON(
                        CryptoService.wrapMasterKey(master, deviceKey)
                    ),
                    wrappingKdfVersion = 1,
                )
            )
            return SecretKeySpec(master, "AES")
        }

        suspend fun seedContent(key: SecretKey) {
            val blobId = UUID.randomUUID().toString()
            val blob = encrypt("attachment bytes", key).toByteArray()
            attachments.insertOrReplace(AttachmentRecord(blobId, "", "text/plain", blob.size, blob))
            val ref = AttachmentRef(blobId, encrypt("file.txt", key), "text/plain", 16, blobId)
            val note = note(key, "Secret text", modifiedAt = "2026-02-01T00:00:00.000Z")
                .copy(attachments = Json.encodeToString(listOf(ref)), pinned = true, color = "green")
            notes.insertOrReplace(note)
            versions.insertOrReplace(NoteVersionRecord.from(note.copy(content = encrypt("Old text", key)), "sync"))
            searches.create("Mine", "#mine", key)
        }

        fun note(key: SecretKey, text: String, modifiedAt: String) = NoteRecord(
            id = noteId,
            createdAt = "2026-01-01T00:00:00.000Z",
            modifiedAt = modifiedAt,
            content = encrypt(text, key),
            tags = CryptoService.serializeEncryptedJSON(CryptoService.encryptStringArray(listOf("t1", "t2"), key)),
            version = 2,
            contentHash = "hash",
            hashChain = "[\"hash\"]",
            needsSync = false,
        )

        suspend fun write(key: SecretKey): ByteArray {
            val out = ByteArrayOutputStream()
            backup.write(out, key)
            return out.toByteArray()
        }
    }

    private fun encrypt(text: String, key: SecretKey) =
        CryptoService.serializeEncryptedJSON(CryptoService.encryptText(text, key))

    private fun decrypt(json: String, key: SecretKey) =
        CryptoService.decryptText(CryptoService.parseEncryptedJSON(json), key)

    private fun source(bytes: ByteArray) = BackupSource { ByteArrayInputStream(bytes) }

    private fun fixture(name: String): ByteArray =
        javaClass.classLoader!!.getResourceAsStream("backup/$name")!!.use { it.readBytes() }

    @After fun tearDown() = vaults.forEach { it.db.close() }

    private suspend fun assertFullyRestored(fresh: Vault, key: SecretKey) {
        val note = fresh.notes.get(noteId, key)!!
        assertEquals("Secret text", note.content)
        assertEquals(listOf("t1", "t2"), note.tags)
        assertTrue(note.pinned)
        assertEquals("green", note.color)
        assertEquals("file.txt", note.attachments.single().filename)
        val blob = fresh.attachments.getBlob(note.attachments.single().data)!!
        assertEquals("attachment bytes", decrypt(String(blob), key))
        assertEquals("Old text", decrypt(fresh.versions.getVersions(noteId).single().content, key))
        assertEquals(listOf("Mine"), fresh.searches.listActive(key).map { it.name })
        assertNotNull(fresh.encryption.get())
    }

    @Test fun `a legacy vault's backup restores on a fresh install with only the password`() = runBlocking {
        val original = Vault()
        val key = original.seedLegacy()
        original.seedContent(key)
        val bytes = original.write(key)

        val fresh = Vault()
        val plan = fresh.backup.inspect(source(bytes), password, currentKey = null)
        fresh.backup.restoreAsNewVault(source(bytes), plan)

        assertFullyRestored(fresh, SecretKeySpec(plan.keyBytes(), "AES"))
        val metadata = fresh.encryption.get()!!
        assertEquals(key.encoded.toList(), CryptoService.deriveKey(password, metadata.saltData!!, metadata.iterations!!).encoded.toList())
    }

    @Test fun `an envelope vault's backup restores on a fresh install with only the password`() = runBlocking {
        val original = Vault()
        val key = original.seedEnvelope()
        original.seedContent(key)
        val bytes = original.write(key)

        val fresh = Vault()
        val plan = fresh.backup.inspect(source(bytes), password, currentKey = null)
        fresh.backup.restoreAsNewVault(source(bytes), plan)

        assertEquals(key.encoded.toList(), plan.keyBytes().toList())
        assertFullyRestored(fresh, key)
        assertEquals(1, fresh.encryption.get()!!.envelopeVersion)
    }

    @Test fun `the wrong password is refused and changes nothing`() = runBlocking {
        val original = Vault()
        val key = original.seedLegacy()
        original.seedContent(key)
        val bytes = original.write(key)

        val fresh = Vault()
        val thrown = runCatching { fresh.backup.inspect(source(bytes), "not-the-password", null) }.exceptionOrNull()

        assertTrue(thrown is BackupException.WrongPassword)
        assertNull(fresh.encryption.get())
        assertTrue(fresh.notes.listAllRaw().isEmpty())
    }

    @Test fun `restoring into a vault on another key re-encrypts everything`() = runBlocking {
        val original = Vault()
        val key = original.seedLegacy()
        original.seedContent(key)
        val bytes = original.write(key)

        val other = Vault()
        val otherKey = other.seedEnvelope()
        val plan = other.backup.inspect(source(bytes), password, otherKey)
        val result = other.backup.restore(source(bytes), plan, otherKey)

        assertEquals(1, result.notesRestored)
        assertFullyRestored(other, otherKey)
        assertTrue(other.notes.getRaw(noteId)!!.needsSync)
    }

    @Test fun `a newer local note is kept and an older one is replaced behind a snapshot`() = runBlocking {
        val original = Vault()
        val key = original.seedLegacy()
        original.seedContent(key)
        val bytes = original.write(key)

        val newer = Vault()
        newer.seedLegacy()
        newer.notes.insertOrReplace(newer.note(key, "Newer here", "2026-06-01T00:00:00.000Z"))
        val keepPlan = newer.backup.inspect(source(bytes), password, key)
        val kept = newer.backup.restore(source(bytes), keepPlan, key)
        assertEquals(1, kept.keptNewer)
        assertEquals(1, keepPlan.keptNewer)
        assertEquals("Newer here", newer.notes.get(noteId, key)!!.content)

        val older = Vault()
        older.seedLegacy()
        older.notes.insertOrReplace(older.note(key, "Older here", "2026-01-15T00:00:00.000Z"))
        val replacePlan = older.backup.inspect(source(bytes), password, key)
        older.backup.restore(source(bytes), replacePlan, key)
        assertEquals(1, replacePlan.replaced)
        assertEquals("Secret text", older.notes.get(noteId, key)!!.content)
        val snapshot = older.versions.getVersions(noteId).first { it.reason == "pre-restore" }
        assertEquals("Older here", decrypt(snapshot.content, key))
    }

    @Test fun `a failure partway through a restore writes nothing`() = runBlocking {
        val original = Vault()
        val key = original.seedLegacy()
        original.seedContent(key)
        val good = original.write(key)
        val fresh = Vault()
        val plan = fresh.backup.inspect(source(good), password, null)

        // The file changes between the check and the write
        val text = String(good)
        val damaged = text.replace(Regex("\"type\":\"versions\",\"index\":0,\"count\":1,\"data\":\"[^\"]+\""),
            "\"type\":\"versions\",\"index\":0,\"count\":1,\"data\":\"not.a.valid.token.here\"")
        val thrown = runCatching { fresh.backup.restoreAsNewVault(source(damaged.toByteArray()), plan) }.exceptionOrNull()

        assertTrue(thrown is BackupException)
        assertTrue(fresh.notes.listAllRaw().isEmpty())
        assertNull(fresh.encryption.get())
    }

    @Test fun `Android writes web's format`() = runBlocking {
        val vault = Vault()
        val key = vault.seedLegacy()
        vault.seedContent(key)
        val file = parser.parseToJsonElement(String(vault.write(key))).jsonObject

        assertEquals("3.0", file["version"]!!.jsonPrimitive.content)
        assertEquals("jottery-encrypted-backup", file["type"]!!.jsonPrimitive.content)
        assertNotNull(file["encryption"]!!.jsonObject["salt"])
        assertEquals("1", file["summary"]!!.jsonObject["notes"]!!.jsonPrimitive.content)

        val batch = file["batches"]!!.jsonArray[0].jsonObject
        assertEquals("notes", batch["type"]!!.jsonPrimitive.content)
        val jwe = batch["data"]!!.jsonPrimitive.content
        val parts = jwe.split(".")
        assertEquals(5, parts.size)
        assertEquals("", parts[1])

        // Strict JWE: the header must be the AAD and the payload must be zlib
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        val decoder = Base64.getUrlDecoder()
        cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(128, decoder.decode(parts[2])))
        cipher.updateAAD(parts[0].toByteArray())
        val compressed = cipher.doFinal(decoder.decode(parts[3]) + decoder.decode(parts[4]))
        assertEquals(0x78, compressed[0].toInt() and 0xFF)

        val note = parser.parseToJsonElement(String(BackupCodec.inflate(compressed))).jsonArray[0].jsonObject
        assertTrue("web's tags are an array", note["tags"] is JsonArray)
        assertTrue("web's attachments are an array", note["attachments"] is JsonArray)
        assertTrue("web's hash chain is an array", note["hashChain"] is JsonArray)
    }

    private fun assertWebFixture(name: String) = runBlocking {
        val bytes = fixture(name)
        val fresh = Vault()
        val plan = fresh.backup.inspect(source(bytes), "fixture-password-web", null)
        assertEquals(1, plan.notes)
        assertEquals(1, plan.versions)
        assertEquals(1, plan.attachments)
        assertEquals(1, plan.savedSearches)

        fresh.backup.restoreAsNewVault(source(bytes), plan)

        val key = SecretKeySpec(plan.keyBytes(), "AES")
        val note = fresh.notes.get(noteId, key)!!
        assertEquals("Written on the web", note.content)
        assertEquals(listOf("alpha", "beta"), note.tags)
        assertEquals("photo.png", note.attachments.single().filename)
        assertEquals("python", note.syntaxLanguage)
        assertEquals("red", note.color)
        assertEquals(listOf("hash-web"), note.hashChain)
        val blob = fresh.attachments.getBlob(note.attachments.single().data)!!
        assertEquals("bytes", decrypt(String(blob), key))
        val version = fresh.versions.getVersions(noteId).single()
        assertEquals("Earlier text", decrypt(version.content, key))
        assertEquals(listOf("Work"), fresh.searches.listActive(key).map { it.name })
    }

    @Test fun `a backup written by web restores, from an envelope vault`() = assertWebFixture("web-v3-envelope.json")

    @Test fun `a backup written by web restores, from a legacy vault`() = assertWebFixture("web-v3-legacy.json")

    /** iOS: raw DEFLATE, no AAD, and tags and attachments as JSON strings. */
    private fun iosBatch(items: String, key: SecretKey): String {
        val deflater = Deflater(Deflater.DEFAULT_COMPRESSION, true)
        deflater.setInput(items.toByteArray())
        deflater.finish()
        val buffer = ByteArray(65536)
        val compressed = buffer.copyOf(deflater.deflate(buffer))

        val iv = ByteArray(12) { 7 }
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, key, GCMParameterSpec(128, iv))
        val sealed = cipher.doFinal(compressed)
        val enc = Base64.getUrlEncoder().withoutPadding()
        val header = enc.encodeToString("{\"alg\":\"dir\",\"enc\":\"A256GCM\"}".toByteArray())
        return listOf(
            header, "", enc.encodeToString(iv),
            enc.encodeToString(sealed.copyOfRange(0, sealed.size - 16)),
            enc.encodeToString(sealed.copyOfRange(sealed.size - 16, sealed.size)),
        ).joinToString(".")
    }

    /** A real file from another client, restored onto nothing but its password. */
    private fun restoreFile(file: java.io.File, password: String) = runBlocking {
        val vault = Vault()
        val source = BackupSource { file.inputStream() }
        val plan = vault.backup.inspect(source, password, null)
        assertEquals("every note opens", 0, plan.unreadable)
        vault.backup.restoreAsNewVault(source, plan)
        vault to SecretKeySpec(plan.keyBytes(), "AES")
    }

    @Test fun `the real file written by the iOS app restores completely`() = runBlocking {
        val (vault, key) = restoreFile(
            RepoFile.at("src/test/fixtures/ios-v3.jottery-backup"),
            "notes-password-correct-horse",
        )

        val notes = vault.notes.listAllRaw().map { vault.notes.get(it.id, key)!! }
        assertEquals(2, notes.size)
        val fromIos = notes.single { it.content.contains("From iOS") }
        assertTrue(notes.single { it.content.contains("no tags at all") }.tags.isEmpty())
        assertEquals(1, fromIos.attachments.size)
        val blob = vault.attachments.getBlob(fromIos.attachments[0].data)!!
        val bytes = CryptoService.decrypt(CryptoService.parseEncryptedJSON(String(blob)), key)
        assertTrue(bytes.isNotEmpty())
        assertTrue(fromIos.attachments[0].filename.isNotEmpty())
        assertEquals(1, vault.versions.getVersions(fromIos.id).size)
        val search = vault.searches.listActive(key).single()
        assertEquals("iOS search", search.name)
        assertEquals("#ios", search.query)
    }

    private val tuiBackup by lazy { RepoFile.at("tui/tests/fixtures/tui-v3.jottery-backup") }
    private val tuiPassword by lazy { java.io.File("${tuiBackup.path}.password").readText().trim() }

    /** What the TUI put in its file, as this vault's [key] reads it. */
    private suspend fun assertTuiContents(vault: Vault, key: SecretKey) {
        val notes = vault.notes.listAllRaw().map { vault.notes.get(it.id, key)!! }
        assertEquals(2, notes.size)

        val note = notes.single { it.content.contains("A note from the TUI") }
        assertEquals("A note from the TUI, with a caf\u00e9 and \u65e5\u672c\u8a9e\n", note.content)
        assertEquals(setOf("work", "cross-client"), note.tags.toSet())

        val attachment = note.attachments.single()
        assertEquals("hello.txt", attachment.filename)
        val blob = vault.attachments.getBlob(attachment.data)!!
        assertEquals("attachment body", decrypt(String(blob), key))

        assertTrue(notes.single { it.content.contains("An archived note") }.archived)
    }

    @Test fun `the real file written by the TUI restores completely`() = runBlocking {
        val (vault, key) = restoreFile(tuiBackup, tuiPassword)

        assertTuiContents(vault, key)
    }

    @Test fun `the real file written by the TUI restores into a vault with another key`() = runBlocking {
        val vault = Vault()
        val key = vault.seedLegacy()
        val source = BackupSource { tuiBackup.inputStream() }

        val plan = vault.backup.inspect(source, tuiPassword, key)
        assertEquals(0, plan.unreadable)
        vault.backup.restore(source, plan, key)

        assertTuiContents(vault, key)
    }

    @Test fun `the TUI's file refuses the wrong password`() = runBlocking {
        val source = BackupSource { tuiBackup.inputStream() }

        val thrown = runCatching { Vault().backup.inspect(source, "not-the-password", null) }.exceptionOrNull()

        assertTrue(thrown is BackupException.WrongPassword)
    }

    @Test fun `the files written by this app on a device read back`() = runBlocking {
        val cases = listOf(
            "android-v3-legacy.jottery-backup" to "notes-password-correct-horse",
            "android-v3-envelope.jottery-backup" to "notes-password-battery-staple",
        )
        for ((name, password) in cases) {
            val (vault, key) = restoreFile(
                RepoFile.at("android-native/app/src/test/resources/backup/$name"),
                password,
            )
            val notes = vault.notes.listAllRaw().map { vault.notes.get(it.id, key)!! }
            val main = notes.single { it.content.startsWith("From Android") }
            assertEquals(setOf("android", "fixture"), main.tags.toSet())
            assertEquals("hello.txt", main.attachments.single().filename)
        }
    }

    @Test fun `a backup written by iOS restores`() = runBlocking {
        val salt = CryptoService.generateSalt()
        val key = CryptoService.deriveKey(password, salt, CryptoService.MIN_ITERATIONS)
        val tags = CryptoService.serializeEncryptedJSON(CryptoService.encryptStringArray(listOf("ios"), key))
        val item = kotlinx.serialization.json.buildJsonObject {
            put("id", kotlinx.serialization.json.JsonPrimitive(noteId))
            put("createdAt", kotlinx.serialization.json.JsonPrimitive("2026-01-01T00:00:00.000Z"))
            put("modifiedAt", kotlinx.serialization.json.JsonPrimitive("2026-01-01T00:00:00.000Z"))
            put("content", kotlinx.serialization.json.JsonPrimitive(encrypt("From iOS", key)))
            put("tags", kotlinx.serialization.json.JsonPrimitive(tags))
            put("attachments", kotlinx.serialization.json.JsonPrimitive("[]"))
            put("hashChain", kotlinx.serialization.json.JsonPrimitive("[\"h\"]"))
        }
        val file = """{"version":"3.0","type":"jottery-encrypted-backup","createdAt":"x",
            "encryption":{"salt":"${Base64.getEncoder().encodeToString(salt)}","iterations":${CryptoService.MIN_ITERATIONS},"algorithm":"PBKDF2"},
            "batches":[{"type":"notes","index":0,"count":1,"data":"${iosBatch(JsonArray(listOf(item)).toString(), key)}"}],
            "summary":{"notes":1,"attachments":0,"versions":0,"savedSearches":0}}""".toByteArray()

        val fresh = Vault()
        val plan = fresh.backup.inspect(source(file), password, null)
        fresh.backup.restoreAsNewVault(source(file), plan)

        val note = fresh.notes.get(noteId, key)!!
        assertEquals("From iOS", note.content)
        assertEquals(listOf("ios"), note.tags)
        assertEquals(listOf("h"), note.hashChain)
    }

    /** The file an Android build before this format wrote. */
    private fun oldAndroidFile(key: SecretKey): ByteArray {
        val tags = CryptoService.serializeEncryptedJSON(CryptoService.encryptStringArray(listOf("old"), key))
        val content = encrypt("Old format", key)
        return """{"version":3,"createdAt":"2026-01-01T00:00:00.000Z","notes":[{
            "id":"$noteId","createdAt":"2026-01-01T00:00:00.000Z","modifiedAt":"2026-01-01T00:00:00.000Z",
            "content":${Json.encodeToString(content)},"tags":${Json.encodeToString(tags)},"attachments":"[]",
            "pinned":false,"archived":false,"deleted":false,"version":1}],
            "versions":[],"attachments":[],"savedSearches":[]}""".toByteArray()
    }

    @Test fun `an old Android backup still restores into the vault that made it`() = runBlocking {
        val vault = Vault()
        val key = vault.seedLegacy()
        val bytes = oldAndroidFile(key)

        val plan = vault.backup.inspect(source(bytes), password, key)
        vault.backup.restore(source(bytes), plan, key)

        assertEquals("Old format", vault.notes.get(noteId, key)!!.content)
    }

    @Test fun `an old Android backup is refused on any other vault, with nothing written`() = runBlocking {
        val otherKey = SecretKeySpec(ByteArray(32) { 3 }, "AES")
        val bytes = oldAndroidFile(SecretKeySpec(ByteArray(32) { 4 }, "AES"))

        val vault = Vault()
        val onWrongVault = runCatching { vault.backup.inspect(source(bytes), password, otherKey) }.exceptionOrNull()
        val onFreshInstall = runCatching { vault.backup.inspect(source(bytes), password, null) }.exceptionOrNull()

        assertTrue(onWrongVault is BackupException.KeyNotInBackup)
        assertTrue(onFreshInstall is BackupException.KeyNotInBackup)
        assertTrue(vault.notes.listAllRaw().isEmpty())
    }

    @Test fun `files that aren't backups are refused`() {
        val vault = Vault()
        for (bytes in listOf("{}".toByteArray(), "not json".toByteArray(), """{"version":"9.0","type":"jottery-encrypted-backup","encryption":{}}""".toByteArray())) {
            val thrown = runCatching { runBlocking { vault.backup.inspect(source(bytes), password, null) } }.exceptionOrNull()
            assertTrue("$thrown", thrown is BackupException)
        }
    }
}
