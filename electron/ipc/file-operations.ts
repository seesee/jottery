import { ipcMain, dialog, BrowserWindow } from 'electron';
import { readFile, writeFile } from 'fs/promises';
import path from 'path';

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

/**
 * Setup IPC handlers for native file operations.
 * Provides native open/save dialogs for import/export functionality.
 */
export function setupFileOperations(): void {
  // Open file dialog and read file contents
  ipcMain.handle(
    'file:open',
    async (_event, options?: FileDialogOptions): Promise<FileResult | null> => {
      const mainWindow = BrowserWindow.getFocusedWindow();

      const defaultFilters = [
        { name: 'JSON Files', extensions: ['json'] },
        { name: 'All Files', extensions: ['*'] },
      ];

      const result = await dialog.showOpenDialog(mainWindow!, {
        title: options?.title ?? 'Open File',
        filters: options?.filters ?? defaultFilters,
        defaultPath: options?.defaultPath,
        properties: ['openFile'],
      });

      if (result.canceled || result.filePaths.length === 0) {
        return null;
      }

      const filePath = result.filePaths[0];
      try {
        const data = await readFile(filePath, 'utf-8');
        return {
          path: filePath,
          name: path.basename(filePath),
          data,
        };
      } catch (error) {
        console.error('Failed to read file:', error);
        throw new Error(`Failed to read file: ${error instanceof Error ? error.message : 'Unknown error'}`);
      }
    }
  );

  // Save file dialog and write file contents
  ipcMain.handle(
    'file:save',
    async (_event, data: string, options?: SaveDialogOptions): Promise<string | null> => {
      const mainWindow = BrowserWindow.getFocusedWindow();

      const defaultFilters = [
        { name: 'JSON Files', extensions: ['json'] },
        { name: 'All Files', extensions: ['*'] },
      ];

      const result = await dialog.showSaveDialog(mainWindow!, {
        title: options?.title ?? 'Save File',
        filters: options?.filters ?? defaultFilters,
        defaultPath: options?.defaultPath,
      });

      if (result.canceled || !result.filePath) {
        return null;
      }

      try {
        await writeFile(result.filePath, data, 'utf-8');
        return result.filePath;
      } catch (error) {
        console.error('Failed to write file:', error);
        throw new Error(`Failed to write file: ${error instanceof Error ? error.message : 'Unknown error'}`);
      }
    }
  );

  // Open directory dialog
  ipcMain.handle('file:openDirectory', async (): Promise<string | null> => {
    const mainWindow = BrowserWindow.getFocusedWindow();

    const result = await dialog.showOpenDialog(mainWindow!, {
      title: 'Select Directory',
      properties: ['openDirectory', 'createDirectory'],
    });

    if (result.canceled || result.filePaths.length === 0) {
      return null;
    }

    return result.filePaths[0];
  });
}
