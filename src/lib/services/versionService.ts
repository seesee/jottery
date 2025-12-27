/**
 * Version checking service
 * Periodically checks for new versions and notifies the app
 */

import { writable } from 'svelte/store';

interface VersionInfo {
  version: string;
  buildTime: string;
  buildHash: string;
}

// Store to track if an update is available
export const updateAvailable = writable<boolean>(false);
export const newVersionInfo = writable<VersionInfo | null>(null);

class VersionService {
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
      console.log('[VersionService] Skipping version checks in development mode');
      return;
    }

    console.log('[VersionService] Starting version checks');

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
      console.log('[VersionService] Stopped version checks');
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
        console.warn('[VersionService] Failed to fetch version.json:', response.status);
        return;
      }

      const versionInfo: VersionInfo = await response.json();

      // Store the build hash on first check
      if (this.currentBuildHash === null) {
        this.currentBuildHash = versionInfo.buildHash;
        console.log('[VersionService] Current version:', {
          version: this.currentVersion,
          buildHash: this.currentBuildHash,
        });
        return;
      }

      // Check if the build hash has changed (more reliable than version for detecting updates)
      if (versionInfo.buildHash !== this.currentBuildHash) {
        console.log('[VersionService] New version detected!', {
          current: { version: this.currentVersion, buildHash: this.currentBuildHash },
          new: versionInfo,
        });

        // Notify the app that an update is available
        newVersionInfo.set(versionInfo);
        updateAvailable.set(true);

        // Stop checking once we've detected an update
        this.stopChecking();
      } else {
        console.log('[VersionService] No update available (current build hash:', this.currentBuildHash, ')');
      }
    } catch (error) {
      console.error('[VersionService] Error checking for updates:', error);
    }
  }

  /**
   * Reload the app to get the new version
   */
  reloadApp(): void {
    console.log('[VersionService] Reloading app...');
    window.location.reload();
  }
}

export const versionService = new VersionService();
