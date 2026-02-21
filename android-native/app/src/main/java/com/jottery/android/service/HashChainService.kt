package com.jottery.android.service

import com.jottery.android.crypto.CryptoService
import com.jottery.android.model.SyncableNoteFields

/**
 * Hash chain management for sync conflict detection.
 * Delegates to CryptoService for the actual hash computations.
 */
object HashChainService {

    /** Compute the content hash for a note's current state. */
    fun computeContentHash(fields: SyncableNoteFields): String {
        return CryptoService.computeContentHash(fields)
    }

    /** Update hash chain by prepending the new hash. */
    fun updateHashChain(currentHash: String, parentChain: List<String>): List<String> {
        return CryptoService.updateHashChain(currentHash, parentChain)
    }

    /** Find a common ancestor between two hash chains. */
    fun findCommonAncestor(chainA: List<String>, chainB: List<String>): String? {
        return CryptoService.findCommonAncestor(chainA, chainB)
    }

    /** Determine if chain B is a descendant of chain A (A's head is in B). */
    fun isDescendant(ancestorChain: List<String>, descendantChain: List<String>): Boolean {
        if (ancestorChain.isEmpty()) return true
        val ancestorHead = ancestorChain.first()
        return ancestorHead in descendantChain
    }
}
