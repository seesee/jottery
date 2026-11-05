package com.jottery.android.model

import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

/**
 * The names given to colours and the tags that carry them, in the same settings
 * columns as the web. They belong to this device: no client syncs them.
 */
data class NoteCategories(
    /** Colour key to the name the user gave it. */
    val names: Map<String, String> = emptyMap(),
    /** Lower-case tag to colour key. */
    val tagColours: Map<String, String> = emptyMap(),
) {
    fun colourOfTag(tag: String): String? = tagColours[normalise(tag)]

    fun named(colour: String, name: String): NoteCategories = copy(
        names = if (name.isBlank()) names - colour else names + (colour to name.trim()),
    )

    fun withTag(tag: String, colour: String?): NoteCategories = copy(
        tagColours = if (colour == null) tagColours - normalise(tag) else tagColours + (normalise(tag) to colour),
    )

    /** Applied to [settings], leaving its other fields alone. */
    fun appliedTo(settings: UserSettings): UserSettings = settings.copy(
        colorPalette = names.takeIf { it.isNotEmpty() }?.let { names ->
            json.encodeToString(names.mapValues { PaletteEntry(displayName = it.value) })
        },
        tagColors = tagColours.takeIf { it.isNotEmpty() }?.let { json.encodeToString(it) },
    )

    /** The web also keeps hex codes here. They aren't read, but mustn't stop a name being found. */
    @Serializable
    private data class PaletteEntry(val displayName: String? = null)

    companion object {
        private val json = Json { ignoreUnknownKeys = true }

        private fun normalise(tag: String) = tag.trim().lowercase()

        /** Whatever is stored, a problem with it costs a label and no more. */
        fun from(settings: UserSettings): NoteCategories {
            val palette = try {
                settings.colorPalette?.let { json.decodeFromString<Map<String, PaletteEntry>>(it) }.orEmpty()
            } catch (_: Exception) {
                emptyMap()
            }
            val tags = try {
                settings.tagColors?.let { json.decodeFromString<Map<String, String>>(it) }.orEmpty()
            } catch (_: Exception) {
                emptyMap()
            }
            return NoteCategories(
                names = palette.mapNotNull { (key, entry) -> entry.displayName?.let { key to it } }.toMap(),
                tagColours = tags,
            )
        }
    }
}
