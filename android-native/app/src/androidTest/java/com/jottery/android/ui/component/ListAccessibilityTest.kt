package com.jottery.android.ui.component

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.test.assertHeightIsAtLeast
import androidx.compose.ui.test.assertTouchHeightIsEqualTo
import androidx.compose.ui.test.assertTouchWidthIsEqualTo
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.jottery.android.model.DecryptedNote
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import java.time.Instant

/** What TalkBack and large text get, read from the real semantics tree. */
@RunWith(AndroidJUnit4::class)
class ListAccessibilityTest {
    @get:Rule val rule = createComposeRule()

    private fun note(color: String? = null) = DecryptedNote(
        id = "id",
        createdAt = Instant.EPOCH,
        modifiedAt = Instant.EPOCH,
        content = "A title\nAnd a line of text under it",
        tags = emptyList(),
        attachments = emptyList(),
        pinned = false,
        archived = false,
        locked = false,
        deleted = false,
        version = 1,
        wordWrap = true,
        syntaxLanguage = "markdown",
        showPreview = false,
        color = color,
        needsSync = false,
    )

    private fun row() = rule.onNodeWithTag("row")

    @Test fun whatASwipeDoesIsOfferedAsAnAction() {
        var pinned = false
        rule.setContent {
            NoteRowView(
                note = note(),
                isSelected = false,
                isMultiSelectMode = false,
                isChecked = false,
                onClick = {},
                onLongClick = {},
                modifier = Modifier.testTag("row"),
                accessibilityActions = listOf(CustomAccessibilityAction("Pin") { pinned = true; true }),
            )
        }

        val actions = row().fetchSemanticsNode().config[SemanticsActions.CustomActions]
        assertEquals(listOf("Pin"), actions.map { it.label })
        rule.runOnIdle { actions.single().action() }
        assertTrue(pinned)
    }

    @Test fun aRowInSelectionAnnouncesItsState() {
        rule.setContent {
            NoteRowView(
                note = note(),
                isSelected = false,
                isMultiSelectMode = true,
                isChecked = true,
                onClick = {},
                onLongClick = {},
                modifier = Modifier.testTag("row"),
            )
        }

        val config = row().fetchSemanticsNode().config
        assertEquals(ToggleableState.On, config[SemanticsProperties.ToggleableState])
        assertEquals(Role.Checkbox, config[SemanticsProperties.Role])
    }

    @Test fun anOrdinaryRowClaimsNoCheckedState() {
        rule.setContent {
            NoteRowView(
                note = note(),
                isSelected = false,
                isMultiSelectMode = false,
                isChecked = false,
                onClick = {},
                onLongClick = {},
                modifier = Modifier.testTag("row"),
            )
        }

        assertNull(row().fetchSemanticsNode().config.getOrNull(SemanticsProperties.ToggleableState))
    }

    private fun largeText(content: @Composable () -> Unit) {
        rule.setContent {
            val density = LocalDensity.current
            CompositionLocalProvider(LocalDensity provides Density(density.density, fontScale = 2f), content = content)
        }
    }

    @Test fun theColourStripeFollowsLargeText() {
        largeText {
            NoteRowView(
                note = note(color = "red"),
                isSelected = false,
                isMultiSelectMode = false,
                isChecked = false,
                onClick = {},
                onLongClick = {},
            )
        }

        // It used to stop at a fixed 52dp, whatever the text beside it needed
        rule.onNodeWithTag("colour-stripe", useUnmergedTree = true).assertHeightIsAtLeast(80.dp)
    }

    @Test fun aTagChipGrowsWithLargeText() {
        largeText { TagInputView(tags = listOf("work"), allTags = listOf("work"), onTagsChanged = {}) }

        // This size of text needs 32dp, and it used to be fixed at 28
        rule.onNodeWithTag("tag-chip").assertHeightIsAtLeast(32.dp)
    }

    @Test fun removingATagHasAFullSizeTarget() {
        rule.setContent { TagInputView(tags = listOf("work"), allTags = listOf("work"), onTagsChanged = {}) }

        rule.onNodeWithTag("remove-tag")
            .assertTouchHeightIsEqualTo(48.dp)
            .assertTouchWidthIsEqualTo(48.dp)
    }
}
