/**
 * Visibility logic for NoteListItem component
 *
 * This module contains the pure logic for determining when various UI elements
 * (checkbox, delete button) should be visible in the NoteListItem component.
 *
 * Extracted for testability and documentation of the visibility rules.
 *
 * ## Mobile Interaction Model (single tap + swipe)
 *
 * On mobile touch devices:
 * - Single tap: Opens the note immediately
 * - Swipe right-to-left: Reveals delete action
 * - Swipe left-to-right: Reveals checkbox for multi-select
 *
 * The checkbox and delete button are NOT shown by default on mobile.
 * They are revealed through swipe gestures for a cleaner interface.
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
 * Desktop: Shows when selected or in multi-select mode
 * Mobile: Only shows in multi-select mode (swipe reveals checkbox otherwise)
 *
 * The checkbox is NEVER shown for pinned notes.
 */
export function shouldShowCheckbox(state: NoteListItemVisibilityState): boolean {
  const { isMultiSelectMode, isSelected, isPinned, forceMobileLayout } = state;

  // Never show checkbox for pinned notes
  if (isPinned) {
    return false;
  }

  // Show if in multi-select mode (both mobile and desktop)
  if (isMultiSelectMode) {
    return true;
  }

  // On desktop only: show if note is selected (allows entering multi-select)
  // On mobile: don't show on selection - swipe reveals checkbox instead
  if (!forceMobileLayout && isSelected) {
    return true;
  }

  return false;
}

/**
 * Determines whether the delete button should be visible.
 *
 * Desktop: Shows on hover (quick access without selection), but NOT for pinned notes
 * Mobile: Never shows (swipe reveals delete action)
 *
 * Pinned notes are protected from accidental deletion - the delete button is hidden.
 */
export function shouldShowDeleteButton(state: NoteListItemVisibilityState): boolean {
  const { isHovered, forceMobileLayout, isPinned } = state;

  // Never show delete button for pinned notes - they are protected
  if (isPinned) {
    return false;
  }

  // On mobile: never show delete button - swipe reveals it instead
  if (forceMobileLayout) {
    return false;
  }

  // On desktop: show on hover for quick access
  if (isHovered) {
    return true;
  }

  return false;
}
