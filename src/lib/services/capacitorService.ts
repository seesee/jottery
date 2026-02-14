/**
 * Capacitor lifecycle service
 *
 * Handles native app lifecycle events when running inside Capacitor:
 * - App state changes (foreground/background) for sync triggers
 * - Status bar styling to match current theme
 * - Keyboard show/hide to set --keyboard-height CSS variable
 * - Viewport locking (no pinch-zoom in native app)
 * - Dynamic Type text scaling from iOS accessibility settings
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
 * The default body font size on iOS (Dynamic Type "Large" / default).
 * -apple-system-body returns 17px at the default setting.
 */
const IOS_DEFAULT_BODY_PX = 17;

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
 * Lock the viewport so users cannot pinch-zoom the native app.
 * The web build keeps its existing viewport (allows zoom).
 */
function lockViewport(): void {
  const meta = document.querySelector('meta[name="viewport"]');
  if (meta) {
    meta.setAttribute(
      'content',
      'width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no, viewport-fit=cover',
    );
  }
}

/**
 * Read the user's preferred text size from iOS Dynamic Type and
 * apply it as a CSS custom property (--text-scale) on :root.
 *
 * We insert a hidden probe element styled with -apple-system-body,
 * read its computed font-size (which WKWebView adjusts to match the
 * Dynamic Type setting), then derive a scale factor relative to the
 * iOS default of 17 px.
 */
function applyDynamicTypeScale(): void {
  const probe = document.createElement('div');
  probe.style.cssText =
    'font: -apple-system-body; position: absolute; visibility: hidden; pointer-events: none;';
  document.body.appendChild(probe);

  const preferredPx = parseFloat(getComputedStyle(probe).fontSize);
  document.body.removeChild(probe);

  if (!preferredPx || isNaN(preferredPx)) return;

  const scale = preferredPx / IOS_DEFAULT_BODY_PX;
  document.documentElement.style.setProperty('--text-scale', scale.toFixed(3));
  console.log(
    `[CapacitorService] Dynamic Type: ${preferredPx}px → scale ${scale.toFixed(3)}`,
  );
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

  // --- Viewport ---
  lockViewport();

  // --- Dynamic Type (iOS) ---
  if (platform === 'ios') {
    applyDynamicTypeScale();
  }

  // --- App state changes ---
  await App.addListener('appStateChange', ({ isActive }) => {
    if (isActive) {
      // Returning to foreground — pull latest changes
      syncService.triggerBackgroundSync();
      // Re-read Dynamic Type in case the user changed text size in Settings
      if (platform === 'ios') {
        applyDynamicTypeScale();
      }
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
