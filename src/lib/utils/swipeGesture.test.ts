/**
 * Tests for swipe gesture logic
 *
 * These tests verify the swipe gesture handling for mobile note list items.
 */
import { describe, it, expect } from 'vitest';
import {
  createSwipeState,
  handleSwipeStart,
  handleSwipeMove,
  handleSwipeEnd,
  getSwipeTransform,
  getActionOpacity,
  SWIPE_THRESHOLD,
  SWIPE_CANCEL_Y,
  SWIPE_MAX_DISTANCE,
} from './swipeGesture';

describe('swipeGesture', () => {
  describe('createSwipeState', () => {
    it('should create initial inactive state', () => {
      const state = createSwipeState();

      expect(state.isActive).toBe(false);
      expect(state.direction).toBe('none');
      expect(state.distance).toBe(0);
      expect(state.isPastThreshold).toBe(false);
      expect(state.isCancelled).toBe(false);
    });
  });

  describe('handleSwipeStart', () => {
    it('should initialize tracking with touch position', () => {
      const state = handleSwipeStart(100, 200);

      expect(state.startX).toBe(100);
      expect(state.startY).toBe(200);
      expect(state.currentX).toBe(100);
      expect(state.currentY).toBe(200);
      expect(state.isActive).toBe(true);
      expect(state.direction).toBe('none');
    });
  });

  describe('handleSwipeMove', () => {
    it('should detect left swipe (right-to-left)', () => {
      const start = handleSwipeStart(200, 100);
      const moved = handleSwipeMove(start, 150, 100); // Moved 50px left

      expect(moved.direction).toBe('left');
      expect(moved.distance).toBe(50);
      expect(moved.isPastThreshold).toBe(false); // Below 80px threshold
    });

    it('should detect right swipe (left-to-right)', () => {
      const start = handleSwipeStart(100, 100);
      const moved = handleSwipeMove(start, 150, 100); // Moved 50px right

      expect(moved.direction).toBe('right');
      expect(moved.distance).toBe(50);
    });

    it('should mark past threshold when distance exceeds SWIPE_THRESHOLD', () => {
      const start = handleSwipeStart(200, 100);
      const moved = handleSwipeMove(start, 200 - SWIPE_THRESHOLD - 10, 100);

      expect(moved.isPastThreshold).toBe(true);
    });

    it('should cancel swipe when vertical movement exceeds SWIPE_CANCEL_Y', () => {
      const start = handleSwipeStart(100, 100);
      const moved = handleSwipeMove(start, 110, 100 + SWIPE_CANCEL_Y + 1);

      expect(moved.isCancelled).toBe(true);
      expect(moved.direction).toBe('none');
      expect(moved.distance).toBe(0);
    });

    it('should cap distance at SWIPE_MAX_DISTANCE', () => {
      const start = handleSwipeStart(300, 100);
      const moved = handleSwipeMove(start, 100, 100); // 200px left

      expect(moved.distance).toBe(SWIPE_MAX_DISTANCE);
    });

    it('should not update cancelled state', () => {
      const start = handleSwipeStart(100, 100);
      const cancelled = handleSwipeMove(start, 110, 100 + SWIPE_CANCEL_Y + 1);
      const afterCancel = handleSwipeMove(cancelled, 50, 100);

      expect(afterCancel.isCancelled).toBe(true);
      expect(afterCancel.distance).toBe(0);
    });
  });

  describe('handleSwipeEnd', () => {
    it('should return delete action for left swipe past threshold', () => {
      const start = handleSwipeStart(200, 100);
      const moved = handleSwipeMove(start, 200 - SWIPE_THRESHOLD - 10, 100);
      const result = handleSwipeEnd(moved);

      expect(result.action).toBe('delete');
      expect(result.state.isActive).toBe(false);
    });

    it('should return select action for right swipe past threshold', () => {
      const start = handleSwipeStart(100, 100);
      const moved = handleSwipeMove(start, 100 + SWIPE_THRESHOLD + 10, 100);
      const result = handleSwipeEnd(moved);

      expect(result.action).toBe('select');
    });

    it('should return none for swipe below threshold', () => {
      const start = handleSwipeStart(100, 100);
      const moved = handleSwipeMove(start, 150, 100); // Only 50px
      const result = handleSwipeEnd(moved);

      expect(result.action).toBe('none');
    });

    it('should return none for cancelled swipe', () => {
      const start = handleSwipeStart(100, 100);
      const cancelled = handleSwipeMove(start, 50, 150); // Vertical scroll
      const result = handleSwipeEnd(cancelled);

      expect(result.action).toBe('none');
    });

    it('should reset state after end', () => {
      const start = handleSwipeStart(200, 100);
      const moved = handleSwipeMove(start, 100, 100);
      const result = handleSwipeEnd(moved);

      expect(result.state.isActive).toBe(false);
      expect(result.state.distance).toBe(0);
    });
  });

  describe('getSwipeTransform', () => {
    it('should return 0 for inactive state', () => {
      const state = createSwipeState();
      expect(getSwipeTransform(state)).toBe(0);
    });

    it('should return negative value for left swipe', () => {
      const start = handleSwipeStart(200, 100);
      const moved = handleSwipeMove(start, 150, 100);

      expect(getSwipeTransform(moved)).toBe(-50);
    });

    it('should return positive value for right swipe', () => {
      const start = handleSwipeStart(100, 100);
      const moved = handleSwipeMove(start, 150, 100);

      expect(getSwipeTransform(moved)).toBe(50);
    });

    it('should return 0 for cancelled swipe', () => {
      const start = handleSwipeStart(100, 100);
      const cancelled = handleSwipeMove(start, 50, 150);

      expect(getSwipeTransform(cancelled)).toBe(0);
    });
  });

  describe('getActionOpacity', () => {
    it('should return 0 for inactive state', () => {
      const state = createSwipeState();
      expect(getActionOpacity(state)).toBe(0);
    });

    it('should return 0.5 at half threshold', () => {
      const start = handleSwipeStart(100, 100);
      const moved = handleSwipeMove(start, 100 + SWIPE_THRESHOLD / 2, 100);

      expect(getActionOpacity(moved)).toBeCloseTo(0.5);
    });

    it('should return 1 at or above threshold', () => {
      const start = handleSwipeStart(100, 100);
      const moved = handleSwipeMove(start, 100 + SWIPE_THRESHOLD + 20, 100);

      expect(getActionOpacity(moved)).toBe(1);
    });

    it('should return 0 for cancelled swipe', () => {
      const start = handleSwipeStart(100, 100);
      const cancelled = handleSwipeMove(start, 50, 150);

      expect(getActionOpacity(cancelled)).toBe(0);
    });
  });

  describe('swipe gesture flow', () => {
    it('should complete full delete swipe flow', () => {
      // User starts touch
      const start = handleSwipeStart(200, 100);
      expect(start.isActive).toBe(true);

      // User swipes left
      const moving1 = handleSwipeMove(start, 160, 100);
      expect(moving1.direction).toBe('left');
      expect(moving1.isPastThreshold).toBe(false);

      // User continues past threshold
      const moving2 = handleSwipeMove(moving1, 100, 100);
      expect(moving2.isPastThreshold).toBe(true);

      // User releases
      const result = handleSwipeEnd(moving2);
      expect(result.action).toBe('delete');
    });

    it('should complete full select swipe flow', () => {
      // User starts touch
      const start = handleSwipeStart(100, 100);

      // User swipes right past threshold
      const moved = handleSwipeMove(start, 100 + SWIPE_THRESHOLD + 10, 100);
      expect(moved.direction).toBe('right');
      expect(moved.isPastThreshold).toBe(true);

      // User releases
      const result = handleSwipeEnd(moved);
      expect(result.action).toBe('select');
    });

    it('should cancel swipe when user scrolls vertically', () => {
      // User starts touch
      const start = handleSwipeStart(100, 100);

      // User moves slightly horizontal, then vertical
      const moving1 = handleSwipeMove(start, 120, 105);
      expect(moving1.isCancelled).toBe(false);

      // User scrolls down
      const moving2 = handleSwipeMove(moving1, 130, 140);
      expect(moving2.isCancelled).toBe(true);

      // Even if user swipes horizontally after, it stays cancelled
      const moving3 = handleSwipeMove(moving2, 200, 140);
      expect(moving3.isCancelled).toBe(true);

      // Release should not trigger action
      const result = handleSwipeEnd(moving3);
      expect(result.action).toBe('none');
    });

    it('should snap back when swipe released below threshold', () => {
      const start = handleSwipeStart(100, 100);
      const moved = handleSwipeMove(start, 140, 100); // Only 40px

      expect(moved.isPastThreshold).toBe(false);

      const result = handleSwipeEnd(moved);
      expect(result.action).toBe('none');
      expect(result.state.distance).toBe(0);
    });
  });
});
