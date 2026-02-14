/**
 * Capacitor lifecycle service
 *
 * Handles native app lifecycle events when running inside Capacitor:
 * - App state changes (foreground/background) for sync triggers
 * - Status bar styling to match current theme
 * - Keyboard show/hide to set --keyboard-height CSS variable
 */

import { App } from '@capacitor/app';
import { StatusBar, Style } from '@capacitor/status-bar';
import { Keyboard } from '@capacitor/keyboard';
import { getNativePlatform } from '../utils/device';
import { syncService } from './syncService';
import { settings } from '../stores/appStore';
import { get } from 'svelte/store';

let initialized = false;
let settingsUnsubscribe: (() => void) | null = null;

/**
 * Update the status bar style to match the current theme
 */
async function updateStatusBarStyle(): Promise<void> {
  const currentSettings = get(settings);
  const isDark = currentSettings.theme === 'dark' ||
    (currentSettings.theme === 'auto' &&
      typeof window !== 'undefined' &&
      window.matchMedia('(prefers-color-scheme: dark)').matches);

  try {
    await StatusBar.setStyle({
      style: isDark ? Style.Dark : Style.Light,
    });
  } catch (e) {
    // StatusBar plugin may not be available on all platforms
    console.debug('[CapacitorService] StatusBar.setStyle failed:', e);
  }
}

/**
 * Initialise Capacitor lifecycle handlers.
 * Call once from App.svelte onMount, guarded by isCapacitor().
 */
export async function initCapacitorService(): Promise<void> {
  if (initialized) return;
  initialized = true;

  const platform = getNativePlatform();
  console.log(`[CapacitorService] Initialising for platform: ${platform}`);

  // --- App state changes ---
  await App.addListener('appStateChange', ({ isActive }) => {
    if (isActive) {
      // Returning to foreground — pull latest changes
      syncService.triggerBackgroundSync();
    } else {
      // Going to background — push any pending changes
      syncService.syncNow();
    }
  });

  // --- Status bar ---
  // Set initial style
  await updateStatusBarStyle();

  // Watch for theme changes in settings
  settingsUnsubscribe = settings.subscribe(() => {
    updateStatusBarStyle();
  });

  // Also listen for system theme changes (for 'auto' mode)
  if (typeof window !== 'undefined') {
    window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
      updateStatusBarStyle();
    });
  }

  // --- Keyboard (iOS) ---
  if (platform === 'ios') {
    await Keyboard.addListener('keyboardWillShow', (info) => {
      document.documentElement.style.setProperty(
        '--keyboard-height',
        `${info.keyboardHeight}px`,
      );
    });

    await Keyboard.addListener('keyboardWillHide', () => {
      document.documentElement.style.setProperty('--keyboard-height', '0px');
    });
  }

  // --- Back button (Android, future) ---
  if (platform === 'android') {
    await App.addListener('backButton', ({ canGoBack }) => {
      if (canGoBack) {
        window.history.back();
      } else {
        App.exitApp();
      }
    });
  }
}

/**
 * Clean up listeners (call on app destroy if needed)
 */
export async function destroyCapacitorService(): Promise<void> {
  if (!initialized) return;
  await App.removeAllListeners();
  await Keyboard.removeAllListeners();
  if (settingsUnsubscribe) {
    settingsUnsubscribe();
    settingsUnsubscribe = null;
  }
  initialized = false;
}
