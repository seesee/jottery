package com.jottery.android.database.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.jottery.android.model.EncryptionMetadata

@Dao
interface EncryptionMetadataDao {

    @Query("SELECT * FROM encryption_metadata WHERE id = 1")
    suspend fun get(): EncryptionMetadata?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertOrReplace(metadata: EncryptionMetadata)
}
