/**
 * Sync HTTP Client
 *
 * Handles all HTTP communication with the sync server.
 * Extracted from syncService for better testability and separation of concerns.
 */

import type {
  SyncPushRequest,
  SyncPushResponse,
  SyncPullRequest,
  SyncPullResponse,
  SyncStatusResponse,
  AuthRegisterRequest,
  AuthRegisterResponse,
} from '../types';
import { toast } from '../utils/toast.svelte';

const API_VERSION = 'v1';
const DEFAULT_TIMEOUT_MS = 30000; // 30 second timeout for fetch requests

/**
 * Create a fetch request with timeout
 */
function fetchWithTimeout(url: string, options: RequestInit, timeoutMs = DEFAULT_TIMEOUT_MS): Promise<Response> {
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(new Error(`Request timed out after ${timeoutMs}ms`)), timeoutMs);

  return fetch(url, {
    ...options,
    signal: controller.signal,
  }).finally(() => clearTimeout(timeoutId));
}

/**
 * Normalize endpoint URL by removing trailing slash
 */
export function normalizeEndpoint(endpoint: string): string {
  return endpoint.endsWith('/') ? endpoint.slice(0, -1) : endpoint;
}

/**
 * Custom error class for sync API errors
 */
export class SyncApiError extends Error {
  constructor(
    message: string,
    public readonly status: number,
    public readonly responseText: string,
    /** From Retry-After, which a 429 and a 503 can carry */
    public readonly retryAfterSeconds?: number
  ) {
    super(message);
    this.name = 'SyncApiError';
  }
}

/** Bounded, because a server shouldn't be able to stop a client syncing for a year */
const MAX_RETRY_AFTER_SECONDS = 3600;

function retryAfterSeconds(response: Response): number | undefined {
  const seconds = Number(response.headers.get('Retry-After'));
  return Number.isFinite(seconds) && seconds > 0 ? Math.min(seconds, MAX_RETRY_AFTER_SECONDS) : undefined;
}

/**
 * Handle HTTP error responses with specific error messages
 */
function handleHttpError(response: Response, errorText: string, operation: string): never {
  if (response.status === 413) {
    // A refused push is split and retried, so the caller reports the outcome once
    if (operation === 'push') {
      throw new SyncApiError('Upload too large', response.status, errorText);
    }
    const errorMessage = 'Server response too large. Please contact your administrator.';
    toast.error(errorMessage);
    throw new SyncApiError(errorMessage, response.status, errorText);
  }

  throw new SyncApiError(
    `${operation} failed: ${response.statusText} - ${errorText}`,
    response.status,
    errorText,
    retryAfterSeconds(response)
  );
}

/**
 * Delete an inbox item from the server. One that's already gone counts as deleted.
 */
export async function deleteInboxItem(endpoint: string, apiKey: string, itemId: string): Promise<void> {
  const response = await fetchWithTimeout(`${endpoint}/api/${API_VERSION}/inbox/${itemId}`, {
    method: 'DELETE',
    headers: { 'Authorization': `Bearer ${apiKey}` },
  });
  if (!response.ok && response.status !== 404) {
    throw new Error(`Failed to delete inbox item: ${response.statusText}`);
  }
}

/**
 * Register a new device with the sync server
 */
export async function registerDevice(
  endpoint: string,
  deviceName: string
): Promise<AuthRegisterResponse> {
  endpoint = normalizeEndpoint(endpoint);

  const request: AuthRegisterRequest = {
    deviceName,
    deviceType: 'web',
  };

  const response = await fetchWithTimeout(`${endpoint}/api/${API_VERSION}/auth/register`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(request),
  });

  if (!response.ok) {
    const errorText = await response.text();
    throw new SyncApiError(
      `Registration failed: ${response.statusText} - ${errorText}`,
      response.status,
      errorText
    );
  }

  return await response.json();
}

/**
 * Push notes to the sync server
 */
export async function pushToServer(
  endpoint: string,
  apiKey: string,
  request: SyncPushRequest
): Promise<SyncPushResponse> {
  endpoint = normalizeEndpoint(endpoint);

  const url = `${endpoint}/api/${API_VERSION}/sync/push`;
  const response = await fetchWithTimeout(url, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${apiKey}`,
    },
    body: JSON.stringify(request),
  });

  if (!response.ok) {
    const errorText = await response.text();
    handleHttpError(response, errorText, 'push');
  }

  return await response.json();
}

/**
 * Pull notes from the sync server
 */
export async function pullFromServer(
  endpoint: string,
  apiKey: string,
  request: SyncPullRequest
): Promise<SyncPullResponse> {
  endpoint = normalizeEndpoint(endpoint);

  const url = `${endpoint}/api/${API_VERSION}/sync/pull`;
  const response = await fetchWithTimeout(url, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${apiKey}`,
    },
    body: JSON.stringify(request),
  });

  if (!response.ok) {
    const errorText = await response.text();
    handleHttpError(response, errorText, 'pull');
  }

  return await response.json();
}

/**
 * Revoke the calling device, so its API key stops working
 */
export async function revokeDevice(endpoint: string, apiKey: string): Promise<void> {
  endpoint = normalizeEndpoint(endpoint);

  const response = await fetchWithTimeout(`${endpoint}/api/${API_VERSION}/sync/device`, {
    method: 'DELETE',
    headers: {
      'Authorization': `Bearer ${apiKey}`,
    },
  }, 10000);

  if (!response.ok) {
    handleHttpError(response, await response.text(), 'revoke device');
  }
}

/**
 * Get sync status from the server
 */
export async function getServerStatus(
  endpoint: string,
  apiKey: string
): Promise<SyncStatusResponse> {
  endpoint = normalizeEndpoint(endpoint);

  const url = `${endpoint}/api/${API_VERSION}/sync/status`;
  const response = await fetchWithTimeout(url, {
    method: 'GET',
    headers: {
      'Authorization': `Bearer ${apiKey}`,
    },
  }, 10000); // 10 second timeout for status check

  if (!response.ok) {
    const errorText = await response.text();
    throw new SyncApiError(
      `Status check failed: ${response.statusText} - ${errorText}`,
      response.status,
      errorText
    );
  }

  return await response.json();
}

/**
 * Fetch a single attachment blob from the server (vault health repair).
 * Returns the attachment id and its base64-encoded data.
 */
export async function fetchAttachment(
  endpoint: string,
  apiKey: string,
  attachmentId: string
): Promise<{ id: string; data: string }> {
  endpoint = normalizeEndpoint(endpoint);

  const url = `${endpoint}/api/${API_VERSION}/sync/attachments/${encodeURIComponent(attachmentId)}`;
  const response = await fetchWithTimeout(url, {
    method: 'GET',
    headers: {
      'Authorization': `Bearer ${apiKey}`,
    },
  }, 30000); // Attachments can be large; allow 30 seconds

  if (!response.ok) {
    const errorText = await response.text();
    throw new SyncApiError(
      `Attachment fetch failed: ${response.statusText} - ${errorText}`,
      response.status,
      errorText
    );
  }

  return await response.json();
}
