// Where a dashboard keeps its session.
//
// The dashboards share an origin with the notes app, so a token in localStorage
// can be read by any script running in the notes app, in any tab, and a flaw in
// note rendering becomes a server-wide admin compromise. sessionStorage belongs
// to one tab, so a notes tab can't reach the token held by a dashboard tab, and
// it still survives a reload. The token is gone when the tab closes.
//
// A server that supports it holds the session in an HttpOnly cookie instead,
// which script can't read at all. Only a marker is kept here then, so the tab
// knows to rely on the cookie.

const COOKIE_SESSION = 'cookie';

// Sent with cookie sessions, so the server can refuse state-changing requests
// that a cross-site form could have made.
const CSRF_HEADER = 'X-Requested-With';
const CSRF_HEADER_VALUE = 'JotteryDashboard';

/** The parts of a login response that decide how the session is kept. */
export interface SessionGrant {
  sessionId?: string;
  cookieSession?: boolean;
}

export class DashboardSession {
  private value: string | null;

  constructor(private readonly storageKey: string) {
    // Earlier versions kept the token here. Don't carry it over: leaving a live
    // token in storage the notes app can read is what this class exists to stop.
    localStorage.removeItem(storageKey);
    this.value = sessionStorage.getItem(storageKey);
  }

  get isActive(): boolean {
    return this.value !== null;
  }

  get usesCookie(): boolean {
    return this.value === COOKIE_SESSION;
  }

  /** Headers that authenticate a request. */
  authHeaders(): Record<string, string> {
    if (this.value === null) {
      return {};
    }
    if (this.usesCookie) {
      return { [CSRF_HEADER]: CSRF_HEADER_VALUE };
    }
    return { Authorization: `Bearer ${this.value}` };
  }

  /** Records the session a login returned. */
  start(grant: SessionGrant): void {
    const value = grant.cookieSession ? COOKIE_SESSION : grant.sessionId;
    if (!value) {
      throw new Error('Login response contained no session');
    }
    this.value = value;
    sessionStorage.setItem(this.storageKey, value);
  }

  clear(): void {
    this.value = null;
    sessionStorage.removeItem(this.storageKey);
  }
}
