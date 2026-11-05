// Which accounts the dashboard mustn't offer to remove. The server has the final say;
// this keeps the actions that would lock everyone out from appearing in the first place.

interface ListedUser {
  email: string;
  approved: boolean;
  isAdmin: boolean;
  isActive: boolean;
}

function canAdminister(user: ListedUser): boolean {
  return user.approved && user.isAdmin && user.isActive;
}

/**
 * True for the signed-in admin's own row, and for the only admin left. The second
 * check still protects that row when a reload couldn't find out who is signed in.
 */
export function isProtectedAccount(user: ListedUser, users: ListedUser[], signedInEmail?: string): boolean {
  if (signedInEmail && user.email.toLowerCase() === signedInEmail.toLowerCase()) {
    return true;
  }
  return canAdminister(user) && users.filter(canAdminister).length === 1;
}
