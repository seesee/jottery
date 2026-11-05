import { describe, it, expect, beforeEach } from 'vitest';
import { clearDashboardSessions } from './dashboardSession';

describe('clearDashboardSessions', () => {
  beforeEach(() => sessionStorage.clear());

  it('removes both dashboard tokens', () => {
    sessionStorage.setItem('admin_session_token', 'a');
    sessionStorage.setItem('user_session_token', 'b');

    clearDashboardSessions();

    expect(sessionStorage.getItem('admin_session_token')).toBeNull();
    expect(sessionStorage.getItem('user_session_token')).toBeNull();
  });

  it('leaves everything else alone', () => {
    sessionStorage.setItem('jottery_session', 'c');

    clearDashboardSessions();

    expect(sessionStorage.getItem('jottery_session')).toBe('c');
  });
});
