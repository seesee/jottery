package com.jottery.android.database.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.jottery.android.model.AttachmentRecord

@Dao
interface AttachmentDao {

    @Query("SELECT * FROM attachments WHERE id = :id")
    suspend fun getById(id: String): AttachmentRecord?

    @Query("SELECT data FROM attachments WHERE id = :id")
    suspend fun getBlob(id: String): ByteArray?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertOrReplace(attachment: AttachmentRecord)

    @Query("DELETE FROM attachments WHERE id = :id")
    suspend fun delete(id: String)

    @Query("SELECT id FROM attachments")
    suspend fun listBlobIds(): List<String>

    @Query("UPDATE attachments SET data = :data WHERE id = :id")
    suspend fun updateBlobData(id: String, data: ByteArray)
}
