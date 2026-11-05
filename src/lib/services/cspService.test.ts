import { describe, it, expect, beforeEach, vi } from 'vitest';

async function load() {
  vi.resetModules();
  return await import('./cspService');
}

function policies(): string[] {
  return Array.from(document.head.querySelectorAll('meta[http-equiv="Content-Security-Policy"]'))
    .map(meta => meta.getAttribute('content') as string);
}

describe('restrictConnections', () => {
  beforeEach(() => {
    document.head.querySelectorAll('meta').forEach(meta => meta.remove());
  });

  it('allows this origin and the sync server only', async () => {
    const { restrictConnections } = await load();
    restrictConnections('https://sync.example.com/some/path');
    expect(policies()).toEqual(["connect-src 'self' blob: https://sync.example.com"]);
  });

  it('keeps a non-default port', async () => {
    const { restrictConnections } = await load();
    restrictConnections('http://localhost:3030');
    expect(policies()).toEqual(["connect-src 'self' blob: http://localhost:3030"]);
  });

  it('reports that it is in force', async () => {
    const { restrictConnections, isConnectionRestricted } = await load();
    expect(isConnectionRestricted()).toBe(false);
    restrictConnections('https://sync.example.com');
    expect(isConnectionRestricted()).toBe(true);
  });

  it('adds one policy however often it runs', async () => {
    const { restrictConnections } = await load();
    restrictConnections('https://sync.example.com');
    restrictConnections('https://sync.example.com');
    expect(policies()).toHaveLength(1);
  });

  it('cannot widen a policy already in force', async () => {
    const { restrictConnections } = await load();
    restrictConnections('https://one.example.com');
    restrictConnections('https://two.example.com');
    expect(policies()).toEqual(["connect-src 'self' blob: https://one.example.com"]);
  });

  it.each(['', 'not a url', 'javascript:alert(1)', 'file:///etc/passwd', 'data:text/html,x'])(
    'leaves connections alone for %j, rather than cutting off sync',
    async endpoint => {
      const { restrictConnections, isConnectionRestricted } = await load();
      restrictConnections(endpoint);
      expect(policies()).toEqual([]);
      expect(isConnectionRestricted()).toBe(false);
    }
  );

  it('cannot be used to inject a directive', async () => {
    const { restrictConnections } = await load();
    restrictConnections("https://sync.example.com; script-src 'unsafe-inline'");
    for (const policy of policies()) {
      expect(policy).not.toContain('script-src');
      expect(policy.split(';')).toHaveLength(1);
    }
  });
});
