import type { UserSettings } from '../types';

/**
 * Detect if the current device is mobile
 */
export function isMobileDevice(): boolean {
  if (typeof window === 'undefined') return false;

  // Check for touch support and small screen
  const hasTouch = 'ontouchstart' in window || navigator.maxTouchPoints > 0;
  const isSmallScreen = window.innerWidth < 768;

  // Also check user agent for mobile devices
  const mobileRegex = /Android|webOS|iPhone|iPad|iPod|BlackBerry|IEMobile|Opera Mini/i;
  const isMobileUA = mobileRegex.test(navigator.userAgent);

  return (hasTouch && isSmallScreen) || isMobileUA;
}

/**
 * Get the font size in pixels based on the setting
 * @param setting The fontSize setting from UserSettings
 * @returns Font size in pixels
 */
export function getFontSize(setting: UserSettings['fontSize']): number {
  switch (setting) {
    case 'auto':
      // Use 16px on mobile to prevent Safari auto-zoom, 14px on desktop
      return isMobileDevice() ? 16 : 14;
    case 'small':
      return 12;
    case 'medium':
      return 14;
    case 'large':
      return 16;
    default:
      return 14; // Fallback
  }
}

/**
 * Get the font size as a CSS string
 * @param setting The fontSize setting from UserSettings
 * @returns Font size as CSS string (e.g., "14px")
 */
export function getFontSizeCSS(setting: UserSettings['fontSize']): string {
  return `${getFontSize(setting)}px`;
}

/**
 * Get the minimum font size for form inputs (always 16px to prevent iOS auto-zoom)
 * @returns Minimum font size in pixels
 */
export function getMinInputFontSize(): number {
  return isMobileDevice() ? 16 : 14;
}
