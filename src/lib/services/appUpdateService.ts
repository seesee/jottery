/**
 * App update checking service
 * Periodically checks for new app versions and notifies the user
 */

import { writable } from 'svelte/store';
import { isCapacitor } from '../utils/device';

interface VersionInfo {
  version: string;
  buildTime: string;
  buildHash: string;
}

// Store to track if an update is available
export const updateAvailable = writable<boolean>(false);
export const newVersionInfo = writable<VersionInfo | null>(null);

class AppUpdateService {
  private currentVersion: string;
  private currentBuildHash: string | null = null;
  private checkInterval: number | null = null;
  private readonly CHECK_INTERVAL_MS = 5 * 60 * 1000; // 5 minutes

  constructor() {
    this.currentVersion = __APP_VERSION__;
  }

  /**
   * Start periodic version checking
   */
  startChecking(): void {
    // Don't check in development mode
    if (import.meta.env.DEV) {
      console.log('[AppUpdateService] Skipping version checks in development mode');
      return;
    }

    // Don't check in Capacitor — bundled assets can't be updated via /version.json
    if (isCapacitor()) {
      console.log('[AppUpdateService] Skipping version checks in Capacitor');
      return;
    }

    console.log('[AppUpdateService] Starting version checks');

    // Check immediately on start
    this.checkForUpdate();

    // Then check periodically
    this.checkInterval = window.setInterval(() => {
      this.checkForUpdate();
    }, this.CHECK_INTERVAL_MS);
  }

  /**
   * Stop periodic version checking
   */
  stopChecking(): void {
    if (this.checkInterval !== null) {
      clearInterval(this.checkInterval);
      this.checkInterval = null;
      console.log('[AppUpdateService] Stopped version checks');
    }
  }

  /**
   * Check for a new version
   */
  private async checkForUpdate(): Promise<void> {
    try {
      // Fetch version.json with cache-busting
      const response = await fetch(`/version.json?t=${Date.now()}`, {
        cache: 'no-cache',
        headers: {
          'Cache-Control': 'no-cache',
        },
      });

      if (!response.ok) {
        console.warn('[AppUpdateService] Failed to fetch version.json:', response.status);
        return;
      }

      const versionInfo: VersionInfo = await response.json();

      // Store the build hash on first check
      if (this.currentBuildHash === null) {
        this.currentBuildHash = versionInfo.buildHash;
        console.log('[AppUpdateService] Current version:', {
          version: this.currentVersion,
          buildHash: this.currentBuildHash,
        });
        return;
      }

      // Check if the build hash has changed (more reliable than version for detecting updates)
      if (versionInfo.buildHash !== this.currentBuildHash) {
        console.log('[AppUpdateService] New version detected!', {
          current: { version: this.currentVersion, buildHash: this.currentBuildHash },
          new: versionInfo,
        });

        // Notify the app that an update is available
        newVersionInfo.set(versionInfo);
        updateAvailable.set(true);

        // Stop checking once we've detected an update
        this.stopChecking();
      } else {
        console.log('[AppUpdateService] No update available (current build hash:', this.currentBuildHash, ')');
      }
    } catch (error) {
      console.error('[AppUpdateService] Error checking for updates:', error);
    }
  }

  /**
   * Reload the app to get the new version
   */
  reloadApp(): void {
    console.log('[AppUpdateService] Reloading app...');
    window.location.reload();
  }
}

export const appUpdateService = new AppUpdateService();
