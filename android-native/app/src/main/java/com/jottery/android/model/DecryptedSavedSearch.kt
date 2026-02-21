package com.jottery.android.model

/**
 * In-memory decrypted saved search — never persisted.
 */
data class DecryptedSavedSearch(
    val id: String,
    val name: String,
    val query: String,
    val displayOrder: Int,
    val createdAt: String,
    val modifiedAt: String,
)
