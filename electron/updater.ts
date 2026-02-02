import { BrowserWindow, ipcMain } from 'electron';
import { autoUpdater, UpdateInfo } from 'electron-updater';

/**
 * Setup auto-updater with GitHub Releases.
 * Sends events to the renderer process for UI updates.
 */
export function setupAutoUpdater(mainWindow: BrowserWindow | null): void {
  // Configure auto-updater
  autoUpdater.autoDownload = false;
  autoUpdater.autoInstallOnAppQuit = true;

  // Log to console for debugging
  autoUpdater.logger = console;

  // Send update status to renderer
  function sendToWindow(channel: string, ...args: unknown[]): void {
    mainWindow?.webContents.send(channel, ...args);
  }

  // Update events
  autoUpdater.on('checking-for-update', () => {
    console.log('Checking for update...');
  });

  autoUpdater.on('update-available', (info: UpdateInfo) => {
    console.log('Update available:', info.version);
    sendToWindow('updater:updateAvailable', {
      version: info.version,
      releaseNotes: typeof info.releaseNotes === 'string' ? info.releaseNotes : undefined,
    });
  });

  autoUpdater.on('update-not-available', () => {
    console.log('Update not available');
  });

  autoUpdater.on('download-progress', (progress) => {
    console.log(`Download progress: ${progress.percent.toFixed(1)}%`);
    sendToWindow('updater:downloadProgress', {
      percent: progress.percent,
      bytesPerSecond: progress.bytesPerSecond,
      total: progress.total,
      transferred: progress.transferred,
    });
  });

  autoUpdater.on('update-downloaded', (info: UpdateInfo) => {
    console.log('Update downloaded:', info.version);
    sendToWindow('updater:updateDownloaded', {
      version: info.version,
      releaseNotes: typeof info.releaseNotes === 'string' ? info.releaseNotes : undefined,
    });
  });

  autoUpdater.on('error', (error) => {
    console.error('Auto-updater error:', error);
    sendToWindow('updater:error', error.message);
  });

  // IPC handlers for manual update control
  ipcMain.on('updater:checkForUpdates', () => {
    autoUpdater.checkForUpdates().catch((error) => {
      console.error('Failed to check for updates:', error);
      sendToWindow('updater:error', error.message);
    });
  });

  ipcMain.on('updater:downloadUpdate', () => {
    autoUpdater.downloadUpdate().catch((error) => {
      console.error('Failed to download update:', error);
      sendToWindow('updater:error', error.message);
    });
  });

  ipcMain.on('updater:installUpdate', () => {
    // Quit and install immediately
    autoUpdater.quitAndInstall(false, true);
  });

  // Check for updates on startup (with delay to not block app start)
  setTimeout(() => {
    autoUpdater.checkForUpdates().catch((error) => {
      console.error('Failed to check for updates on startup:', error);
    });
  }, 5000);
}
