// Sign-in state for a dashboard. The admin dashboard and the user portal behave the
// same way here and differ only in which routes they call.

interface SignedInUser {
  email: string;
}

interface LoginResult {
  user: SignedInUser;
  mustChangePassword?: boolean;
}

/** The parts of an API client that sign-in state depends on. */
export interface AuthBackend {
  isAuthenticated(): boolean;
  login(email: string, password: string): Promise<LoginResult>;
  logout(): Promise<void>;
  changePassword(currentPassword: string, newPassword: string): Promise<void>;
  currentUser(): Promise<SignedInUser>;
  onSessionExpired(listener: () => void): void;
  onPasswordChangeRequired(listener: () => void): void;
}

interface AuthState {
  isAuthenticated: boolean;
  user: SignedInUser | null;
  restoring: boolean;
  sessionExpired: boolean;
  mustChangePassword: boolean;
}

export class DashboardAuth {
  private state = $state<AuthState>({
    isAuthenticated: false,
    user: null,
    restoring: false,
    sessionExpired: false,
    mustChangePassword: false,
  });

  constructor(private readonly backend: AuthBackend) {
    const signedIn = backend.isAuthenticated();
    this.state.isAuthenticated = signedIn;
    this.state.restoring = signedIn;

    backend.onSessionExpired(() => {
      this.signOutLocally();
      this.state.sessionExpired = true;
    });
    backend.onPasswordChangeRequired(() => {
      this.state.mustChangePassword = true;
    });
  }

  get isAuthenticated() {
    return this.state.isAuthenticated;
  }

  get user() {
    return this.state.user;
  }

  /** True until `restore` has found out whether a stored session still works. */
  get restoring() {
    return this.state.restoring;
  }

  /** True when the sign-in page is showing because the session ended by itself. */
  get sessionExpired() {
    return this.state.sessionExpired;
  }

  /** True when the server refuses everything else until the password is changed. */
  get mustChangePassword() {
    return this.state.mustChangePassword;
  }

  /**
   * Checks a session left over from before a reload, and finds out whose it is. A
   * rejected session reaches the listeners set up above. If the server can't be
   * reached the session is kept, so the pages can report that themselves.
   */
  async restore(): Promise<void> {
    if (!this.state.isAuthenticated) {
      this.state.restoring = false;
      return;
    }
    try {
      this.state.user = await this.backend.currentUser();
    } catch (e) {
      console.warn('Could not restore the session:', e);
    } finally {
      this.state.restoring = false;
    }
  }

  async login(email: string, password: string): Promise<void> {
    const response = await this.backend.login(email, password);
    this.state.isAuthenticated = true;
    this.state.user = response.user;
    this.state.sessionExpired = false;
    this.state.mustChangePassword = response.mustChangePassword === true;
  }

  async logout(): Promise<void> {
    await this.backend.logout();
    this.signOutLocally();
    this.state.sessionExpired = false;
  }

  async changePassword(currentPassword: string, newPassword: string): Promise<void> {
    await this.backend.changePassword(currentPassword, newPassword);
    this.state.mustChangePassword = false;
    if (!this.state.user) {
      await this.restore();
    }
  }

  /** For when the account has gone, such as after deleting it. */
  clearAuth(): void {
    this.signOutLocally();
  }

  private signOutLocally(): void {
    this.state.isAuthenticated = false;
    this.state.user = null;
    this.state.mustChangePassword = false;
  }
}
