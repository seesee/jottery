import { describe, it, expect } from 'vitest';
import { isNotesPasswordLongEnough, isSamePassword, MIN_NOTES_PASSWORD_LENGTH } from './passwordRules';

describe('isNotesPasswordLongEnough', () => {
  it('uses the agreed minimum', () => {
    expect(MIN_NOTES_PASSWORD_LENGTH).toBe(8);
  });

  it('rejects anything shorter', () => {
    expect(isNotesPasswordLongEnough('')).toBe(false);
    expect(isNotesPasswordLongEnough('1234567')).toBe(false);
  });

  it('accepts the minimum and above', () => {
    expect(isNotesPasswordLongEnough('12345678')).toBe(true);
    expect(isNotesPasswordLongEnough('пароль-длинный')).toBe(true);
  });
});

describe('isSamePassword', () => {
  it('detects a match', () => {
    expect(isSamePassword('same-password', 'same-password')).toBe(true);
  });

  it('is exact', () => {
    expect(isSamePassword('same-password', 'Same-password')).toBe(false);
    expect(isSamePassword('same-password', 'same-password ')).toBe(false);
  });
});
