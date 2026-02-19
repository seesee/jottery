package com.jottery.android.database.repository

import com.jottery.android.database.dao.NoteVersionDao
import com.jottery.android.model.NoteVersionRecord

/**
 * Repository for note version snapshots.
 */
class VersionRepository(
    private val dao: NoteVersionDao,
) {
    suspend fun getVersions(noteId: String): List<NoteVersionRecord> =
        dao.getVersions(noteId)

    suspend fun getByKey(versionKey: String): NoteVersionRecord? =
        dao.getByKey(versionKey)

    suspend fun insertOrReplace(version: NoteVersionRecord) =
        dao.insertOrReplace(version)

    suspend fun insertOrReplaceAll(versions: List<NoteVersionRecord>) =
        dao.insertOrReplaceAll(versions)

    suspend fun deleteByNoteId(noteId: String) =
        dao.deleteByNoteId(noteId)

    suspend fun listNeedingSync(): List<NoteVersionRecord> =
        dao.listNeedingSync()
}
