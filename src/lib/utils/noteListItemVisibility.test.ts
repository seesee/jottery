/**
 * Tests for NoteListItem visibility logic
 *
 * These tests verify the conditions under which the checkbox and delete button
 * are visible in the NoteListItem component.
 *
 * Key design decisions tested:
 * - Mobile uses selection-based visibility (not hover-based)
 * - Desktop uses hover for delete button, selection for checkbox
 * - This prevents UI elements from intercepting taps on mobile
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

    describe('mobile - NO hover-based visibility', () => {
      it('should NOT show on mobile hover - prevents unreliable UI', () => {
        // On mobile, hover state is unreliable (mouseleave may not fire)
        // This could cause multiple notes to show delete buttons
        const state = createState({
          isHovered: true,
          forceMobileLayout: true,
          isSelected: false,
        });
        expect(shouldShowDeleteButton(state)).toBe(false);
      });

      it('should show when mobile and selected', () => {
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

  describe('mobile 2-tap interaction model', () => {
    /**
     * On mobile, the interaction should be:
     * 1. First tap: Selects the note, shows checkbox and delete
     * 2. Second tap: Opens the note
     *
     * We use selection-based visibility (not hover) because:
     * - Hover is unreliable on touch devices
     * - Elements appearing on hover can intercept taps
     * - Multiple notes could end up with hover state
     */

    it('before first tap: nothing should show', () => {
      const beforeTap = createState({
        forceMobileLayout: true,
        isSelected: false,
        isHovered: false,
      });

      expect(shouldShowCheckbox(beforeTap)).toBe(false);
      expect(shouldShowDeleteButton(beforeTap)).toBe(false);
    });

    it('during first tap (hover fires): still nothing should show', () => {
      // Even though mouseenter fires before click, we don't show UI
      // This prevents the checkbox from intercepting the tap
      const duringTapHover = createState({
        forceMobileLayout: true,
        isSelected: false,
        isHovered: true,
      });

      expect(shouldShowCheckbox(duringTapHover)).toBe(false);
      expect(shouldShowDeleteButton(duringTapHover)).toBe(false);
    });

    it('after first tap (selected): both should show', () => {
      // After the click handler runs, note is selected
      // Now both checkbox and delete appear
      const afterFirstTap = createState({
        forceMobileLayout: true,
        isSelected: true,
        isHovered: true, // May still be hovered
        isPinned: false,
      });

      expect(shouldShowCheckbox(afterFirstTap)).toBe(true);
      expect(shouldShowDeleteButton(afterFirstTap)).toBe(true);
    });

    it('after hover ends but still selected: both should still show', () => {
      const hoverEnded = createState({
        forceMobileLayout: true,
        isSelected: true,
        isHovered: false,
        isPinned: false,
      });

      expect(shouldShowCheckbox(hoverEnded)).toBe(true);
      expect(shouldShowDeleteButton(hoverEnded)).toBe(true);
    });

    it('when tapping different note: old note UI should hide', () => {
      // When user taps note B, note A becomes unselected
      // Note A's checkbox and delete should hide
      const oldNoteAfterNewSelection = createState({
        forceMobileLayout: true,
        isSelected: false, // No longer selected
        isHovered: true, // Hover might still be true (unreliable on mobile)
        isPinned: false,
      });

      // Even with hover=true, nothing shows because we use selection-based visibility
      expect(shouldShowCheckbox(oldNoteAfterNewSelection)).toBe(false);
      expect(shouldShowDeleteButton(oldNoteAfterNewSelection)).toBe(false);
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

    it('should not show delete button for pinned notes on mobile even when selected', () => {
      const pinnedMobile = createState({
        isPinned: true,
        isSelected: true,
        forceMobileLayout: true,
      });

      expect(shouldShowDeleteButton(pinnedMobile)).toBe(false);
    });

    it('should show delete button for pinned notes on desktop hover', () => {
      // Pinned notes can still be deleted via hover on desktop
      const pinnedDesktopHover = createState({
        isPinned: true,
        isHovered: true,
        forceMobileLayout: false,
      });

      expect(shouldShowDeleteButton(pinnedDesktopHover)).toBe(true);
    });
  });
});
