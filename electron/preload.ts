import { contextBridge, ipcRenderer } from 'electron';

/**
 * Electron API exposed to the renderer process via contextBridge.
 * This provides a secure way to access Electron features from the web app.
 */
export interface ElectronAPI {
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
    openFile: (options?: FileDialogOptions) => Promise<FileResult | null>;
    saveFile: (data: string, options?: SaveDialogOptions) => Promise<string | null>;
    openDirectory: () => Promise<string | null>;
  };

  // Auto-updater
  updater: {
    checkForUpdates: () => void;
    downloadUpdate: () => void;
    installUpdate: () => void;
    onUpdateAvailable: (callback: (info: UpdateInfo) => void) => void;
    onUpdateDownloaded: (callback: (info: UpdateInfo) => void) => void;
    onDownloadProgress: (callback: (progress: DownloadProgress) => void) => void;
    onUpdateError: (callback: (error: string) => void) => void;
  };
}

interface FileDialogOptions {
  title?: string;
  filters?: Array<{ name: string; extensions: string[] }>;
  defaultPath?: string;
}

interface SaveDialogOptions {
  title?: string;
  defaultPath?: string;
  filters?: Array<{ name: string; extensions: string[] }>;
}

interface FileResult {
  path: string;
  name: string;
  data: string;
}

interface UpdateInfo {
  version: string;
  releaseNotes?: string;
}

interface DownloadProgress {
  percent: number;
  bytesPerSecond: number;
  total: number;
  transferred: number;
}

// Expose the API to the renderer process
contextBridge.exposeInMainWorld('electronAPI', {
  // App info
  getVersion: () => ipcRenderer.invoke('app:getVersion'),
  getPlatform: () => ipcRenderer.invoke('app:getPlatform'),
  getAppPath: () => ipcRenderer.invoke('app:getAppPath'),

  // Theme
  getTheme: () => ipcRenderer.invoke('app:getTheme'),
  setTheme: (theme: 'light' | 'dark' | 'system') => ipcRenderer.send('app:setTheme', theme),
  onThemeChange: (callback: (theme: 'light' | 'dark') => void) => {
    ipcRenderer.on('app:themeChanged', (_event, theme) => callback(theme));
  },

  // Window controls
  minimize: () => ipcRenderer.send('window:minimize'),
  maximize: () => ipcRenderer.send('window:maximize'),
  close: () => ipcRenderer.send('window:close'),

  // Password storage
  password: {
    get: (service: string, account: string) =>
      ipcRenderer.invoke('password:get', service, account),
    set: (service: string, account: string, password: string) =>
      ipcRenderer.invoke('password:set', service, account, password),
    delete: (service: string, account: string) =>
      ipcRenderer.invoke('password:delete', service, account),
    isAvailable: () => ipcRenderer.invoke('password:isAvailable'),
  },

  // File operations
  file: {
    openFile: (options?: FileDialogOptions) =>
      ipcRenderer.invoke('file:open', options),
    saveFile: (data: string, options?: SaveDialogOptions) =>
      ipcRenderer.invoke('file:save', data, options),
    openDirectory: () => ipcRenderer.invoke('file:openDirectory'),
  },

  // Auto-updater
  updater: {
    checkForUpdates: () => ipcRenderer.send('updater:checkForUpdates'),
    downloadUpdate: () => ipcRenderer.send('updater:downloadUpdate'),
    installUpdate: () => ipcRenderer.send('updater:installUpdate'),
    onUpdateAvailable: (callback: (info: UpdateInfo) => void) => {
      ipcRenderer.on('updater:updateAvailable', (_event, info) => callback(info));
    },
    onUpdateDownloaded: (callback: (info: UpdateInfo) => void) => {
      ipcRenderer.on('updater:updateDownloaded', (_event, info) => callback(info));
    },
    onDownloadProgress: (callback: (progress: DownloadProgress) => void) => {
      ipcRenderer.on('updater:downloadProgress', (_event, progress) => callback(progress));
    },
    onUpdateError: (callback: (error: string) => void) => {
      ipcRenderer.on('updater:error', (_event, error) => callback(error));
    },
  },
} satisfies ElectronAPI);

// Type declaration for the global window object
declare global {
  interface Window {
    electronAPI?: ElectronAPI;
  }
}
