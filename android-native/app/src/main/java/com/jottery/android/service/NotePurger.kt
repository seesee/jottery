package com.jottery.android.service

import com.jottery.android.database.repository.AttachmentRepository
import com.jottery.android.database.repository.NoteRepository
import com.jottery.android.database.repository.SyncRepository
import com.jottery.android.database.repository.VersionRepository
import com.jottery.android.model.NoteDeletion
import com.jottery.android.util.DateUtils
import java.time.Duration
import java.time.Instant

/**
 * The one way a note leaves this device for good, whether the user asked or its
 * time in the bin ran out. In one transaction: a tombstone for the other devices,
 * the note, its versions, any conflict on it, and blobs nothing else refers to.
 * A kill can't then leave a note gone with no tombstone, or a blob behind.
 */
class NotePurger(
    private val noteRepo: NoteRepository,
    private val versionRepo: VersionRepository,
    private val attachmentRepo: AttachmentRepository,
    private val syncRepo: SyncRepository,
    private val transaction: suspend (suspend () -> Unit) -> Unit,
) {
    /** What the user removed from the bin. The tombstone is dated now. */
    suspend fun deletePermanently(ids: Collection<String>) {
        val now = DateUtils.nowISO8601()
        remove(ids.map { NoteDeletion(it, now) })
    }

    /**
     * Notes that have been in the bin for [olderThan], going by this device's own
     * clock: the time in a note can be set by the server. Their tombstones carry
     * the time they were binned, so an edit made since on another device still wins.
     * Returns how many were removed.
     */
    suspend fun purgeExpired(olderThan: Duration = BIN_RETENTION): Int {
        noteRepo.startBinClocks()
        val cutoff = DateUtils.formatISO8601(Instant.now().minus(olderThan))
        val expired = noteRepo.listExpiredDeleted(cutoff)
        remove(expired.map { NoteDeletion(it.id, it.deletedAt ?: DateUtils.nowISO8601()) })
        return expired.size
    }

    private suspend fun remove(tombstones: List<NoteDeletion>) {
        if (tombstones.isEmpty()) return
        transaction {
            val candidates = mutableSetOf<String>()
            for (tombstone in tombstones) {
                candidates += noteRepo.blobIdsOf(tombstone.id)
                candidates += versionRepo.blobIdsOf(tombstone.id)
                syncRepo.addDeletion(tombstone)
                syncRepo.clearConflict(tombstone.id)
                versionRepo.deleteByNoteId(tombstone.id)
                noteRepo.hardDelete(tombstone.id)
            }
            deleteUnreferencedBlobs(candidates)
        }
    }

    /** Delete those of [candidates] that no note or version refers to any more. */
    suspend fun deleteUnreferencedBlobs(candidates: Set<String>) {
        if (candidates.isEmpty()) return
        val stillUsed = noteRepo.allBlobIds() + versionRepo.allBlobIds()
        (candidates - stillUsed).forEach { attachmentRepo.delete(it) }
    }

    companion object {
        val BIN_RETENTION: Duration = Duration.ofDays(30)
    }
}
