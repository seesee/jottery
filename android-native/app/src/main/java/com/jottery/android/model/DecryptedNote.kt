package com.jottery.android.model

import java.time.Instant

/**
 * In-memory decrypted note — never persisted.
 * Created by NoteRepository from a NoteRecord.
 */
data class DecryptedNote(
    val id: String,
    var createdAt: Instant,
    var modifiedAt: Instant,
    var syncedAt: Instant? = null,
    var content: String,
    var tags: List<String>,
    var attachments: List<AttachmentRef>,
    var pinned: Boolean,
    var archived: Boolean,
    var archivedAt: Instant? = null,
    var locked: Boolean,
    var lockedAt: Instant? = null,
    var deleted: Boolean,
    var deletedAt: Instant? = null,
    var version: Int,
    var wordWrap: Boolean,
    var syntaxLanguage: String,
    var showPreview: Boolean,
    var color: String? = null,
    var contentHash: String? = null,
    var parentHash: String? = null,
    var hashChain: List<String> = emptyList(),
    var needsSync: Boolean,
) {
    /**
     * Title from virtual tag (t: or title:) or first non-empty line. Sorting asks
     * for this on every comparison, so it's worked out once: an edit makes a new
     * instance, so nothing here can go stale.
     */
    val title: String by lazy(LazyThreadSafetyMode.PUBLICATION) {
        val titleTag = tags.firstOrNull { it.startsWith("t:") || it.startsWith("title:") }
        if (titleTag != null) {
            val prefix = if (titleTag.startsWith("t:")) "t:" else "title:"
            val tagTitle = titleTag.removePrefix(prefix).trim()
            if (tagTitle.isNotEmpty()) return@lazy tagTitle.take(80)
        }
        val firstLine = nonBlankLines.firstOrNull() ?: return@lazy untitled
        val trimmed = firstLine.trim().replace(HEADING_MARK, "")
        if (trimmed.isEmpty()) untitled else trimmed.take(80)
    }

    /** Preview snippet: second non-empty line, truncated. */
    val preview: String by lazy(LazyThreadSafetyMode.PUBLICATION) {
        nonBlankLines.getOrNull(1)?.trim()?.take(120).orEmpty()
    }

    /** The first two lines are all a title and preview need. */
    private val nonBlankLines: List<String>
        get() = content.lineSequence().filter { it.isNotBlank() }.take(2).toList()

    /** Lowercase once per note, not once per search. */
    val searchableContent: String by lazy(LazyThreadSafetyMode.PUBLICATION) { content.lowercase() }

    val searchableTags: List<String> by lazy(LazyThreadSafetyMode.PUBLICATION) { tags.map { it.lowercase() } }

    /** Tags excluding virtual tags (e.g. title tags). */
    val regularTags: List<String>
        get() = tags.filter { !isTitleTag(it) }

    /** Word count for the note. */
    val wordCount: Int by lazy(LazyThreadSafetyMode.PUBLICATION) {
        content.split(WORD_BREAK).count { it.isNotEmpty() }
    }

    companion object {
        private val HEADING_MARK = Regex("^#+\\s*")
        private val WORD_BREAK = Regex("[\\s\\n]+")

        /** Shown for a note with no text. Set from resources when the app starts. */
        @Volatile var untitled: String = "Untitled"

        /** Whether a tag is a virtual title tag (t: or title: prefix). */
        fun isTitleTag(tag: String): Boolean =
            tag.startsWith("t:") || tag.startsWith("title:")

        /** Extract the value from a title tag, or null if not a title tag. */
        fun titleTagValue(tag: String): String? {
            for (prefix in listOf("t:", "title:")) {
                if (tag.startsWith(prefix)) {
                    return tag.removePrefix(prefix).trim()
                }
            }
            return null
        }
    }
}

/** Each tag and how many of these notes carry it, most used first. */
fun List<DecryptedNote>.tagCounts(): List<Pair<String, Int>> =
    flatMap { it.regularTags.distinct() }
        .groupingBy { it }
        .eachCount()
        .toList()
        .sortedWith(compareByDescending<Pair<String, Int>> { it.second }.thenBy { it.first.lowercase() })
