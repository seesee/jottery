/**
 * Visibility logic for NoteListItem component
 *
 * This module contains the pure logic for determining when various UI elements
 * (checkbox, delete button) should be visible in the NoteListItem component.
 *
 * Extracted for testability and documentation of the visibility rules.
 *
 * ## Mobile Interaction Model (2 taps to open)
 *
 * On mobile touch devices, we use a selection-based model, NOT hover-based:
 *
 * 1. First tap: Selects the note, shows checkbox and delete button
 * 2. Second tap: Opens the note
 *
 * We deliberately avoid showing UI elements on hover for mobile because:
 * - Hover events on touch devices are unreliable (mouseenter fires but mouseleave may not)
 * - Elements appearing on hover can intercept the tap (checkbox has stopPropagation)
 * - Multiple notes can end up with hover state, causing multiple checkboxes to show
 *
 * ## Desktop Interaction Model
 *
 * On desktop with mouse:
 * - Delete button appears on hover (quick access without selection)
 * - Checkbox appears when note is selected or in multi-select mode
 * - Single click selects AND opens the note
 */

export interface NoteListItemVisibilityState {
  isMultiSelectMode: boolean;
  isSelected: boolean;
  isHovered: boolean;
  isPinned: boolean;
  forceMobileLayout: boolean;
}

/**
 * Determines whether the mobile selection UI should be shown.
 * This is used when the note is selected on mobile to show controls.
 */
export function shouldShowMobileSelectionUI(state: NoteListItemVisibilityState): boolean {
  const { forceMobileLayout, isSelected, isPinned } = state;
  return forceMobileLayout && isSelected && !isPinned;
}

/**
 * Determines whether the multi-select checkbox should be visible.
 *
 * The checkbox appears when:
 * - In multi-select mode (selecting multiple notes)
 * - When the note is selected (allows entering multi-select)
 *
 * The checkbox is NEVER shown for pinned notes.
 *
 * NOTE: We intentionally do NOT show the checkbox on hover for mobile.
 * If we did, the checkbox would appear before the click fires and could
 * intercept the tap (via stopPropagation), entering multi-select mode
 * instead of single-selecting the note.
 */
export function shouldShowCheckbox(state: NoteListItemVisibilityState): boolean {
  const { isMultiSelectMode, isSelected, isPinned } = state;

  // Never show checkbox for pinned notes
  if (isPinned) {
    return false;
  }

  // Show if in multi-select mode
  if (isMultiSelectMode) {
    return true;
  }

  // Show if note is selected
  if (isSelected) {
    return true;
  }

  return false;
}

/**
 * Determines whether the delete button should be visible.
 *
 * Desktop: Shows on hover (quick access without selection)
 * Mobile: Shows only when selected (avoids hover-related issues)
 *
 * NOTE: On mobile, we don't show the delete button on hover because:
 * 1. Hover state is unreliable (mouseleave may not fire when tapping elsewhere)
 * 2. This can cause multiple notes to show delete buttons simultaneously
 * 3. The delete button could intercept taps meant for selecting the note
 */
export function shouldShowDeleteButton(state: NoteListItemVisibilityState): boolean {
  const { isHovered, isPinned, isSelected, forceMobileLayout } = state;

  // On desktop only: show on hover for quick access
  if (!forceMobileLayout && isHovered) {
    return true;
  }

  // On mobile: only show when note is selected
  if (forceMobileLayout && isSelected && !isPinned) {
    return true;
  }

  return false;
}
