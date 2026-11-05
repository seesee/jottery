package com.jottery.android.ui.editor

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class EditorSearchTest {
    @Test fun `matching ignores case and doesn't overlap`() {
        assertEquals(listOf(0..1, 2..3), EditorSearch.matches("aAaa"[0].toString() + "Aaa", "aa"))
    }

    @Test fun `an empty query matches nothing`() {
        assertTrue(EditorSearch.matches("text", "").isEmpty())
    }

    @Test fun `next and previous go round`() {
        val matches = EditorSearch.matches("x x x", "x")

        assertEquals(1, EditorSearch.next(matches, 1))
        assertEquals(0, EditorSearch.next(matches, 5))
        assertEquals(1, EditorSearch.previous(matches, 4))
        assertEquals(2, EditorSearch.previous(matches, 0))
    }

    @Test fun `replace all keeps every position right, even as lengths change`() {
        assertEquals("cat and cat, CAT no more".replace("CAT", "cat"), EditorSearch.replaceAll("dog and dog, DOG no more", "dog", "cat"))
    }

    @Test fun `a replacement containing the query doesn't run away`() {
        assertEquals("aa aa", EditorSearch.replaceAll("a a", "a", "aa"))
    }

    @Test fun `replace changes one match only`() {
        val text = "one two one"
        val second = EditorSearch.matches(text, "one")[1]

        assertEquals("one two 1", EditorSearch.replace(text, second, "1"))
    }
}
