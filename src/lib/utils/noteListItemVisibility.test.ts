/**
 * Tests for NoteListItem visibility logic
 *
 * These tests verify the conditions under which the checkbox and delete button
 * are visible in the NoteListItem component.
 *
 * Key design decisions tested:
 * - Mobile uses swipe gestures for checkbox/delete (not shown by default)
 * - Desktop uses hover for delete button, selection for checkbox
 * - Single tap opens note immediately on mobile
 */
import { describe, it, expect } from 'vitest';
import {
  shouldShowCheckbox,
  shouldShowDeleteButton,
  shouldShowMobileSelectionUI,
  type NoteListItemVisibilityState,
} from './noteListItemVisibility';

function createState(overrides: Partial<NoteListItemVisibilityState> = {}): NoteListItemVisibilityState {
  return {
    isMultiSelectMode: false,
    isSelected: false,
    isHovered: false,
    isPinned: false,
    isLocked: false,
    forceMobileLayout: false,
    ...overrides,
  };
}

describe('noteListItemVisibility', () => {
  describe('shouldShowMobileSelectionUI', () => {
    it('should return true when mobile, selected, and not pinned', () => {
      const state = createState({
        forceMobileLayout: true,
        isSelected: true,
        isPinned: false,
      });
      expect(shouldShowMobileSelectionUI(state)).toBe(true);
    });

    it('should return false when not mobile', () => {
      const state = createState({
        forceMobileLayout: false,
        isSelected: true,
        isPinned: false,
      });
      expect(shouldShowMobileSelectionUI(state)).toBe(false);
    });

    it('should return false when not selected', () => {
      const state = createState({
        forceMobileLayout: true,
        isSelected: false,
        isPinned: false,
      });
      expect(shouldShowMobileSelectionUI(state)).toBe(false);
    });

    it('should return false when pinned', () => {
      const state = createState({
        forceMobileLayout: true,
        isSelected: true,
        isPinned: true,
      });
      expect(shouldShowMobileSelectionUI(state)).toBe(false);
    });
  });

  describe('shouldShowCheckbox', () => {
    describe('multi-select mode', () => {
      it('should show when in multi-select mode', () => {
        const state = createState({ isMultiSelectMode: true });
        expect(shouldShowCheckbox(state)).toBe(true);
      });

      it('should NOT show when in multi-select mode but pinned', () => {
        const state = createState({
          isMultiSelectMode: true,
          isPinned: true,
        });
        expect(shouldShowCheckbox(state)).toBe(false);
      });
    });

    describe('selected state', () => {
      it('should show when selected on desktop', () => {
        const state = createState({
          isSelected: true,
          forceMobileLayout: false,
        });
        expect(shouldShowCheckbox(state)).toBe(true);
      });

      it('should NOT show when selected on mobile (swipe reveals it)', () => {
        const state = createState({
          isSelected: true,
          forceMobileLayout: true,
        });
        expect(shouldShowCheckbox(state)).toBe(false);
      });

      it('should NOT show when selected but pinned', () => {
        const state = createState({
          isSelected: true,
          isPinned: true,
        });
        expect(shouldShowCheckbox(state)).toBe(false);
      });
    });

    describe('hover state - NO checkbox on hover', () => {
      it('should NOT show on desktop hover (unselected)', () => {
        const state = createState({
          isHovered: true,
          forceMobileLayout: false,
        });
        expect(shouldShowCheckbox(state)).toBe(false);
      });

      it('should NOT show on mobile hover - prevents click interception', () => {
        // This is critical: if checkbox appeared on hover, it would intercept
        // the tap via stopPropagation, entering multi-select instead of selecting
        const state = createState({
          isHovered: true,
          forceMobileLayout: true,
          isSelected: false,
        });
        expect(shouldShowCheckbox(state)).toBe(false);
      });
    });

    describe('unselected, unhovered state', () => {
      it('should NOT show when nothing is active', () => {
        const state = createState();
        expect(shouldShowCheckbox(state)).toBe(false);
      });
    });
  });

  describe('shouldShowDeleteButton', () => {
    describe('desktop hover', () => {
      it('should show on desktop hover', () => {
        const state = createState({
          isHovered: true,
          forceMobileLayout: false,
        });
        expect(shouldShowDeleteButton(state)).toBe(true);
      });
    });

    describe('mobile - swipe-based visibility', () => {
      it('should NEVER show on mobile - swipe reveals delete action', () => {
        // On mobile, delete button is revealed via swipe gesture
        const state = createState({
          isHovered: true,
          forceMobileLayout: true,
          isSelected: true,
        });
        expect(shouldShowDeleteButton(state)).toBe(false);
      });

      it('should NOT show on mobile even when selected', () => {
        const state = createState({
          forceMobileLayout: true,
          isSelected: true,
          isPinned: false,
          isHovered: false,
        });
        expect(shouldShowDeleteButton(state)).toBe(false);
      });
    });

    describe('unselected, unhovered state', () => {
      it('should NOT show when nothing is active', () => {
        const state = createState();
        expect(shouldShowDeleteButton(state)).toBe(false);
      });
    });
  });

  describe('mobile single-tap + swipe interaction model', () => {
    /**
     * On mobile, the interaction should be:
     * - Single tap: Opens the note immediately
     * - Swipe right-to-left: Reveals delete action
     * - Swipe left-to-right: Reveals checkbox for multi-select
     *
     * No UI elements shown by default - cleaner interface.
     */

    it('nothing should show by default on mobile', () => {
      const defaultState = createState({
        forceMobileLayout: true,
        isSelected: false,
        isHovered: false,
      });

      expect(shouldShowCheckbox(defaultState)).toBe(false);
      expect(shouldShowDeleteButton(defaultState)).toBe(false);
    });

    it('nothing should show even when selected on mobile (swipe reveals controls)', () => {
      const selectedState = createState({
        forceMobileLayout: true,
        isSelected: true,
        isHovered: false,
        isPinned: false,
      });

      // Checkbox and delete are accessed via swipe, not selection
      expect(shouldShowCheckbox(selectedState)).toBe(false);
      expect(shouldShowDeleteButton(selectedState)).toBe(false);
    });

    it('checkbox should show in multi-select mode on mobile', () => {
      const multiSelectState = createState({
        forceMobileLayout: true,
        isMultiSelectMode: true,
        isSelected: false,
        isPinned: false,
      });

      // In multi-select mode, show checkbox so user can see selection state
      expect(shouldShowCheckbox(multiSelectState)).toBe(true);
      // Delete button still hidden - use bulk operations toolbar
      expect(shouldShowDeleteButton(multiSelectState)).toBe(false);
    });
  });

  describe('desktop single-click interaction model', () => {
    /**
     * On desktop, a single click both selects AND opens the note.
     * The delete button appears on hover for quick access.
     * The checkbox appears when selected (for entering multi-select).
     */

    it('on hover: delete button shows, checkbox does not', () => {
      const desktopHover = createState({
        forceMobileLayout: false,
        isHovered: true,
        isSelected: false,
      });

      expect(shouldShowDeleteButton(desktopHover)).toBe(true);
      expect(shouldShowCheckbox(desktopHover)).toBe(false);
    });

    it('when selected: both show', () => {
      const desktopSelected = createState({
        forceMobileLayout: false,
        isHovered: true,
        isSelected: true,
      });

      expect(shouldShowDeleteButton(desktopSelected)).toBe(true);
      expect(shouldShowCheckbox(desktopSelected)).toBe(true);
    });
  });

  describe('pinned notes', () => {
    it('should never show checkbox for pinned notes regardless of state', () => {
      const pinnedSelected = createState({
        isPinned: true,
        isSelected: true,
        isMultiSelectMode: true,
      });

      expect(shouldShowCheckbox(pinnedSelected)).toBe(false);
    });

    it('should not show delete button on mobile (swipe disabled for pinned in Phase 4)', () => {
      const pinnedMobile = createState({
        isPinned: true,
        isSelected: true,
        forceMobileLayout: true,
      });

      expect(shouldShowDeleteButton(pinnedMobile)).toBe(false);
    });

    it('should NOT show delete button for pinned notes on desktop hover', () => {
      // Pinned notes are protected from accidental deletion - hide the delete button
      const pinnedDesktopHover = createState({
        isPinned: true,
        isHovered: true,
        forceMobileLayout: false,
      });

      expect(shouldShowDeleteButton(pinnedDesktopHover)).toBe(false);
    });
  });

  describe('locked notes', () => {
    it('should never show checkbox for locked notes regardless of state', () => {
      const lockedSelected = createState({
        isLocked: true,
        isSelected: true,
        isMultiSelectMode: true,
      });

      expect(shouldShowCheckbox(lockedSelected)).toBe(false);
    });

    it('should NOT show checkbox for locked notes in multi-select mode', () => {
      const lockedMultiSelect = createState({
        isLocked: true,
        isMultiSelectMode: true,
      });

      expect(shouldShowCheckbox(lockedMultiSelect)).toBe(false);
    });

    it('should not show delete button on mobile for locked notes', () => {
      const lockedMobile = createState({
        isLocked: true,
        isSelected: true,
        forceMobileLayout: true,
      });

      expect(shouldShowDeleteButton(lockedMobile)).toBe(false);
    });

    it('should NOT show delete button for locked notes on desktop hover', () => {
      // Locked notes are protected from accidental deletion - hide the delete button
      const lockedDesktopHover = createState({
        isLocked: true,
        isHovered: true,
        forceMobileLayout: false,
      });

      expect(shouldShowDeleteButton(lockedDesktopHover)).toBe(false);
    });

    it('locked notes behave like pinned notes for protection', () => {
      // Both pinned and locked notes should be protected the same way
      const lockedState = createState({ isLocked: true, isHovered: true, isSelected: true, isMultiSelectMode: true });
      const pinnedState = createState({ isPinned: true, isHovered: true, isSelected: true, isMultiSelectMode: true });

      expect(shouldShowCheckbox(lockedState)).toBe(shouldShowCheckbox(pinnedState));
      expect(shouldShowDeleteButton(lockedState)).toBe(shouldShowDeleteButton(pinnedState));
    });
  });
});
