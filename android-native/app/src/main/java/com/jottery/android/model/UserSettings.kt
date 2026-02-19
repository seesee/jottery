package com.jottery.android.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

/**
 * User preferences. Singleton table (id=1).
 */
@Entity(tableName = "settings")
data class UserSettings(
    @PrimaryKey
    val id: Int = 1,

    val language: String = "system",

    val theme: String = "auto",

    @ColumnInfo(name = "sort_order")
    val sortOrder: String = "recent",

    @ColumnInfo(name = "auto_lock_timeout")
    val autoLockTimeout: Int = 15,

    @ColumnInfo(name = "sync_enabled")
    val syncEnabled: Boolean = false,

    @ColumnInfo(name = "sync_endpoint")
    val syncEndpoint: String? = null,

    @ColumnInfo(name = "color_palette")
    val colorPalette: String? = null,

    @ColumnInfo(name = "tag_colors")
    val tagColors: String? = null,
) {
    val themeMode: ThemeMode
        get() = ThemeMode.fromString(theme)

    val sort: SortOrder
        get() = SortOrder.fromString(sortOrder)

    companion object {
        val defaults = UserSettings(
            language = "system",
            theme = "auto",
            sortOrder = "recent",
            autoLockTimeout = 15,
            syncEnabled = false,
            syncEndpoint = null,
        )
    }
}

enum class ThemeMode(val value: String) {
    LIGHT("light"),
    DARK("dark"),
    AUTO("auto");

    companion object {
        fun fromString(value: String): ThemeMode =
            entries.firstOrNull { it.value == value } ?: AUTO
    }
}

enum class SortOrder(val value: String, val displayName: String) {
    RECENT("recent", "Most Recent"),
    OLDEST("oldest", "Oldest First"),
    ALPHA("alpha", "Alphabetical"),
    CREATED("created", "Date Created");

    companion object {
        fun fromString(value: String): SortOrder =
            entries.firstOrNull { it.value == value } ?: RECENT
    }
}
