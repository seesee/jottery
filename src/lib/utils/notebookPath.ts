/**
 * Notebook Path Utility
 *
 * Provides path-based notebook isolation for Jottery.
 * Allows multiple independent notebooks at different URL paths (e.g., /recipes, /finances)
 * with separate passwords and databases.
 *
 * This is an undocumented developer convenience feature.
 */

export interface NotebookInfo {
  id: string;           // Notebook identifier (e.g., 'main', 'recipes', 'finances')
  dbName: string;       // IndexedDB database name (e.g., 'jottery', 'jottery-recipes')
  displayName: string;  // Human-readable name (e.g., 'Main', 'Recipes', 'Finances')
}

/**
 * Paths that should not be treated as notebooks
 */
const EXCLUDED_PATHS = ['/admin', '/api'];

/**
 * Maximum length for notebook IDs
 */
const MAX_NOTEBOOK_ID_LENGTH = 50;

/**
 * Valid notebook ID pattern: alphanumeric + hyphens/underscores
 */
const VALID_NOTEBOOK_ID_PATTERN = /^[a-z0-9][a-z0-9_-]*$/;

/**
 * Extract notebook information from current URL pathname
 *
 * @returns NotebookInfo for the current notebook
 */
export function getCurrentNotebook(): NotebookInfo {
  const pathname = window.location.pathname;
  return getNotebookFromPath(pathname);
}

/**
 * Extract notebook information from a given pathname
 *
 * @param pathname - URL pathname to parse
 * @returns NotebookInfo for the given path
 */
export function getNotebookFromPath(pathname: string): NotebookInfo {
  // Normalize path: remove trailing slash, extract first segment
  const normalizedPath = pathname.replace(/\/$/, '');
  const segments = normalizedPath.split('/').filter(s => s.length > 0);

  // Root path = default 'main' notebook
  if (segments.length === 0) {
    return {
      id: 'main',
      dbName: 'jottery', // Backward compatible - no suffix for root
      displayName: 'Main',
    };
  }

  // Use first segment as notebook ID
  const notebookId = segments[0].toLowerCase();

  // Validate notebook ID
  if (!isValidNotebookId(notebookId)) {
    throw new Error(
      `Invalid notebook ID: "${notebookId}". ` +
      `Must be alphanumeric with hyphens/underscores, max ${MAX_NOTEBOOK_ID_LENGTH} characters.`
    );
  }

  return {
    id: notebookId,
    dbName: `jottery-${notebookId}`,
    displayName: formatDisplayName(notebookId),
  };
}

/**
 * Check if a pathname should be treated as a notebook path
 *
 * @param pathname - URL pathname to check
 * @returns true if path is valid notebook path, false if excluded
 */
export function isNotebookPath(pathname: string): boolean {
  // Check if path is excluded
  if (isExcludedPath(pathname)) {
    return false;
  }

  // Try to parse as notebook - if it throws, it's invalid
  try {
    getNotebookFromPath(pathname);
    return true;
  } catch {
    return false;
  }
}

/**
 * Check if a pathname is in the excluded paths list
 *
 * @param pathname - URL pathname to check
 * @returns true if path is excluded (e.g., /admin, /api)
 */
export function isExcludedPath(pathname: string): boolean {
  const normalizedPath = pathname.replace(/\/$/, '');

  return EXCLUDED_PATHS.some(excluded =>
    normalizedPath === excluded || normalizedPath.startsWith(excluded + '/')
  );
}

/**
 * Validate a notebook ID
 *
 * @param id - Notebook ID to validate
 * @returns true if valid, false otherwise
 */
function isValidNotebookId(id: string): boolean {
  if (!id || id.length === 0) {
    return false;
  }

  if (id.length > MAX_NOTEBOOK_ID_LENGTH) {
    return false;
  }

  return VALID_NOTEBOOK_ID_PATTERN.test(id);
}

/**
 * Format notebook ID into display name
 *
 * @param id - Notebook ID
 * @returns Formatted display name
 */
function formatDisplayName(id: string): string {
  // Replace hyphens and underscores with spaces
  const withSpaces = id.replace(/[-_]/g, ' ');

  // Capitalize first letter of each word
  return withSpaces
    .split(' ')
    .map(word => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ');
}
