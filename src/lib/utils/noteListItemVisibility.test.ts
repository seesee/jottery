/**
 * Tests for NoteListItem visibility logic
 *
 * These tests verify the conditions under which the checkbox and delete button
 * are visible in the NoteListItem component, particularly the fix for mobile
 * interaction where both should appear simultaneously on first tap.
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

      it('should show when selected on mobile', () => {
        const state = createState({
          isSelected: true,
          forceMobileLayout: true,
        });
        expect(shouldShowCheckbox(state)).toBe(true);
      });

      it('should NOT show when selected but pinned', () => {
        const state = createState({
          isSelected: true,
          isPinned: true,
        });
        expect(shouldShowCheckbox(state)).toBe(false);
      });
    });

    describe('hover state - desktop vs mobile', () => {
      it('should NOT show on desktop hover (unselected)', () => {
        const state = createState({
          isHovered: true,
          forceMobileLayout: false,
        });
        expect(shouldShowCheckbox(state)).toBe(false);
      });

      it('should show on mobile hover (first tap) - KEY FIX', () => {
        // This is the key behaviour we fixed:
        // On mobile, when the user taps, the browser fires mouseenter
        // which sets isHovered=true. The checkbox should appear immediately
        // so it appears alongside the delete button.
        const state = createState({
          isHovered: true,
          forceMobileLayout: true,
          isSelected: false, // Not yet selected (first tap)
        });
        expect(shouldShowCheckbox(state)).toBe(true);
      });

      it('should NOT show on mobile hover when pinned', () => {
        const state = createState({
          isHovered: true,
          forceMobileLayout: true,
          isPinned: true,
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
    describe('hover state', () => {
      it('should show on desktop hover', () => {
        const state = createState({
          isHovered: true,
          forceMobileLayout: false,
        });
        expect(shouldShowDeleteButton(state)).toBe(true);
      });

      it('should show on mobile hover (first tap)', () => {
        const state = createState({
          isHovered: true,
          forceMobileLayout: true,
        });
        expect(shouldShowDeleteButton(state)).toBe(true);
      });
    });

    describe('mobile selection UI', () => {
      it('should show when mobile, selected, not pinned', () => {
        const state = createState({
          forceMobileLayout: true,
          isSelected: true,
          isPinned: false,
          isHovered: false,
        });
        expect(shouldShowDeleteButton(state)).toBe(true);
      });

      it('should NOT show when mobile, selected, but pinned', () => {
        const state = createState({
          forceMobileLayout: true,
          isSelected: true,
          isPinned: true,
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

  describe('mobile first tap symmetry - THE KEY BEHAVIOUR', () => {
    /**
     * This is the critical behaviour that was fixed:
     *
     * On mobile touch devices, when a user taps on a note:
     * 1. The browser fires mouseenter (synthetic hover) BEFORE click
     * 2. This sets isHovered = true
     * 3. BOTH the checkbox AND delete button should appear
     *
     * Previously, only the delete button appeared on hover, requiring
     * a second tap for the checkbox to appear (when isSelected became true).
     *
     * Now both appear together, enabling a 2-tap interaction:
     * - First tap: Shows both checkbox and delete button
     * - Second tap: Opens the note
     */

    it('checkbox and delete button should BOTH appear on mobile first tap', () => {
      // State after first tap's mouseenter event (before click)
      const stateAfterFirstTapHover = createState({
        isHovered: true,
        forceMobileLayout: true,
        isSelected: false, // Click hasn't fired yet
        isPinned: false,
      });

      // Both should be visible
      expect(shouldShowCheckbox(stateAfterFirstTapHover)).toBe(true);
      expect(shouldShowDeleteButton(stateAfterFirstTapHover)).toBe(true);
    });

    it('checkbox and delete button should BOTH remain visible after selection', () => {
      // State after first tap completes (mouseenter + click)
      const stateAfterFirstTapClick = createState({
        isHovered: true, // Still hovered
        forceMobileLayout: true,
        isSelected: true, // Click has fired, note is selected
        isPinned: false,
      });

      // Both should still be visible
      expect(shouldShowCheckbox(stateAfterFirstTapClick)).toBe(true);
      expect(shouldShowDeleteButton(stateAfterFirstTapClick)).toBe(true);
    });

    it('checkbox and delete button should BOTH remain visible when hover ends but selected', () => {
      // State when finger lifts (mouseleave fires but note is selected)
      const stateAfterHoverEnds = createState({
        isHovered: false, // Hover ended
        forceMobileLayout: true,
        isSelected: true, // Still selected
        isPinned: false,
      });

      // Both should still be visible via selection
      expect(shouldShowCheckbox(stateAfterHoverEnds)).toBe(true);
      expect(shouldShowDeleteButton(stateAfterHoverEnds)).toBe(true);
    });
  });

  describe('desktop behaviour remains unchanged', () => {
    it('on desktop: hover shows delete button only, not checkbox', () => {
      const desktopHoverState = createState({
        isHovered: true,
        forceMobileLayout: false,
        isSelected: false,
      });

      // Delete button appears on hover
      expect(shouldShowDeleteButton(desktopHoverState)).toBe(true);
      // Checkbox does NOT appear on hover (only on selection or multi-select)
      expect(shouldShowCheckbox(desktopHoverState)).toBe(false);
    });

    it('on desktop: click immediately selects AND shows checkbox', () => {
      const desktopSelectedState = createState({
        isHovered: true,
        forceMobileLayout: false,
        isSelected: true,
      });

      expect(shouldShowCheckbox(desktopSelectedState)).toBe(true);
      expect(shouldShowDeleteButton(desktopSelectedState)).toBe(true);
    });
  });
});
