/**
 * Visibility logic for NoteListItem component
 *
 * This module contains the pure logic for determining when various UI elements
 * (checkbox, delete button) should be visible in the NoteListItem component.
 *
 * Extracted for testability and documentation of the visibility rules.
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
 * - On mobile when the note is selected (showMobileSelectionUI)
 * - When the note is selected (allows entering multi-select)
 * - On mobile when hovered (first tap triggers hover on touch devices)
 *
 * The checkbox is NEVER shown for pinned notes.
 *
 * IMPORTANT: On mobile touch devices, the browser fires mouseenter before click.
 * The mobile hover condition ensures the checkbox appears on the first tap,
 * symmetrical with the delete button's behaviour.
 */
export function shouldShowCheckbox(state: NoteListItemVisibilityState): boolean {
  const { isMultiSelectMode, isSelected, isHovered, isPinned, forceMobileLayout } = state;

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

  // On mobile, also show when hovered (first tap)
  // This ensures checkbox and delete button appear together
  if (forceMobileLayout && isHovered) {
    return true;
  }

  return false;
}

/**
 * Determines whether the delete button should be visible.
 *
 * The delete button appears when:
 * - Hovered (desktop mouse hover, or mobile touch synthesized hover)
 * - On mobile when the note is selected (showMobileSelectionUI)
 */
export function shouldShowDeleteButton(state: NoteListItemVisibilityState): boolean {
  const { isHovered, isPinned, isSelected, forceMobileLayout } = state;

  // Show on hover (works for both desktop and mobile first tap)
  if (isHovered) {
    return true;
  }

  // Show on mobile when note is selected (showMobileSelectionUI logic)
  if (forceMobileLayout && isSelected && !isPinned) {
    return true;
  }

  return false;
}
