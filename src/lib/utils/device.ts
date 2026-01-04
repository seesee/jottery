/**
 * Device detection utilities for mobile-specific features
 */

/**
 * Check if the current device is a mobile device based on screen width
 * @returns true if viewport width is 768px or less
 */
export function isMobileDevice(): boolean {
  if (typeof window === 'undefined') return false;
  return window.innerWidth <= 768;
}

/**
 * Check if the current device supports touch input
 * @returns true if touch is supported
 */
export function isTouchDevice(): boolean {
  if (typeof window === 'undefined') return false;
  return 'ontouchstart' in window || navigator.maxTouchPoints > 0;
}

/**
 * Check if the current device is both mobile and touch-enabled
 * Useful for features like pull-to-refresh
 * @returns true if device is mobile with touch support
 */
export function isMobileTouchDevice(): boolean {
  return isMobileDevice() && isTouchDevice();
}
