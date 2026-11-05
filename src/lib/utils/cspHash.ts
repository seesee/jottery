/**
 * Build step: allow the app's own inlined script in the Content Security Policy by
 * hash, so the policy doesn't need 'unsafe-inline'. That keyword would also allow
 * every inline event handler and javascript: URL, so any HTML injection would run.
 *
 * Used from vite.config.ts only. It never reaches the browser bundle.
 */
import { createHash } from 'node:crypto';

const INLINE_SCRIPT = /<script\b(?![^>]*\ssrc=)[^>]*>([\s\S]*?)<\/script>/gi;
const CSP_META = /(<meta\s+http-equiv="Content-Security-Policy"\s+content=")([^"]*)(")/i;

/** The CSP source for each inline script, computed over what the browser hashes. */
export function inlineScriptHashes(html: string): string[] {
  return Array.from(html.matchAll(INLINE_SCRIPT), match => {
    // The HTML parser normalises line endings before the script text exists
    const text = match[1].replace(/\r\n?/g, '\n');
    return `sha256-${createHash('sha256').update(text, 'utf8').digest('base64')}`;
  });
}

/**
 * Add the hash of every inline script to the page's script-src. Anything unexpected
 * fails the build: a wrong policy here means an app that won't start, or one that
 * isn't protected.
 */
export function addInlineScriptHashes(html: string): string {
  const meta = html.match(CSP_META);
  if (!meta) {
    throw new Error('Content Security Policy meta tag not found in the built page');
  }

  if (!/(^|;\s*)script-src\s/.test(meta[2])) {
    throw new Error('The Content Security Policy has no script-src directive');
  }

  const hashes = inlineScriptHashes(html);
  if (hashes.length === 0) {
    throw new Error('No inline script found to hash in the built page');
  }

  const sources = hashes.map(hash => `'${hash}'`).join(' ');
  const policy = meta[2].replace(/(^|;\s*)(script-src\s[^;]*)/, (_, lead, directive) => `${lead}${directive} ${sources}`);

  return html.replace(CSP_META, (_, start, __, end) => `${start}${policy}${end}`);
}
