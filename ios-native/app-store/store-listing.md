# App Store Listing — Jottery for iOS

Copy-paste source for the App Store Connect product page. Keep this in lockstep
with what the app actually does — review rejects listings that promise features
the binary doesn't ship (Guideline 2.3.2). Companion docs: `review-notes.md`
(App Review Information) and `release-notes.md` (What's New).

## Basic information

- **App Name:** Jottery
- **Subtitle** (≤30 chars): `Private, encrypted notes` (24)
- **Category:** Productivity (secondary: Utilities)
- **Price:** Free — no in-app purchases, no ads
- **Content Rating:** 4+ (All Ages)
- **Support URL:** https://github.com/seesee/jottery/issues
- **Marketing URL:** https://jottery.org
- **Privacy Policy URL:** https://jottery.org/privacy
- **Copyright:** © 2026 Chris Carline
- **Locale:** en-GB (only localisation; serves all storefronts)

## Promotional text (≤170 chars, editable without review)

```text
An encrypted notepad that works entirely offline. No account, no ads, no
tracking — and optional sync to a server you run yourself.
```
(148 chars)

## Description

```text
Jottery is a private, encrypted notepad. Your notes are stored on your device
and encrypted with a password only you know. There is no account to create, no
advertising, no analytics, and nothing is ever sent anywhere unless you choose
to sync — and even then, only to a server you run yourself.

COMPLETELY FREE
Every feature is included. No subscriptions, no in-app purchases, no ads.

PRIVATE BY DESIGN
• AES-256 encryption with a password that never leaves your device
• Auto-lock with Face ID or Touch ID unlock
• No tracking of any kind — the privacy label is "Data Not Collected"
• Open source (MIT licence), so you can verify all of it

A PROPER PLACE TO THINK
• Markdown editing with live syntax highlighting
• Code snippets with highlighting for popular programming languages
• Calc notes: write sums as plain text and see results as you type
• Outline notes for structured lists and planning
• Attach photos and files to any note

FIND ANYTHING
• Search by words, #tags, dates, or word count — combined however you like
• Pin what matters, archive what's done, lock notes against edits
• Recycle bin and version history, so nothing is lost by accident

SEND THINGS IN
• Share text, links, and images into Jottery from any app
• Import and export your notes as JSON — your data is yours

SYNC, ON YOUR TERMS (OPTIONAL)
Jottery syncs between devices through a server you host yourself — the server
is open source and ships as a Docker image. Notes are encrypted on the device
before upload, so the server only ever stores ciphertext. No Jottery servers,
no third-party cloud. Web and terminal clients are included in the same
project, so your notes are wherever you work.

One honest caveat: your password is the only key. If you lose it, nobody —
including us — can recover your notes. That is the point.
```

## Keywords (≤100 chars, comma-separated; avoid words already in name/subtitle)

```text
notepad,secure,markdown,offline,journal,scratchpad,self-hosted,sync,code,vault,writing,privacy
```
(95 chars — "notes", "private", "encrypted" omitted: already indexed via name/subtitle)

## App Privacy questionnaire

- **Data Not Collected** — accurate: no analytics, crash reporting, or
  telemetry of any kind. Sync accounts exist only on servers users run
  themselves; we operate no service and collect nothing.

## Export compliance

- Uses only Apple CryptoKit / CommonCrypto standard algorithms
  (`ITSAppUsesNonExemptEncryption = false` already in Info.plist), so answer
  "standard encryption, exempt" in the submission questions.

## Screenshots

Generated assets (regenerate with `./demo-generation/ios/generate.sh`):

- iPhone 6.9": `demo-generation/screenshots/appstore/iphone-69/` (7 frames, 1320×2868)
- iPad 13": `demo-generation/screenshots/appstore/ipad-13/` (8 frames, 2064×2752)

Upload order matches the numeric prefixes (01-list first — it is the hero
frame). Captions live in `demo-generation/ios/screens.json`.

## Submission checklist

- [ ] Screenshots regenerated on the release build's UI and eyeballed
- [ ] Description / subtitle / keywords pasted from this file
- [ ] Privacy Policy URL + Support URL set in App Store Connect
- [ ] App Privacy questionnaire answered as "Data Not Collected"
- [ ] Review account created on jottery.org and credentials entered in App
      Review Information (see `review-notes.md` — never commit credentials)
- [ ] Review notes pasted from `review-notes.md`
- [ ] Export compliance answered (standard/exempt)
- [ ] What's New: not shown for the first submission; for updates, paste from
      `release-notes.md`
