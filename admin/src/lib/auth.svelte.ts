// Authentication state management
import { api } from './api';
import type { LoginResponse } from './api';

interface AuthState {
  isAuthenticated: boolean;
  user: LoginResponse['user'] | null;
}

class AuthStore {
  private state = $state<AuthState>({
    isAuthenticated: api.isAuthenticated(),
    user: null,
  });

  get isAuthenticated() {
    return this.state.isAuthenticated;
  }

  get user() {
    return this.state.user;
  }

  async login(email: string, password: string): Promise<void> {
    const response = await api.login(email, password);
    this.state.isAuthenticated = true;
    this.state.user = response.user;
  }

  logout(): void {
    api.logout();
    this.state.isAuthenticated = false;
    this.state.user = null;
  }
}

export const auth = new AuthStore();
