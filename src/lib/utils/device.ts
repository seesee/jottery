/**
 * Device detection utilities for mobile-specific features
 */

import { Capacitor } from '@capacitor/core';

/**
 * Check if running inside a Capacitor native shell (iOS/Android)
 * @returns true if running in Capacitor
 */
export function isCapacitor(): boolean {
  return Capacitor.isNativePlatform();
}

/**
 * Get the native platform name when running in Capacitor
 * @returns 'ios', 'android', or 'web'
 */
export function getNativePlatform(): 'ios' | 'android' | 'web' {
  return Capacitor.getPlatform() as 'ios' | 'android' | 'web';
}

/**
 * Check if running inside Electron desktop app
 * @returns true if running in Electron
 */
export function isElectron(): boolean {
  if (typeof window === 'undefined') return false;
  return (
    typeof window.electronAPI !== 'undefined' ||
    (typeof navigator !== 'undefined' &&
      navigator.userAgent.toLowerCase().includes('electron'))
  );
}

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
