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
    val decryptedAt: Instant = Instant.now(),
) {
    /** Title from virtual tag (t: or title:) or first non-empty line. */
    val title: String
        get() {
            // Check for virtual title tag
            val titleTag = tags.firstOrNull { it.startsWith("t:") || it.startsWith("title:") }
            if (titleTag != null) {
                val prefix = if (titleTag.startsWith("t:")) "t:" else "title:"
                val tagTitle = titleTag.removePrefix(prefix).trim()
                if (tagTitle.isNotEmpty()) return tagTitle.take(80)
            }
            // Fall back to first non-empty line of content
            val lines = content.split("\n").filter { it.isNotBlank() }
            val firstLine = lines.firstOrNull() ?: return "Untitled"
            val trimmed = firstLine.trim().replace(Regex("^#+\\s*"), "")
            if (trimmed.isEmpty()) return "Untitled"
            return trimmed.take(80)
        }

    /** Preview snippet: second non-empty line, truncated. */
    val preview: String
        get() {
            val lines = content.split("\n").filter { it.isNotBlank() }
            if (lines.size <= 1) return ""
            return lines[1].trim().take(120)
        }

    /** Tags excluding virtual tags (e.g. title tags). */
    val regularTags: List<String>
        get() = tags.filter { !isTitleTag(it) }

    /** Word count for the note. */
    val wordCount: Int
        get() = content.split(Regex("[\\s\\n]+")).count { it.isNotEmpty() }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is DecryptedNote) return false
        return id == other.id && modifiedAt == other.modifiedAt && version == other.version
    }

    override fun hashCode(): Int = id.hashCode()

    companion object {
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
