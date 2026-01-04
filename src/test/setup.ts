import '@testing-library/jest-dom';
import 'fake-indexeddb/auto';
import { beforeEach, afterEach, vi } from 'vitest';

// Mock Web Crypto API for testing
if (!globalThis.crypto) {
  const crypto = require('crypto');
  globalThis.crypto = crypto.webcrypto as Crypto;
}

// Reset mocks before each test
beforeEach(() => {
  vi.clearAllMocks();
});

// Cleanup after each test
afterEach(() => {
  vi.restoreAllMocks();
});

// Suppress console errors/warnings in tests unless explicitly needed
globalThis.console = {
  ...console,
  error: vi.fn(),
  warn: vi.fn(),
};
