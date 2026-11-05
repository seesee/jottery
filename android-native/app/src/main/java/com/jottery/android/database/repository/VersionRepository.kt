package com.jottery.android.database.repository

import com.jottery.android.database.dao.NoteVersionDao
import com.jottery.android.model.AttachmentRef
import com.jottery.android.model.NoteVersionRecord
import kotlinx.serialization.json.Json

private const val READ_CHUNK = 512 * 1024

/**
 * Repository for note version snapshots.
 */
class VersionRepository(
    private val dao: NoteVersionDao,
) {
    private val json = Json { ignoreUnknownKeys = true }

    suspend fun getVersions(noteId: String): List<NoteVersionRecord> =
        dao.getVersions(noteId).map { hydrate(it) }

    suspend fun listAll(): List<NoteVersionRecord> = dao.listAll().map { hydrate(it) }

    suspend fun getByKey(versionKey: String): NoteVersionRecord? =
        dao.getByKey(versionKey)?.let { hydrate(it) }

    /** Fill in content too large to read with the row: see INLINE_CONTENT_LIMIT. */
    private suspend fun hydrate(version: NoteVersionRecord): NoteVersionRecord {
        if (version.content.isNotEmpty()) return version
        val length = dao.contentLength(version.versionKey) ?: return version
        val content = StringBuilder(length)
        var start = 1
        while (start <= length) {
            content.append(dao.contentChunk(version.versionKey, start, READ_CHUNK) ?: break)
            start += READ_CHUNK
        }
        return version.copy(content = content.toString())
    }

    suspend fun insertOrReplace(version: NoteVersionRecord) =
        dao.insertOrReplace(version)

    suspend fun insertOrReplaceAll(versions: List<NoteVersionRecord>) =
        dao.insertOrReplaceAll(versions)

    suspend fun deleteByNoteId(noteId: String) =
        dao.deleteByNoteId(noteId)

    /** The blobs one note's versions refer to. */
    suspend fun blobIdsOf(noteId: String): Set<String> =
        dao.listAttachments(noteId).flatMapTo(mutableSetOf(), ::blobIds)

    /** The blobs any version refers to. */
    suspend fun allBlobIds(): Set<String> =
        dao.listAllAttachments().flatMapTo(mutableSetOf(), ::blobIds)

    private fun blobIds(attachments: String): List<String> = try {
        json.decodeFromString<List<AttachmentRef>>(attachments).map { it.data }
    } catch (_: Exception) {
        emptyList()
    }
}
