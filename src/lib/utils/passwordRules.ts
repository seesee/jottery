/**
 * Rules for the notes password. Every client applies the same ones.
 *
 * The notes password protects the master key. The account password is sent to the
 * sync server, so if the two match, the server can unwrap the key it stores and
 * read every note.
 */

/** Applies to new vaults and password changes, never at unlock: existing short passwords must keep working. */
export const MIN_NOTES_PASSWORD_LENGTH = 8;

export function isNotesPasswordLongEnough(password: string): boolean {
  return password.length >= MIN_NOTES_PASSWORD_LENGTH;
}

/** Compared locally, wherever both passwords are known. */
export function isSamePassword(notesPassword: string, accountPassword: string): boolean {
  return notesPassword === accountPassword;
}
