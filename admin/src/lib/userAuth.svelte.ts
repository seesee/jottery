// User portal authentication state management
import { userApi } from './userApi';
import type { LoginResponse } from './userApi';

interface UserAuthState {
  isAuthenticated: boolean;
  user: LoginResponse['user'] | null;
  /// True when the server accepted the password but expects a passkey
  /// assertion before the session can access anything. The UI should
  /// show the WebAuthn prompt and not yet mark the user as logged in.
  mfaPending: boolean;
}

class UserAuthStore {
  private state = $state<UserAuthState>({
    isAuthenticated: userApi.isAuthenticated(),
    user: null,
    mfaPending: false,
  });

  get isAuthenticated() {
    return this.state.isAuthenticated;
  }

  get user() {
    return this.state.user;
  }

  get mfaPending() {
    return this.state.mfaPending;
  }

  /// Password-stage login. If the response flags mfaRequired, `mfaPending`
  /// is set to true and `isAuthenticated` stays false until
  /// `completeMfa()` succeeds.
  async login(email: string, password: string): Promise<void> {
    const response = await userApi.login(email, password);
    if (response.mfaRequired) {
      this.state.user = response.user;
      this.state.mfaPending = true;
      this.state.isAuthenticated = false;
    } else {
      this.state.isAuthenticated = true;
      this.state.user = response.user;
      this.state.mfaPending = false;
    }
  }

  /// Second-stage passkey assertion. Flips the session from MFA-pending
  /// to fully authenticated on success; rethrows on failure so the UI
  /// can show an error and optionally offer retry.
  async completeMfa(): Promise<void> {
    await userApi.completePasskeyAuthentication();
    this.state.isAuthenticated = true;
    this.state.mfaPending = false;
  }

  async logout(): Promise<void> {
    await userApi.logout();
    this.state.isAuthenticated = false;
    this.state.user = null;
    this.state.mfaPending = false;
  }

  clearAuth(): void {
    // Called when account is deactivated/deleted
    this.state.isAuthenticated = false;
    this.state.user = null;
    this.state.mfaPending = false;
  }
}

export const userAuth = new UserAuthStore();
