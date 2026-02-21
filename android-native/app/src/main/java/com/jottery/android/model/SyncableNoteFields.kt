package com.jottery.android.model

/**
 * Fields needed for content hash computation.
 */
data class SyncableNoteFields(
    val content: String,
    val tags: List<String>,
    val attachmentIds: List<String>,
    val pinned: Boolean,
    val archived: Boolean,
    val locked: Boolean,
    val syntaxLanguage: String?,
    val wordWrap: Boolean,
    val showPreview: Boolean,
    val color: String?,
)
