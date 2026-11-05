// Turns a failed request into text for the reader, in their language.

import { ApiError } from './dashboardClient';

export type Translate = (key: string, options?: { values?: Record<string, string | number> }) => string;

// Server messages that have a translation. The server only sends English.
export const KNOWN_MESSAGES: Record<string, string> = {
  'New password must be at least 12 characters': 'errors.passwordTooShort',
  'New password must be different from the current one': 'errors.passwordSameAsCurrent',
  'Cannot remove your own admin privileges': 'errors.ownAdminRole',
  'Cannot demote the last admin user': 'errors.lastAdminDemote',
  'Cannot delete your own account': 'errors.ownAccountDelete',
  'Cannot delete the last admin user': 'errors.lastAdminDelete',
  'User not found': 'errors.userNotFound',
  'Device not found': 'errors.deviceNotFound',
  'Device name cannot be empty': 'devices.nameRequired',
  'Storage quota must be between 1 and 100000 MB': 'errors.quotaRange',
  'Max upload size must be between 1 and 1000 MB': 'errors.maxUploadRange',
  'Inbox max items must be between 1 and 10000': 'errors.inboxItemsRange',
  'Inbox max size must be between 1 and 1000 MB': 'errors.inboxSizeRange',
};

/**
 * What to show for an error. `fallbackKey` is used when there's nothing more specific
 * to say, such as a 500 with no body.
 */
export function describeError(err: unknown, t: Translate, fallbackKey = 'errors.unknown'): string {
  if (err instanceof TypeError) {
    return t('errors.network'); // fetch rejects with this when the server can't be reached
  }
  if (!(err instanceof ApiError)) {
    return t(fallbackKey);
  }

  const known = KNOWN_MESSAGES[err.message];
  if (known) {
    return t(known);
  }
  // A rejected session goes back to the sign-in page, so a page only sees a 401 from
  // a route that checks a password.
  if (err.status === 401) {
    return t('errors.wrongPassword');
  }
  if (err.status === 429) {
    return err.retryAfter
      ? t('errors.rateLimited', { values: { seconds: err.retryAfter } })
      : t('errors.tooManyAttempts');
  }
  if (err.message) {
    return err.message;
  }
  return t(err.status === 403 ? 'errors.forbidden' : fallbackKey);
}
