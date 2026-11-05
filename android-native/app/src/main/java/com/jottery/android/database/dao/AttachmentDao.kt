package com.jottery.android.database.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.jottery.android.model.AttachmentRecord

@Dao
interface AttachmentDao {

    @Query("SELECT length(data) FROM attachments WHERE id = :id")
    suspend fun blobLength(id: String): Int?

    /**
     * Blobs are read in pieces: one row over the 2 MB CursorWindow can be written
     * but never read back. [start] counts from 1, as SQLite's substr does.
     */
    @Query("SELECT substr(data, :start, :length) FROM attachments WHERE id = :id")
    suspend fun blobChunk(id: String, start: Int, length: Int): ByteArray?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertOrReplace(attachment: AttachmentRecord)

    @Query("DELETE FROM attachments WHERE id = :id")
    suspend fun delete(id: String)

    @Query("SELECT id FROM attachments")
    suspend fun listBlobIds(): List<String>

    @Query("UPDATE attachments SET data = :data, uploaded = 0 WHERE id = :id")
    suspend fun updateBlobData(id: String, data: ByteArray)

    @Query("SELECT id FROM attachments WHERE uploaded = 1")
    suspend fun listUploadedIds(): List<String>

    @Query("UPDATE attachments SET uploaded = 1 WHERE id = :id")
    suspend fun markUploaded(id: String)

    @Query("UPDATE attachments SET uploaded = 0")
    suspend fun markAllNotUploaded()

    /**
     * Blank up to [limit] of the plaintext filenames earlier builds left here, and say
     * how many that was. Each row's data is rewritten with it, so this is a few rows at
     * a time, never one statement over the table.
     */
    @Query(
        "UPDATE attachments SET filename = '' WHERE id IN " +
            "(SELECT id FROM attachments WHERE filename != '' LIMIT :limit)"
    )
    suspend fun clearOldFilenames(limit: Int): Int
}
