# App Review Notes — Jottery for iOS

Paste the **App Review Information → Notes** section below into App Store Connect,
substituting the placeholders. Everything else in this file is context for us, not
for Apple.

> **Credentials are never committed here.** `seesee/jottery` is a public
> repository. Create the review account separately and type the real values
> straight into App Store Connect, whose App Review Information fields are
> private to Apple.

---

## Notes to paste into App Store Connect

```text
WHAT JOTTERY IS

Jottery is an offline-first encrypted notepad. Notes are stored on the device
and encrypted with a password only the user knows (AES-256-GCM). There is no
analytics, no tracking, no advertising, and no in-app purchase.

NO ACCOUNT IS REQUIRED

The app is fully functional with no account and no network connection. On first
launch, choose "New Vault", set a password, and use every feature: create and
edit notes, syntax highlighting, tags, search, attachments, pinning, archive,
recycle bin, version history, import/export.

Please note the on-screen warning: the vault password encrypts the notes and is
not recoverable. Whatever password you set at this step is needed to unlock the
app again in the same session.

SYNC IS OPTIONAL AND SELF-HOSTED

The second tab, "Connect to Server", is entirely optional. Jottery syncs to a
server the user runs themselves — the server is open source (MIT) and published
as a Docker image. The app never talks to any service operated by us unless the
user supplies their own server address.

The app does not create accounts. It only registers a device against an account
that already exists on the user's own server, so there is no sign-up flow to
review.

A test account on our public instance is provided below if you would like to
exercise sync. It is not required to evaluate the app.

    Server URL:  https://jottery.org
    Email:       <REVIEW_ACCOUNT_EMAIL>
    Password:    <REVIEW_ACCOUNT_PASSWORD>

TO TEST SYNC

1. Launch the app and select "Connect to Server".
2. Method: "Register".
3. Server URL:      https://jottery.org
   Email:           <REVIEW_ACCOUNT_EMAIL>
   Server Password: <REVIEW_ACCOUNT_PASSWORD>
   Device Name:     anything, e.g. "Review"
4. Tap "Register Device".
5. At "Encryption Password", enter <REVIEW_ACCOUNT_PASSWORD> again. This account
   deliberately uses the same value for both. The two fields are separate by
   design: the first authenticates to the server, the second decrypts the notes.
   The server never receives the encryption password and cannot read note
   content.
6. Tap "Unlock & Sync". Notes will download and appear in the list.

DISCONNECTING AND ACCOUNT DELETION

Settings → Sync → "Disconnect" unlinks this device from the server — it revokes
the device's credential server-side and then clears it locally, so the device no
longer appears as active on the account.

Settings → Sync → "Delete Account on Server" deletes the account itself. It asks
for the account email and password, because deleting an account is irreversible
and should not be possible from a device credential alone. Two options are
offered: deactivate (account disabled, re-registration needs administrator
approval) or delete permanently. Notes already on the device are kept — removing
those is Settings → Reset → "Wipe All Data".

ENCRYPTION

All note content, tags and attachments are encrypted on the device before being
sent anywhere. The server stores ciphertext only, and cannot decrypt it. The app
uses Apple's own cryptography APIs — CryptoKit and CommonCrypto — with
AES-256-GCM and PBKDF2-HMAC-SHA256. No cryptography is implemented from scratch.

PRIVACY

The privacy manifest declares no tracking and no collected data. The only
required-reason API used is UserDefaults (reason CA92.1), for a single editor
font-size preference. Privacy policy: https://jottery.org/privacy

PERMISSIONS

- Face ID / Touch ID (optional): unlocking the app without retyping the vault
  password. The app works normally if declined.
- Local network (optional): only requested if the user points the app at a sync
  server on their own network. Not needed for the test account above, which is
  a public HTTPS host.
```

---

## Why these points are here

Each paragraph answers a specific rejection risk found during the pre-submission
audit (epic `jottery-hp3d`):

| Section | Guideline | Risk it defuses |
| --- | --- | --- |
| No account required | 2.1 | Reviewer cannot get past a login wall and marks the app incomplete. |
| Sync is optional / self-hosted | 2.1 | Reviewer has no server, concludes the headline feature is broken. |
| Disconnect / account deletion | 5.1.1(v) | Reviewer sees an email/password form and asks where account deletion is. |
| Encryption | 2.1 / export | Explains the `ITSAppUsesNonExemptEncryption=false` declaration. |
| Privacy | 5.1.1, 5.1.2 | Matches the privacy manifest and the App Privacy answers. |
| Permissions | 2.3, 5.1.1 | Pre-empts "why does a notepad want my local network?" |

## Before submitting — checklist

- [ ] Create the review account on `https://jottery.org` and confirm it is
      approved and active (the server has an admin approval workflow; an
      unapproved account cannot register a device).
- [ ] Set the account's server password and encryption password to the **same**
      value, as the notes above state.
- [ ] Seed the account with a handful of notes so the reviewer sees content
      arrive rather than an empty list.
- [ ] Walk the flow once on a clean install to confirm the numbered steps still
      match the UI.
- [ ] Substitute `<REVIEW_ACCOUNT_EMAIL>` and `<REVIEW_ACCOUNT_PASSWORD>` when
      pasting into App Store Connect.
- [ ] Keep the account alive for the life of the submission, including any
      re-review after an update.

## Pre-submission blockers

Two things must be true before this text is accurate:

1. **`https://jottery.org/privacy` must serve the actual policy.** As of the last
   check it returned the web app shell (4.8 MB, `<title>Jottery</title>`, no
   occurrence of "Privacy Policy") — `dist/privacy.html` is not deployed, and
   `/privacy.html` returns the SPA too, so this needs a rebuild and redeploy of
   the web assets rather than just a Caddy reload. Verify by content, not status
   code:

   ```
   curl -s https://jottery.org/privacy | grep -c "Privacy Policy"   # must be > 0
   ```

   The in-app Settings → About → Privacy Policy link points here, so a reviewer
   following it currently lands on the web app. This is the single most likely
   cause of a rejection under 5.1.1(i).

2. **The server must be running a build that includes `DELETE /api/v1/sync/device`
   and the account-deletion flow.** Older builds return 404/405; the app handles
   that gracefully and tells the user the device is still registered, but the
   review notes above would then overstate what happens.

## Export compliance — decide before uploading

`Info.plist` sets `ITSAppUsesNonExemptEncryption = false`. Confirm which
exemption that rests on before the first upload; an incorrect declaration is
caught at upload rather than in review.

The case for exemption: every cryptographic primitive comes from Apple —
CryptoKit and CommonCrypto — and nothing is implemented from scratch. Apple's
questionnaire treats use of the platform's own cryptography for an app's own
data as exempt.

The case against: the app does not merely rely on OS-level encryption such as
HTTPS or Data Protection; it composes those primitives into its own envelope
encryption scheme protecting user content. That is further than the narrowest
reading of "encryption limited to the operating system".

Walk Apple's "Complying with Encryption Export Regulations" questionnaire and
record the answer here. If it comes out non-exempt, the fallback is mass-market
self-classification — a one-off ERN plus an annual report — not a blocker, but
it needs doing before submission rather than after a rejected upload.
