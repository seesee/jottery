/**
 * Modal utilities - Svelte action for consistent modal behaviour
 *
 * Provides:
 * - Escape key handling to close modal
 * - Body scroll locking while modal is open
 * - Focus trapping within the modal
 * - Restores focus to trigger element on close
 */

export interface ModalOptions {
  /** Callback when escape key is pressed */
  onEscape?: () => void;
  /** Whether to lock body scroll (default: true) */
  lockScroll?: boolean;
  /** Whether to trap focus within modal (default: true) */
  trapFocus?: boolean;
  /** Whether to restore focus on close (default: true) */
  restoreFocus?: boolean;
}

/**
 * Svelte action for modal behaviour
 *
 * Usage:
 * <div use:modal={{ onEscape: handleClose }}>
 *   Modal content
 * </div>
 */
export function modal(node: HTMLElement, options: ModalOptions = {}) {
  const {
    onEscape,
    lockScroll = true,
    trapFocus = true,
    restoreFocus = true,
  } = options;

  // Store the previously focused element to restore later
  const previouslyFocused = restoreFocus ? document.activeElement as HTMLElement : null;

  // Lock body scroll
  const originalOverflow = document.body.style.overflow;
  if (lockScroll) {
    document.body.style.overflow = 'hidden';
  }

  // Focus the modal or first focusable element
  const focusableSelectors = [
    'button:not([disabled])',
    'input:not([disabled])',
    'select:not([disabled])',
    'textarea:not([disabled])',
    'a[href]',
    '[tabindex]:not([tabindex="-1"])',
  ].join(', ');

  function getFocusableElements(): HTMLElement[] {
    return Array.from(node.querySelectorAll(focusableSelectors));
  }

  function focusFirstElement() {
    const focusable = getFocusableElements();
    if (focusable.length > 0) {
      focusable[0].focus();
    } else {
      // If no focusable elements, focus the modal itself
      node.focus();
    }
  }

  // Initial focus
  requestAnimationFrame(() => {
    focusFirstElement();
  });

  // Handle keydown events
  function handleKeyDown(event: KeyboardEvent) {
    // Escape key
    if (event.key === 'Escape' && onEscape) {
      event.preventDefault();
      event.stopPropagation();
      onEscape();
      return;
    }

    // Tab key - focus trapping
    if (trapFocus && event.key === 'Tab') {
      const focusable = getFocusableElements();
      if (focusable.length === 0) {
        event.preventDefault();
        return;
      }

      const firstFocusable = focusable[0];
      const lastFocusable = focusable[focusable.length - 1];

      if (event.shiftKey) {
        // Shift+Tab: if on first element, go to last
        if (document.activeElement === firstFocusable) {
          event.preventDefault();
          lastFocusable.focus();
        }
      } else {
        // Tab: if on last element, go to first
        if (document.activeElement === lastFocusable) {
          event.preventDefault();
          firstFocusable.focus();
        }
      }
    }
  }

  // Add event listener
  node.addEventListener('keydown', handleKeyDown);

  return {
    update(newOptions: ModalOptions) {
      // Update options if needed
      Object.assign(options, newOptions);
    },
    destroy() {
      // Remove event listener
      node.removeEventListener('keydown', handleKeyDown);

      // Restore body scroll
      if (lockScroll) {
        document.body.style.overflow = originalOverflow;
      }

      // Restore focus
      if (restoreFocus && previouslyFocused && previouslyFocused.focus) {
        previouslyFocused.focus();
      }
    },
  };
}

/**
 * Helper for backdrop click handling
 * Returns true if the click was on the backdrop (not on modal content)
 */
export function isBackdropClick(event: MouseEvent): boolean {
  return event.target === event.currentTarget;
}

/**
 * Create a backdrop click handler
 * Only calls onClose if the click was on the backdrop itself
 */
export function createBackdropHandler(onClose: () => void) {
  return (event: MouseEvent) => {
    if (isBackdropClick(event)) {
      onClose();
    }
  };
}
