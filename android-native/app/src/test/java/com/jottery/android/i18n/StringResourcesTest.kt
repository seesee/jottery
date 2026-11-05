package com.jottery.android.i18n

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.w3c.dom.Element
import java.io.File
import javax.xml.parsers.DocumentBuilderFactory

/**
 * Every language has to carry every string, and a translated format string has to
 * take the same arguments, because a mismatch only shows as a crash on that
 * language's phones. Android's own check for this is lint, which CI doesn't run.
 */
class StringResourcesTest {
    private val res = File("src/main/res")
    private val placeholder = Regex("""%\d+\$[sd]""")

    /** Quantities each language needs beyond English's one and other, or instead of them. */
    private val quantities = mapOf(
        "ru" to setOf("one", "few", "many", "other"),
        "pl" to setOf("one", "few", "many", "other"),
        "ja" to setOf("other"),
        "ko" to setOf("other"),
        "zh" to setOf("other"),
    )

    private data class Strings(val strings: Map<String, String>, val plurals: Map<String, Map<String, String>>)

    private fun load(folder: String): Strings {
        val root = DocumentBuilderFactory.newInstance().newDocumentBuilder()
            .parse(File(res, "$folder/strings.xml")).documentElement
        fun elements(tag: String, parent: Element = root) =
            parent.getElementsByTagName(tag).let { list -> (0 until list.length).map { list.item(it) as Element } }
        return Strings(
            strings = elements("string").filter { it.getAttribute("translatable") != "false" }
                .associate { it.getAttribute("name") to it.textContent },
            plurals = elements("plurals").associate { plural ->
                plural.getAttribute("name") to elements("item", plural).associate { it.getAttribute("quantity") to it.textContent }
            },
        )
    }

    private val english by lazy { load("values") }

    private val languages by lazy {
        res.list()!!.filter { it.startsWith("values-") && it != "values-en-rUS" && File(res, "$it/strings.xml").exists() }.sorted()
    }

    private fun args(text: String) = placeholder.findAll(text).map { it.value }.toSet()

    @Test fun `the languages match the web's`() {
        val web = File("../../src/locales").list()!!
            .filter { it.endsWith(".json") }.map { it.removeSuffix(".json") }
            .filterNot { it.startsWith("en-") }.map { "values-$it" }.sorted()
        assertEquals(web, languages)
    }

    @Test fun `Android offers exactly the languages it has`() {
        val offered = Regex("""android:name="([^"]+)"""").findAll(File(res, "xml/locales_config.xml").readText())
            .map { it.groupValues[1] }.toSet()
        assertEquals(languages.map { it.removePrefix("values-") }.toSet() + setOf("en-GB", "en-US"), offered)
    }

    @Test fun `every language has every string, and nothing else`() {
        for (language in languages) {
            val strings = load(language)
            assertEquals(language, english.strings.keys, strings.strings.keys)
            assertEquals(language, english.plurals.keys, strings.plurals.keys)
        }
    }

    @Test fun `a translation takes the arguments its English does`() {
        for (language in languages) {
            val strings = load(language)
            for ((name, text) in english.strings) {
                assertEquals("$language $name", args(text), args(strings.strings.getValue(name)))
            }
            for ((name, forms) in english.plurals) {
                for ((quantity, text) in strings.plurals.getValue(name)) {
                    assertEquals("$language $name $quantity", args(forms.getValue("other")), args(text))
                }
            }
        }
    }

    @Test fun `every language has the plural forms its grammar needs`() {
        for (language in languages) {
            val needed = quantities[language.removePrefix("values-")] ?: setOf("one", "other")
            for ((name, forms) in load(language).plurals) {
                assertTrue("$language $name has ${forms.keys}", forms.keys.containsAll(needed))
            }
        }
    }

    @Test fun `aapt won't trim a sentence that is joined to another`() {
        for (folder in languages + "values") {
            for ((name, text) in load(folder).strings) {
                assertEquals("$folder $name", text.trim(), text)
            }
        }
    }

    @Test fun `American English overrides every British spelling`() {
        val american = load("values-en-rUS").strings
        for ((name, text) in english.strings) {
            if (Regex("colour|organis|customis|recognis|cancelled", RegexOption.IGNORE_CASE).containsMatchIn(text)) {
                assertTrue("$name needs an American spelling", name in american)
            }
        }
        assertTrue(american.values.none { it.contains("colour", ignoreCase = true) })
    }
}
