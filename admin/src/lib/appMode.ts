/**
 * App mode detection based on URL path
 * Allows the same SPA to serve both admin dashboard and user portal
 */

export type AppMode = 'admin' | 'user';

/**
 * Detect app mode from current URL path
 */
export function getAppMode(): AppMode {
  const path = window.location.pathname;
  if (path.startsWith('/user')) {
    return 'user';
  }
  return 'admin';
}

/**
 * Current app mode (evaluated once at module load)
 */
export const appMode = getAppMode();

/**
 * Convenience flags
 */
export const isUserPortal = appMode === 'user';
export const isAdminPortal = appMode === 'admin';
