-- Phase 1 of the passkey work: store enrolled WebAuthn credentials, hold
-- in-flight challenges, and extend sessions with an MFA-verification flag
-- so a password-only login can be "staged" pending a passkey assertion.

-- Registered passkeys. One row per (user, credential). A user can have
-- multiple passkeys (one per authenticator / browser / phone) — we store
-- them all and accept any valid assertion on login.
CREATE TABLE user_credentials (
    -- credential.id from the authenticator, base64url-encoded. This is what
    -- the browser returns in allowCredentials list during authentication,
    -- so we key on it.
    id TEXT NOT NULL PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    -- webauthn-rs `Passkey` serialised to JSON. Contains the COSE public
    -- key, sign_count, attestation metadata, backup state, etc. Server-side
    -- verification uses the whole object, so store it opaquely.
    passkey_json TEXT NOT NULL,

    -- Human-friendly label set by the user on enrolment ("My iPhone",
    -- "YubiKey 5C"). Never treated as authoritative — the browser picks
    -- which credential actually gets used.
    nickname TEXT,

    created_at TEXT NOT NULL,
    last_used_at TEXT
);

CREATE INDEX idx_user_credentials_user_id ON user_credentials(user_id);

-- Short-lived server state for an in-flight WebAuthn ceremony. The browser
-- side of a register-or-authenticate call takes two round-trips; between
-- them the server needs to remember the challenge + expected parameters
-- (webauthn-rs PasskeyRegistration / PasskeyAuthentication state). We
-- store that serialised here, keyed by a UUID the client returns on the
-- completion call.
CREATE TABLE webauthn_challenges (
    id TEXT NOT NULL PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- "registration" | "authentication"
    kind TEXT NOT NULL,
    -- Serialised webauthn-rs state object.
    state_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    -- 5 minutes post-creation. Stale challenges are rejected on completion
    -- and pruned opportunistically.
    expires_at TEXT NOT NULL
);

CREATE INDEX idx_webauthn_challenges_user_id ON webauthn_challenges(user_id);
CREATE INDEX idx_webauthn_challenges_expires_at ON webauthn_challenges(expires_at);

-- Extend sessions. A session created by password-only login when the user
-- has ≥ 1 passkey enrolled is "pending" — it's a real session row, it has
-- a token, but the user-facing auth middleware rejects it for everything
-- except the passkey-authenticate endpoints. Completing the WebAuthn
-- assertion flips this to 1.
--
-- Default 1 means any session created by code paths that don't know
-- about MFA (admin login, legacy flows, tests) is treated as fully
-- verified — the login handler is the only place that must explicitly
-- opt in to the pending state.
ALTER TABLE sessions ADD COLUMN mfa_verified INTEGER NOT NULL DEFAULT 1;
