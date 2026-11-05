/**
 * Narrows where the app can connect, once it knows which sync server is in use.
 *
 * The static policy has to allow any https host, because the sync server is
 * configured by the user. That would also let injected script post the decrypted
 * vault anywhere. Adding a second policy tightens the first, since a request must
 * pass both.
 *
 * Policies can only ever tighten, so this can't be undone without a page load.
 * Anything that changes the server must reload the page: see isConnectionRestricted.
 */

let restricted = false;

/** The origin to allow, or null if the endpoint isn't a usable server address. */
function originOf(endpoint: string): string | null {
  try {
    const url = new URL(endpoint);
    return url.protocol === 'https:' || url.protocol === 'http:' ? url.origin : null;
  } catch {
    return null;
  }
}

/** Limit connections to this origin and the given sync server, for this page load. */
export function restrictConnections(endpoint: string): void {
  if (restricted) return;

  const origin = originOf(endpoint);
  if (!origin) return;

  const meta = document.createElement('meta');
  meta.httpEquiv = 'Content-Security-Policy';
  meta.content = `connect-src 'self' blob: ${origin}`;
  document.head.appendChild(meta);
  restricted = true;
}

/** True if connections are limited for this page load, so a new server needs a reload. */
export function isConnectionRestricted(): boolean {
  return restricted;
}
