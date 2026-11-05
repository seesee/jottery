// Pins the preview bundle's one inline script with a CSP hash. Run after vite,
// because the hash depends on the final bytes: nothing else can run in the page,
// and the only images it can load are the data: URLs the native side supplies.
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';

const file = process.argv[2] ?? 'dist/preview.html';
const html = readFileSync(file, 'utf8');

const scripts = [...html.matchAll(/<script\b[^>]*>([\s\S]*?)<\/script\b[^>]*>/gi)];
if (scripts.length !== 1) {
  throw new Error(`Expected one inline script in ${file}, found ${scripts.length}`);
}
const hash = createHash('sha256').update(scripts[0][1], 'utf8').digest('base64');

const csp = [
  "default-src 'none'",
  "img-src data:",
  "style-src 'unsafe-inline'",
  `script-src 'sha256-${hash}'`,
  "base-uri 'none'",
  "form-action 'none'",
].join('; ');

const meta = `<meta http-equiv="Content-Security-Policy" content="${csp}">`;
writeFileSync(file, html.replace('<head>', `<head>\n  ${meta}`));
console.log(`CSP added to ${file}`);
