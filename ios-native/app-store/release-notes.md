# App Store release notes — Jottery for iOS

Version-controlled copy for App Store Connect's **What's New in This Version**
field, so wording survives between submissions instead of being retyped.

## How the field behaves

- **First submission:** the field is not shown to users. App Store Connect may
  still accept text; it appears only once there is a previous version to compare
  against. Put the selling points in the app *description*, not here.
- **Updates:** shown on the product page and in the Updates tab. Apple rejects
  placeholder text such as "bug fixes and performance improvements" on its own
  (Guideline 2.3.12) — say what actually changed.

## Style

- Lead with what a user notices, not the internal cause.
- British English, matching the app (`en-GB` is the only localisation).
- No version number in the body; App Store Connect shows it already.
- Skip anything invisible to users — refactors, test coverage, CI.

---

## Unreleased

_Nothing pending._

## 1.2.0 — 2026-07-25

```text
• You can now delete your sync account from the app, under Settings → Sync.
• Disconnecting sync now unlinks the device from your server, instead of only
  clearing it locally.
• Fixed a crash when a mistyped sync server address was entered during setup.
• Sync server addresses are now checked as you enter them, with a clear
  explanation when one cannot be used.
• Syncing over an unencrypted connection to a server on the internet is no
  longer permitted. Servers on your own network are unaffected.
• Added Privacy Policy and Support links, and the correct app version, to
  Settings.
```

### Note on the HTTPS change

The fifth bullet is a genuine behaviour change, not a fix: anyone syncing to a
public hostname over plain `http://` must switch that server to `https://`.
Servers reached by IP address, single-word hostname, or `.local` are unaffected,
which covers the self-hosted-on-a-LAN case. It is called out explicitly so the
change is not a surprise, and the app now explains the problem in-app when an
older stored address stops working.

Deliberately omitted, being invisible to users: the release-logging removal, the
privacy manifests, the renamed Settings section, the retired English (US) option
(untranslated and identical in practice), and the internal endpoint-validation
refactor.

## Checklist per submission

- [ ] Bump `MARKETING_VERSION` and `CURRENT_PROJECT_VERSION`.
- [ ] Move the "Unreleased" block to a new dated section below, and start a
      fresh "Unreleased".
- [ ] Confirm every bullet is user-visible in the build being submitted.
- [ ] Paste into App Store Connect → the version → **What's New in This
      Version**.

## Released

Nothing has shipped to the App Store yet. 1.2.0 above is the pending first
submission — the "What's New" field is not shown to users on a first release,
so that text becomes relevant from the next update onward.
