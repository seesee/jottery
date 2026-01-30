/**
 * Typed error classes for consistent error handling across the application.
 *
 * Error categories:
 * - ApplicationLockedError: User needs to unlock the app
 * - NotFoundError: Resource (note, attachment, etc.) not found
 * - DatabaseError: IndexedDB or storage issues
 * - AuthenticationError: Password/credentials issues
 * - CryptoError: Encryption/decryption failures
 * - ValidationError: Input validation failures
 * - SyncError: Synchronisation failures
 * - InitialisationError: App setup/init issues
 */

/**
 * Base class for all application errors.
 * Provides consistent error structure and optional cause chaining.
 */
export class AppError extends Error {
  /** Original error that caused this error */
  readonly cause?: Error;

  constructor(message: string, cause?: Error) {
    super(message);
    this.name = 'AppError';
    this.cause = cause;

    // Maintain proper stack trace in V8 environments
    if (Error.captureStackTrace) {
      Error.captureStackTrace(this, this.constructor);
    }
  }
}

/**
 * Thrown when an operation requires the app to be unlocked.
 * UI should prompt user to unlock.
 */
export class ApplicationLockedError extends AppError {
  constructor(message = 'Application is locked') {
    super(message);
    this.name = 'ApplicationLockedError';
  }
}

/**
 * Thrown when a requested resource cannot be found.
 */
export class NotFoundError extends AppError {
  /** Type of resource that wasn't found */
  readonly resourceType: string;
  /** ID of the resource that wasn't found */
  readonly resourceId?: string;

  constructor(resourceType: string, resourceId?: string) {
    const message = resourceId
      ? `${resourceType} '${resourceId}' not found`
      : `${resourceType} not found`;
    super(message);
    this.name = 'NotFoundError';
    this.resourceType = resourceType;
    this.resourceId = resourceId;
  }
}

/**
 * Thrown when database operations fail.
 */
export class DatabaseError extends AppError {
  /** The operation that failed */
  readonly operation: string;

  constructor(operation: string, message: string, cause?: Error) {
    super(`Database ${operation} failed: ${message}`, cause);
    this.name = 'DatabaseError';
    this.operation = operation;
  }
}

/**
 * Thrown when authentication fails (wrong password, etc.).
 */
export class AuthenticationError extends AppError {
  constructor(message = 'Authentication failed') {
    super(message);
    this.name = 'AuthenticationError';
  }
}

/**
 * Thrown when encryption or decryption fails.
 */
export class CryptoError extends AppError {
  /** The crypto operation that failed */
  readonly operation: 'encrypt' | 'decrypt' | 'derive' | 'hash';

  constructor(operation: 'encrypt' | 'decrypt' | 'derive' | 'hash', message: string, cause?: Error) {
    super(`${operation} failed: ${message}`, cause);
    this.name = 'CryptoError';
    this.operation = operation;
  }
}

/**
 * Thrown when input validation fails.
 */
export class ValidationError extends AppError {
  /** Field that failed validation */
  readonly field?: string;

  constructor(message: string, field?: string) {
    super(message);
    this.name = 'ValidationError';
    this.field = field;
  }
}

/**
 * Thrown when synchronisation fails.
 */
export class SyncError extends AppError {
  /** HTTP status code if applicable */
  readonly statusCode?: number;
  /** Whether the error is recoverable */
  readonly recoverable: boolean;

  constructor(message: string, options?: { statusCode?: number; recoverable?: boolean; cause?: Error }) {
    super(message, options?.cause);
    this.name = 'SyncError';
    this.statusCode = options?.statusCode;
    this.recoverable = options?.recoverable ?? true;
  }
}

/**
 * Thrown when app initialisation fails.
 */
export class InitialisationError extends AppError {
  constructor(message: string, cause?: Error) {
    super(message, cause);
    this.name = 'InitialisationError';
  }
}

/**
 * Type guard to check if an error is an AppError.
 */
export function isAppError(error: unknown): error is AppError {
  return error instanceof AppError;
}

/**
 * Type guard for ApplicationLockedError.
 */
export function isApplicationLockedError(error: unknown): error is ApplicationLockedError {
  return error instanceof ApplicationLockedError;
}

/**
 * Type guard for NotFoundError.
 */
export function isNotFoundError(error: unknown): error is NotFoundError {
  return error instanceof NotFoundError;
}

/**
 * Type guard for AuthenticationError.
 */
export function isAuthenticationError(error: unknown): error is AuthenticationError {
  return error instanceof AuthenticationError;
}

/**
 * Type guard for SyncError.
 */
export function isSyncError(error: unknown): error is SyncError {
  return error instanceof SyncError;
}

/**
 * Safely extract error message from unknown error type.
 */
export function getErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === 'string') {
    return error;
  }
  return 'An unknown error occurred';
}
