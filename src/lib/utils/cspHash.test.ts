import { describe, it, expect } from 'vitest';
import { createHash } from 'node:crypto';
import { addInlineScriptHashes, inlineScriptHashes } from './cspHash';

function sha256(text: string): string {
  return `sha256-${createHash('sha256').update(text, 'utf8').digest('base64')}`;
}

const META = `<meta http-equiv="Content-Security-Policy" content="default-src 'self'; script-src 'self'; object-src 'none';">`;

function page(body: string): string {
  return `<!doctype html><html><head>${META}<title>x</title>${body}</head><body></body></html>`;
}

function policyOf(html: string): string {
  return (html.match(/content="([^"]*)"/) as RegExpMatchArray)[1];
}

describe('inlineScriptHashes', () => {
  it('hashes the exact text between the tags', () => {
    expect(inlineScriptHashes(page('<script type="module" crossorigin>let a = 1;</script>'))).toEqual([sha256('let a = 1;')]);
  });

  it('ignores scripts that load from a file', () => {
    expect(inlineScriptHashes(page('<script type="module" src="/a.js"></script>'))).toEqual([]);
  });

  it('hashes what the browser hashes, which has line endings normalised', () => {
    expect(inlineScriptHashes(page('<script>a;\r\nb;\rc;</script>'))).toEqual([sha256('a;\nb;\nc;')]);
  });

  it('handles non-ASCII as UTF-8', () => {
    expect(inlineScriptHashes(page('<script>"Jóttery 日本語";</script>'))).toEqual([sha256('"Jóttery 日本語";')]);
  });

  it('handles every inline script', () => {
    expect(inlineScriptHashes(page('<script>a</script><script>b</script>'))).toHaveLength(2);
  });
});

describe('addInlineScriptHashes', () => {
  it('adds the hash to script-src and nothing else', () => {
    const script = '<script>let a = 1;</script>';
    const policy = policyOf(addInlineScriptHashes(page(script)));
    expect(policy).toBe(`default-src 'self'; script-src 'self' '${sha256('let a = 1;')}'; object-src 'none';`);
  });

  it('leaves the script itself unchanged, so the hash still matches', () => {
    const script = '<script>let a = "$& $1";</script>';
    expect(addInlineScriptHashes(page(script))).toContain(script);
  });

  it('never allows unsafe-inline', () => {
    expect(policyOf(addInlineScriptHashes(page('<script>a</script>')))).not.toContain('unsafe-inline');
  });

  it('fails the build if the policy is missing', () => {
    expect(() => addInlineScriptHashes('<html><head><script>a</script></head></html>')).toThrow(/policy/i);
  });

  it('fails the build if the policy has no script-src', () => {
    const html = page('<script>a</script>').replace("script-src 'self'; ", '');
    expect(() => addInlineScriptHashes(html)).toThrow(/script-src/);
  });

  it('fails the build if there is nothing to hash, because the app would not start', () => {
    expect(() => addInlineScriptHashes(page(''))).toThrow(/inline script/i);
  });
});
