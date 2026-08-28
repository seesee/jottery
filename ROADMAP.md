# Roadmap

This is the narrative / "why these exist" view of the jottery work backlog.
The **authoritative** source of task state is beads — `bd ready` shows what
can be picked up now, `bd list --status=open` shows everything. This file
is the map, not the inventory.

Updated: 2026-04-24.

---

## In flight

- **Phase 0 — auth hardening** — PR #64. Three audit findings: password
  change now revokes stale sessions/API keys, the status endpoint is
  no longer an enumeration oracle, rate limiting is on.
- **Phase 1 — passkey second factor** — PR #65. Users can enrol passkeys
  and use them as a second factor on user-portal login. Password stays
  primary. `WEBAUTHN_RP_*` env vars opt deployments in.

---

## Next: Phase 2 — PRF-based E2EE with passkeys ([jottery-k1el](#))

The biggest leverage point on the roadmap. Today's master-key wrapping is
still password-derived — phase 1 added passkeys as a factor but didn't
change the crypto story. With the WebAuthn **PRF extension**:

```
prf_secret  = authenticator.PRF(credential_i, salt_i)   // 32 bytes
K_wrap_i    = HKDF(prf_secret, 'jottery-wrap')
blob_i      = AES-256-GCM(K_master, K_wrap_i)           // stored server-side
```

Each enrolled passkey gets its own wrapped blob. Login = one biometric /
PIN prompt that both authenticates the user **and** yields `K_wrap` to
unwrap the master key locally. No password needed after migration, no
password-derived blob on the server to attack offline.

Sub-tasks (all wired behind `jottery-xc4t` because recovery choice shapes
everything downstream):

| Beads | Step |
|---|---|
| jottery-xc4t | Decide recovery scheme (2-passkey minimum / recovery code / mnemonic) |
| jottery-rsyw | Per-credential wrapped-key schema migration |
| jottery-ke97 | Server endpoints to store/retrieve per-credential blobs |
| jottery-rlxt | Client: PRF extension wiring + HKDF → `K_wrap` derivation |
| jottery-vlw2 | Passwordless-mode toggle (P3, requires 2+ passkeys + recovery) |

**Gating question:** the recovery scheme isn't a technical decision so
much as a UX one — how much friction at onboarding do we force to
prevent catastrophic lockouts? Whatever we pick, the onboarding copy
has to honestly communicate "lose all this and your data is gone".

---

## After Phase 2: Platforms

Independent of each other; both depend on phase 2's client contract
being stable.

### Phase 3 — native iOS + Android passkey bridges ([jottery-q70k](#))

Capacitor webviews have unreliable WebAuthn — associated-domain binding
is finicky, PRF support is incomplete. Native bridges via platform APIs
(iOS `AuthenticationServices`, Android `CredentialManager`) are the
robust path. Sub-tasks will be broken out once phase 2's client API is
settled.

### Phase 4 — TUI device-code flow ([jottery-jxpw](#))

Terminals have no authenticator. Use OAuth 2.0 Device Authorization
Grant (RFC 8628) for onboarding, with an ephemeral ECDH channel to
transfer `K_master` from an already-unlocked client. Server never sees
the master key in the clear at any point. Mirrors Signal Desktop /
Bitwarden CLI onboarding.

---

## Parallel security work (unblocked)

Ready to pick up any time; don't gate phase 2.

| Beads | Priority | Summary |
|---|---|---|
| jottery-axy0 | P2 | Persistent admin audit log (who did what, when) |
| jottery-bn3q | P2 | Regenerate session token on privilege escalation |
| jottery-u05c | P3 | Reduce default `SESSION_EXPIRY_DAYS` from 7 to 1 |
| jottery-4ujn | P2 | Admin reset_user_password should also clear passkeys |
| jottery-csqk | P3 | Full WebAuthn register/authenticate round-trip integration test |

---

## Tech debt / hygiene

| Beads | Priority | Summary |
|---|---|---|
| jottery-urzl | P3 | Relax `@playwright/test` pin when 1.59 regression is fixed upstream |
| jottery-j00t | P3 | `test.slow()` or split the two pre-existing flaky conflict-resolution tests |
| jottery-tdbg | P3 | Sweep remaining `waitForTimeout` hard sleeps in e2e specs |
| jottery-hmf1 | P3 | Consolidate duplicated `AppState` between `server/src/lib.rs` and `server/src/main.rs` |
| jottery-0ej5 | P3 | Evaluate `uuid@14` upgrade (GHSA advisory; breaking change) |

---

## Known limitations (intentionally deferred)

These came out of the phase 0 security audit but aren't planned. If the
threat model changes, reconsider.

- **Email verification on registration.** Admin-approval workflow is the
  gate today; adding email verification doesn't change who can create an
  account, just adds friction.
- **CAPTCHA / bot protection beyond rate limiting.** Rate limiting
  covers the abuse cases we can realistically expect for a self-hosted
  tool. Revisit if we ever see actual registration bots in the wild.
- **Device fingerprinting for session-theft detection.** Genuinely
  hard to do without false positives on NAT/VPN users; the session-per-
  device model already limits blast radius.
- **No key recovery for lost encryption password.** By design — E2EE
  without a server-side recoverable key. Phase 2's recovery-code /
  mnemonic discussion is a choice about what to add, not about whether
  to compromise this.

---

## How this file stays accurate

- Every item here has a beads issue with the current state. `bd show
  jottery-xxxx` for details.
- When a phase completes, update "In flight" and "Next" in this file.
- When a new sub-task is created, link it from the relevant phase table.
- When an issue is closed, the reader can tell from `bd show`;
  they don't need a redundant status column here.
