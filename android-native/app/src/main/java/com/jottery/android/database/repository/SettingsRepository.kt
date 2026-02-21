package com.jottery.android.database.repository

import com.jottery.android.database.dao.SettingsDao
import com.jottery.android.model.UserSettings

/**
 * Repository for user settings (singleton table).
 */
class SettingsRepository(
    private val dao: SettingsDao,
) {
    suspend fun get(): UserSettings = dao.get() ?: UserSettings.defaults

    suspend fun save(settings: UserSettings) = dao.insertOrReplace(settings)
}
