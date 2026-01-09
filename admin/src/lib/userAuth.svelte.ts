// User portal authentication state management
import { userApi } from './userApi';
import type { LoginResponse } from './userApi';

interface UserAuthState {
  isAuthenticated: boolean;
  user: LoginResponse['user'] | null;
}

class UserAuthStore {
  private state = $state<UserAuthState>({
    isAuthenticated: userApi.isAuthenticated(),
    user: null,
  });

  get isAuthenticated() {
    return this.state.isAuthenticated;
  }

  get user() {
    return this.state.user;
  }

  async login(email: string, password: string): Promise<void> {
    const response = await userApi.login(email, password);
    this.state.isAuthenticated = true;
    this.state.user = response.user;
  }

  async logout(): Promise<void> {
    await userApi.logout();
    this.state.isAuthenticated = false;
    this.state.user = null;
  }

  clearAuth(): void {
    // Called when account is deactivated/deleted
    this.state.isAuthenticated = false;
    this.state.user = null;
  }
}

export const userAuth = new UserAuthStore();
