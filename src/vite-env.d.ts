/// <reference types="svelte" />
/// <reference types="vite/client" />

declare const __APP_VERSION__: string;

/**
 * Electron API exposed to renderer via contextBridge (preload.ts)
 * Only available when running in Electron desktop app
 */
interface ElectronAPI {
  // App info
  getVersion: () => Promise<string>;
  getPlatform: () => Promise<NodeJS.Platform>;
  getAppPath: () => Promise<string>;

  // Theme
  getTheme: () => Promise<'light' | 'dark'>;
  setTheme: (theme: 'light' | 'dark' | 'system') => void;
  onThemeChange: (callback: (theme: 'light' | 'dark') => void) => void;

  // Window controls
  minimize: () => void;
  maximize: () => void;
  close: () => void;

  // Password storage (OS keychain)
  password: {
    get: (service: string, account: string) => Promise<string | null>;
    set: (service: string, account: string, password: string) => Promise<void>;
    delete: (service: string, account: string) => Promise<boolean>;
    isAvailable: () => Promise<boolean>;
  };

  // File operations (native dialogs)
  file: {
    openFile: (options?: {
      title?: string;
      filters?: Array<{ name: string; extensions: string[] }>;
      defaultPath?: string;
    }) => Promise<{ path: string; name: string; data: string } | null>;
    saveFile: (data: string, options?: {
      title?: string;
      defaultPath?: string;
      filters?: Array<{ name: string; extensions: string[] }>;
    }) => Promise<string | null>;
    openDirectory: () => Promise<string | null>;
  };

  // Auto-updater
  updater: {
    checkForUpdates: () => void;
    downloadUpdate: () => void;
    installUpdate: () => void;
    onUpdateAvailable: (callback: (info: { version: string; releaseNotes?: string }) => void) => void;
    onUpdateDownloaded: (callback: (info: { version: string; releaseNotes?: string }) => void) => void;
    onDownloadProgress: (callback: (progress: {
      percent: number;
      bytesPerSecond: number;
      total: number;
      transferred: number;
    }) => void) => void;
    onUpdateError: (callback: (error: string) => void) => void;
  };
}

interface Window {
  electronAPI?: ElectronAPI;
}

