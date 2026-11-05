package com.jottery.android.util

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * What a release build may log. Warnings and errors reach bug reports, and an
 * exception's message can quote a server's reply, or a note the JSON parser choked on.
 */
class ReleaseLoggingTest {
    private val sources = File("src/main/java").walkTopDown().filter { it.extension == "kt" }.toList()

    @Test fun `a warning or error never logs an exception's message`() {
        val offending = Regex("""Log\.[we]\(.*\.message""")
        val found = sources.flatMap { file ->
            file.readLines().mapIndexedNotNull { index, line ->
                "${file.name}:${index + 1}".takeIf { offending.containsMatchIn(line) }
            }
        }
        assertEquals(emptyList<String>(), found)
    }


    @Test fun `R8 strips verbose, debug and info logging`() {
        val rules = File("proguard-rules.pro").readText()
        val block = rules.substringAfter("-assumenosideeffects class android.util.Log {").substringBefore("}")
        for (level in listOf("v", "d", "i")) assertTrue(level, "public static int $level(...);" in block)
    }
}
