/**
 * Swipe gesture logic for NoteListItem
 *
 * This module provides pure functions for handling swipe gestures on mobile.
 * Used to reveal actions (delete, multi-select checkbox) on note list items.
 *
 * ## Gesture Model
 *
 * - Swipe right-to-left: Reveals delete action (red background)
 * - Swipe left-to-right: Reveals multi-select checkbox (blue background)
 * - Threshold: 80px to trigger action
 * - Cancel: 30px vertical movement cancels swipe (user is scrolling)
 *
 * ## Usage
 *
 * Touch events call these functions to update state:
 * - handleTouchStart: Initialize tracking
 * - handleTouchMove: Update position and direction
 * - handleTouchEnd: Determine if action should trigger
 */

export const SWIPE_THRESHOLD = 80; // Distance to trigger action
export const SWIPE_CANCEL_Y = 30; // Vertical movement to cancel (user scrolling)
export const SWIPE_MAX_DISTANCE = 120; // Maximum swipe distance for visual feedback

export type SwipeDirection = 'left' | 'right' | 'none';

export interface SwipeState {
  /** Starting X position of touch */
  startX: number;
  /** Starting Y position of touch */
  startY: number;
  /** Current X position */
  currentX: number;
  /** Current Y position */
  currentY: number;
  /** Swipe direction */
  direction: SwipeDirection;
  /** Distance swiped (positive value) */
  distance: number;
  /** Whether swipe has passed threshold */
  isPastThreshold: boolean;
  /** Whether swipe was cancelled (vertical scroll) */
  isCancelled: boolean;
  /** Whether swipe is currently active */
  isActive: boolean;
}

/**
 * Creates initial swipe state
 */
export function createSwipeState(): SwipeState {
  return {
    startX: 0,
    startY: 0,
    currentX: 0,
    currentY: 0,
    direction: 'none',
    distance: 0,
    isPastThreshold: false,
    isCancelled: false,
    isActive: false,
  };
}

/**
 * Initializes swipe tracking from touch start event
 */
export function handleSwipeStart(clientX: number, clientY: number): SwipeState {
  return {
    startX: clientX,
    startY: clientY,
    currentX: clientX,
    currentY: clientY,
    direction: 'none',
    distance: 0,
    isPastThreshold: false,
    isCancelled: false,
    isActive: true,
  };
}

/**
 * Updates swipe state from touch move event
 */
export function handleSwipeMove(state: SwipeState, clientX: number, clientY: number): SwipeState {
  if (!state.isActive || state.isCancelled) {
    return state;
  }

  const deltaX = clientX - state.startX;
  const deltaY = clientY - state.startY;

  // Check if vertical movement exceeds threshold (user is scrolling)
  if (Math.abs(deltaY) > SWIPE_CANCEL_Y) {
    return {
      ...state,
      isCancelled: true,
      direction: 'none',
      distance: 0,
      isPastThreshold: false,
    };
  }

  // Determine direction and distance
  const direction: SwipeDirection = deltaX < 0 ? 'left' : deltaX > 0 ? 'right' : 'none';
  const distance = Math.min(Math.abs(deltaX), SWIPE_MAX_DISTANCE);
  const isPastThreshold = distance >= SWIPE_THRESHOLD;

  return {
    ...state,
    currentX: clientX,
    currentY: clientY,
    direction,
    distance,
    isPastThreshold,
  };
}

/**
 * Finalizes swipe and determines action result
 */
export function handleSwipeEnd(state: SwipeState): {
  action: 'delete' | 'select' | 'none';
  state: SwipeState;
} {
  if (!state.isActive || state.isCancelled || !state.isPastThreshold) {
    return {
      action: 'none',
      state: createSwipeState(),
    };
  }

  // Determine action based on direction
  // Left swipe (right-to-left) = delete
  // Right swipe (left-to-right) = select for multi-select
  const action = state.direction === 'left' ? 'delete' : state.direction === 'right' ? 'select' : 'none';

  return {
    action,
    state: createSwipeState(),
  };
}

/**
 * Calculates transform distance for visual feedback
 * Returns the translateX value in pixels
 */
export function getSwipeTransform(state: SwipeState): number {
  if (!state.isActive || state.isCancelled) {
    return 0;
  }

  // Return signed distance for transform
  return state.direction === 'left' ? -state.distance : state.direction === 'right' ? state.distance : 0;
}

/**
 * Calculates opacity for action reveal (0 to 1)
 */
export function getActionOpacity(state: SwipeState): number {
  if (!state.isActive || state.isCancelled || state.distance === 0) {
    return 0;
  }

  // Ease in opacity as we approach threshold
  return Math.min(state.distance / SWIPE_THRESHOLD, 1);
}

/**
 * Triggers haptic feedback if available
 */
export function triggerHapticFeedback(): void {
  if (navigator.vibrate) {
    navigator.vibrate(10);
  }
}
