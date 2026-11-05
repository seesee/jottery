package com.jottery.android.database.repository

import com.jottery.android.database.dao.NoteVersionDao
import com.jottery.android.model.NoteVersionRecord

private const val READ_CHUNK = 512 * 1024

/**
 * Repository for note version snapshots.
 */
class VersionRepository(
    private val dao: NoteVersionDao,
) {
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

}
