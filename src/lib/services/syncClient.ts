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
import { getNativePlatform } from '../utils/device';

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
    public readonly responseText: string
  ) {
    super(message);
    this.name = 'SyncApiError';
  }
}

/**
 * Handle HTTP error responses with specific error messages
 */
function handleHttpError(response: Response, errorText: string, operation: string): never {
  if (response.status === 413) {
    const errorMessage = operation === 'push'
      ? 'Upload too large. Please reduce attachment sizes or sync fewer notes at once. Maximum size: 5MB'
      : 'Server response too large. Please contact your administrator.';
    toast.error(errorMessage);
    throw new SyncApiError(errorMessage, response.status, errorText);
  }

  throw new SyncApiError(
    `${operation} failed: ${response.statusText} - ${errorText}`,
    response.status,
    errorText
  );
}

/**
 * Register a new device with the sync server
 */
export async function registerDevice(
  endpoint: string,
  deviceName: string
): Promise<AuthRegisterResponse> {
  endpoint = normalizeEndpoint(endpoint);

  const platform = getNativePlatform();
  const request: AuthRegisterRequest = {
    deviceName,
    deviceType: platform === 'web' ? 'web' : platform,
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
