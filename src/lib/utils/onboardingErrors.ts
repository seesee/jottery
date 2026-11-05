/**
 * Translation keys for the onboarding failures a user can do something about.
 */
import { AccountKeyUnreadableError, LocalNotesOrphanedError } from '../errors';

export function onboardingErrorKey(error: unknown): string | null {
  if (error instanceof AccountKeyUnreadableError) {
    return 'unlock.connectExisting.errors.accountKeyUnreadable';
  }
  if (error instanceof LocalNotesOrphanedError) {
    return 'unlock.connectExisting.errors.localNotesOrphaned';
  }
  return null;
}
