// What both dashboards share when talking to the server: sending a request with the
// session, and working out what a refused request means for that session.

import { DashboardSession } from './session';

const API_BASE = import.meta.env.VITE_API_BASE_URL || '';

const PASSWORD_CHANGE_REQUIRED = 'password_change_required';

/**
 * A request the server refused. `message` is the server's own English text, or empty
 * if it sent none. Pages show `describeError` from apiErrors.ts rather than this.
 */
export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
    public code?: string,
    public retryAfter?: number,
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

export interface RequestOptions {
  /**
   * For a route that verifies a password. Its 401 with a body means the password was
   * wrong, not that the session ended. The session middleware's 401 has no body.
   */
  checksPassword?: boolean;
}

interface ErrorBody {
  error?: unknown;
  code?: unknown;
  retryAfter?: unknown;
}

async function readErrorBody(response: Response): Promise<ErrorBody | null> {
  const body: unknown = await response.json().catch(() => null);
  return typeof body === 'object' && body !== null ? (body as ErrorBody) : null;
}

export class DashboardClient {
  readonly session: DashboardSession;
  private sessionExpired: (() => void) | null = null;
  private passwordChangeRequired: (() => void) | null = null;

  constructor(storageKey: string) {
    this.session = new DashboardSession(storageKey);
  }

  /** Calls `listener` when the server stops accepting this session. */
  onSessionExpired(listener: () => void): void {
    this.sessionExpired = listener;
  }

  /** Calls `listener` when the server refuses a request until the password changes. */
  onPasswordChangeRequired(listener: () => void): void {
    this.passwordChangeRequired = listener;
  }

  async request<T>(method: string, path: string, body?: unknown, options: RequestOptions = {}): Promise<T> {
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      ...this.session.authHeaders(),
    };

    const response = await fetch(`${API_BASE}${path}`, {
      method,
      headers,
      body: body ? JSON.stringify(body) : undefined,
      credentials: 'include', // Include cookies
    });

    if (!response.ok) {
      const errorBody = await readErrorBody(response);
      this.handleRefusal(response.status, errorBody, options);
      throw new ApiError(
        response.status,
        typeof errorBody?.error === 'string' ? errorBody.error : '',
        typeof errorBody?.code === 'string' ? errorBody.code : undefined,
        typeof errorBody?.retryAfter === 'number' ? errorBody.retryAfter : undefined,
      );
    }

    // Handle 204 No Content
    if (response.status === 204) {
      return undefined as T;
    }

    return response.json();
  }

  private handleRefusal(status: number, body: ErrorBody | null, options: RequestOptions): void {
    if (status === 401 && !(options.checksPassword && body)) {
      this.expireSession();
    } else if (status === 403 && body?.code === PASSWORD_CHANGE_REQUIRED) {
      this.passwordChangeRequired?.();
    }
  }

  private expireSession(): void {
    // Several requests can fail together, and a failed login has no session to end
    if (!this.session.isActive) {
      return;
    }
    this.session.clear();
    this.sessionExpired?.();
  }
}
