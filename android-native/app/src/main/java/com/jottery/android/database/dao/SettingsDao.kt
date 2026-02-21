package com.jottery.android.database.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.jottery.android.model.UserSettings

@Dao
interface SettingsDao {

    @Query("SELECT * FROM settings WHERE id = 1")
    suspend fun get(): UserSettings?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertOrReplace(settings: UserSettings)
}
