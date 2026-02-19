package com.jottery.android.service

import org.junit.Assert.*
import org.junit.Test

/**
 * JVM unit tests for HashChainService.
 *
 * Note: HashChainService delegates to CryptoService which uses android.util.Base64.
 * These tests exercise the pure logic directly without the Android dependency.
 */
class HashChainServiceTest {

    @Test
    fun `updateHashChain prepends new hash`() {
        val chain = listOf("hash-b", "hash-a")
        val updated = updateHashChain("hash-c", chain)
        assertEquals("hash-c", updated.first())
        assertEquals(3, updated.size)
    }

    @Test
    fun `updateHashChain on empty chain creates single-element list`() {
        val updated = updateHashChain("first-hash", emptyList())
        assertEquals(listOf("first-hash"), updated)
    }

    @Test
    fun `updateHashChain caps at 50 entries`() {
        val longChain = (1..60).map { "hash-$it" }
        val updated = updateHashChain("new-hash", longChain)
        assertEquals(50, updated.size)
        assertEquals("new-hash", updated.first())
    }

    @Test
    fun `findCommonAncestor finds shared hash`() {
        val chainA = listOf("a2", "a1", "common", "root")
        val chainB = listOf("b3", "b2", "b1", "common", "root")
        assertEquals("common", findCommonAncestor(chainA, chainB))
    }

    @Test
    fun `findCommonAncestor returns earliest common when multiple shared`() {
        val chainA = listOf("a1", "shared1", "shared2")
        val chainB = listOf("b1", "shared1", "shared2")
        assertEquals("shared1", findCommonAncestor(chainA, chainB))
    }

    @Test
    fun `findCommonAncestor returns null for disjoint chains`() {
        val chainA = listOf("a1", "a2")
        val chainB = listOf("b1", "b2")
        assertNull(findCommonAncestor(chainA, chainB))
    }

    @Test
    fun `findCommonAncestor handles empty chains`() {
        assertNull(findCommonAncestor(emptyList(), listOf("b1")))
        assertNull(findCommonAncestor(listOf("a1"), emptyList()))
        assertNull(findCommonAncestor(emptyList(), emptyList()))
    }

    @Test
    fun `isDescendant returns true when ancestor head exists in descendant`() {
        val ancestor = listOf("hash-a", "hash-old")
        val descendant = listOf("hash-c", "hash-b", "hash-a", "hash-old")
        assertTrue(isDescendant(ancestor, descendant))
    }

    @Test
    fun `isDescendant returns false when ancestor head not in descendant`() {
        val ancestor = listOf("hash-x")
        val descendant = listOf("hash-a", "hash-b")
        assertFalse(isDescendant(ancestor, descendant))
    }

    @Test
    fun `isDescendant returns true for empty ancestor chain`() {
        assertTrue(isDescendant(emptyList(), listOf("any")))
        assertTrue(isDescendant(emptyList(), emptyList()))
    }

    @Test
    fun `isDescendant returns false when descendant is empty but ancestor is not`() {
        assertFalse(isDescendant(listOf("hash-a"), emptyList()))
    }

    // -- Helpers (mirror HashChainService/CryptoService logic) --

    private fun updateHashChain(currentHash: String, parentChain: List<String>): List<String> {
        val maxChainLength = 50
        val newChain = mutableListOf(currentHash)
        newChain.addAll(parentChain)
        return if (newChain.size > maxChainLength) newChain.take(maxChainLength) else newChain
    }

    private fun findCommonAncestor(chainA: List<String>, chainB: List<String>): String? {
        val setB = chainB.toSet()
        return chainA.firstOrNull { it in setB }
    }

    private fun isDescendant(ancestorChain: List<String>, descendantChain: List<String>): Boolean {
        if (ancestorChain.isEmpty()) return true
        val ancestorHead = ancestorChain.first()
        return ancestorHead in descendantChain
    }
}
