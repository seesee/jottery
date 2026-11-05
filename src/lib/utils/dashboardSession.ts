/**
 * The admin dashboard and user portal keep their session in sessionStorage, which
 * belongs to the tab rather than the page. They share an origin with the notes app,
 * so a dashboard tab navigated here would otherwise leave its token readable by
 * anything that ran in a note.
 */
const DASHBOARD_SESSION_KEYS = ['admin_session_token', 'user_session_token'];

export function clearDashboardSessions(): void {
  try {
    for (const key of DASHBOARD_SESSION_KEYS) {
      sessionStorage.removeItem(key);
    }
  } catch {
    // Storage can be blocked entirely, in which case there is nothing to clear
  }
}
