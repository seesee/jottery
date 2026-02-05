# Inbox API - Manual Testing Guide

> The Inbox API allows external tools and scripts to submit notes to a Jottery account without needing the encryption password. Submitted items are stored unencrypted on the server and appear in the user's inbox for review, where they can be accepted (encrypted into the main note corpus) or deleted.

## Prerequisites

- [ ] Jottery server running locally or accessible remotely
- [ ] Web client accessible at `http://localhost:5173` (or production URL)
- [ ] TUI client compiled and available
- [ ] A registered, approved user account with at least one device
- [ ] `curl` or similar HTTP client for API testing

## Test Environment Setup

### Local Development

```bash
# Terminal 1: Start server
cd server && cargo run

# Terminal 2: Start web client
npm run dev

# Terminal 3: Start TUI (optional)
cd tui && cargo run
```

## Test Scenarios

### Scenario 1: Generate Inbox Token (Web)

**Purpose:** Verify that a user can generate an inbox token from web settings.

**Steps:**
1. Log in to the web app
2. Open Settings (gear icon or Ctrl+,)
3. Navigate to the Sync tab
4. Locate the "Inbox API" section
5. Click "Generate Token"

**Expected Result:**
- A 64-character hex token is displayed
- A warning message says "Save this token now"
- A copy button is available
- A curl example is shown in the "Quick Start" section

**Verification:**
- [ ] Token is displayed (64 hex characters)
- [ ] Copy button works (token copied to clipboard)
- [ ] Curl example includes the correct endpoint and token placeholder
- [ ] After navigating away and back, the section shows "Token active" (not the token itself)

---

### Scenario 2: Submit Item via curl

**Purpose:** Verify that items can be submitted to the inbox via the API.

**Steps:**
1. Generate an inbox token (Scenario 1)
2. Run the following curl command:
   ```bash
   curl -X POST https://<server>/api/v1/inbox \
     -H "Authorization: Bearer <inbox-token>" \
     -H "Content-Type: application/json" \
     -d '{"content":"My first inbox note","tags":["test","inbox"]}'
   ```
3. Verify the response

**Expected Result:**
- HTTP 201 Created
- Response body contains `id` (UUID) and `createdAt` (ISO 8601 timestamp)

**Verification:**
- [ ] Response status is 201
- [ ] Response contains valid `id` and `createdAt`

---

### Scenario 3: Submit Item with Source

**Purpose:** Verify that the optional `source` field is stored correctly.

**Steps:**
1. Submit an item with a source label:
   ```bash
   curl -X POST https://<server>/api/v1/inbox \
     -H "Authorization: Bearer <inbox-token>" \
     -H "Content-Type: application/json" \
     -d '{"content":"From a webhook","tags":["automated"],"source":"github-webhook"}'
   ```

**Expected Result:**
- HTTP 201 Created
- Source field is stored and displayed in the inbox UI

**Verification:**
- [ ] Response status is 201
- [ ] Source label appears in inbox UI when viewing the item

---

### Scenario 4: Quota Enforcement (Item Count)

**Purpose:** Verify that the inbox item count quota is enforced.

**Steps:**
1. Ask an admin to set your inbox quota to 2 items (Admin Dashboard > User > Settings)
2. Submit 2 items via curl (should succeed)
3. Submit a 3rd item

**Expected Result:**
- First 2 submissions return 201
- Third submission returns HTTP 429 Too Many Requests
- Error message indicates "Inbox item limit reached"

**Verification:**
- [ ] Third submission rejected with 429
- [ ] Error message mentions quota limit

---

### Scenario 5: Quota Enforcement (Size)

**Purpose:** Verify that the inbox size quota is enforced.

**Steps:**
1. Ask an admin to set your inbox size quota to 1 MB
2. Submit an item with content close to 1 MB
3. Submit another item that would push the total over 1 MB

**Expected Result:**
- First submission succeeds (201)
- Second submission returns HTTP 413 Payload Too Large
- Error message indicates "Inbox size quota exceeded"

**Verification:**
- [ ] Second submission rejected with 413
- [ ] Error message mentions size quota

---

### Scenario 6: Sync and View Inbox (Web)

**Purpose:** Verify that inbox items appear after sync in the web client.

**Steps:**
1. Submit items via curl (Scenario 2)
2. In the web client, trigger a sync (or wait for auto-sync)
3. Look for the "Inbox (N)" indicator at the bottom of the note list

**Expected Result:**
- Inbox indicator appears showing the correct count
- Clicking the indicator opens the Inbox Panel
- Items are listed with title (first line), tags, date, and source

**Verification:**
- [ ] Inbox indicator shows correct count
- [ ] Inbox Panel opens on click
- [ ] Items display content, tags, date, and source correctly
- [ ] Content is rendered as plain text (no Markdown/HTML rendering)

---

### Scenario 7: Accept Single Item (Web)

**Purpose:** Verify that accepting an inbox item encrypts it into a note.

**Steps:**
1. Open the Inbox Panel
2. Click "Accept" on an item
3. Close the inbox

**Expected Result:**
- Item is removed from the inbox
- A new note appears in the note list with the same content and tags
- Inbox count decreases by 1
- Toast notification shows "Note accepted"

**Verification:**
- [ ] Item removed from inbox
- [ ] New note created with correct content and tags
- [ ] Note is encrypted (stored encrypted in database)

---

### Scenario 8: Delete Single Item (Web)

**Purpose:** Verify that deleting an inbox item removes it.

**Steps:**
1. Open the Inbox Panel
2. Click "Delete" on an item
3. Confirm the deletion in the confirmation modal

**Expected Result:**
- Confirmation modal appears with warning message
- After confirming, item is removed from inbox
- Item is deleted from the server
- Toast notification shows "Inbox item deleted"

**Verification:**
- [ ] Confirmation modal is shown
- [ ] Cancelling does not delete the item
- [ ] Confirming removes the item
- [ ] Item no longer appears on next sync

---

### Scenario 9: Accept All Items (Web)

**Purpose:** Verify bulk acceptance of all inbox items.

**Steps:**
1. Submit 3+ items via curl
2. Sync the web client
3. Open the Inbox Panel
4. Click "Accept All"

**Expected Result:**
- All items are accepted and become encrypted notes
- Inbox is emptied
- Inbox Panel shows empty state or closes
- Toast notification shows "All N notes accepted"

**Verification:**
- [ ] All items become notes
- [ ] Inbox is empty after operation
- [ ] All notes have correct content and tags

---

### Scenario 10: Delete All Items (Web)

**Purpose:** Verify bulk deletion with confirmation.

**Steps:**
1. Submit 3+ items via curl
2. Sync the web client
3. Open the Inbox Panel
4. Click "Delete All"
5. Confirm the deletion

**Expected Result:**
- Confirmation modal shows count of items to be deleted
- After confirming, all items are removed
- Inbox is emptied
- Toast notification shows "All inbox items deleted"

**Verification:**
- [ ] Confirmation modal shows correct count
- [ ] All items deleted after confirmation
- [ ] Items no longer appear on next sync

---

### Scenario 11: Revoke Token

**Purpose:** Verify that revoking a token prevents further submissions.

**Steps:**
1. Go to Settings > Sync > Inbox API
2. Click "Revoke Token" and confirm
3. Try to submit a new item using the old token

**Expected Result:**
- Revoke confirmation modal appears
- After confirming, token is revoked
- Subsequent submissions with the old token return 401 Unauthorized
- Existing inbox items are NOT deleted (only the token is revoked)

**Verification:**
- [ ] Token revoked successfully
- [ ] Old token returns 401
- [ ] Existing inbox items remain accessible

---

### Scenario 12: TUI Inbox Workflow

**Purpose:** Verify the TUI inbox experience.

**Steps:**
1. Submit items via curl
2. Run `jottery` (TUI)
3. Press `y` to sync
4. Look for "Inbox (N)" in the status area
5. Press `b` to open the inbox view
6. Use `j`/`k` to navigate items
7. Press `Enter` to accept the selected item
8. Press `d` to delete an item (confirm with `y`)
9. Press `Esc` to close the inbox

**Expected Result:**
- Inbox count appears after sync
- Inbox view shows items with preview
- Keybindings work as documented
- Accept creates an encrypted note
- Delete removes the item

**Verification:**
- [ ] Inbox indicator shows after sync
- [ ] `b` opens inbox view
- [ ] Navigation with j/k works
- [ ] Enter accepts item (note appears in list)
- [ ] d then y deletes item
- [ ] A accepts all items
- [ ] D then y deletes all items
- [ ] Esc returns to note list

---

### Scenario 13: TUI Inbox Token CLI

**Purpose:** Verify the TUI CLI inbox token management.

**Steps:**
1. Run `jottery inbox-token generate -e user@example.com`
2. Enter password when prompted
3. Note the generated token
4. Run `jottery inbox-token status -e user@example.com`
5. Run `jottery inbox-token revoke -e user@example.com`

**Expected Result:**
- Generate: displays token and curl example
- Status: shows "Inbox token is active"
- Revoke: confirms token revoked

**Verification:**
- [ ] Token generation works
- [ ] Status check works
- [ ] Revoke works
- [ ] Submissions fail after revocation

---

### Scenario 14: Admin Quota Management

**Purpose:** Verify admin can manage user inbox quotas.

**Steps:**
1. Log in to the admin dashboard
2. Navigate to a user's detail page
3. Edit inbox quota settings (max items, max size MB)
4. Save changes

**Expected Result:**
- Inbox quota fields are visible and editable
- Changes are saved and enforced on subsequent submissions
- Current inbox usage (items, size) is displayed

**Verification:**
- [ ] Quota fields visible in user settings
- [ ] Changes persist after saving
- [ ] New quotas are enforced immediately

---

### Scenario 15: User Portal Inbox Token

**Purpose:** Verify the user portal inbox token management.

**Steps:**
1. Log in to the user portal (/user)
2. Navigate to Account
3. Locate the "Inbox API" section
4. Generate/revoke token
5. View usage statistics

**Expected Result:**
- Inbox usage (items/max, size/max) is displayed
- Token can be generated and revoked
- Quick Start guide with curl example is available

**Verification:**
- [ ] Usage statistics are correct
- [ ] Token generation works
- [ ] Token revocation works
- [ ] Curl example is helpful

---

### Scenario 16: Invalid Token

**Purpose:** Verify proper error handling for invalid tokens.

**Steps:**
1. Submit a request with a made-up token:
   ```bash
   curl -X POST https://<server>/api/v1/inbox \
     -H "Authorization: Bearer invalid-token-here" \
     -H "Content-Type: application/json" \
     -d '{"content":"Should fail"}'
   ```

**Expected Result:**
- HTTP 401 Unauthorized

**Verification:**
- [ ] Response status is 401

---

### Scenario 17: Empty Content

**Purpose:** Verify that empty content is rejected.

**Steps:**
1. Submit a request with empty/whitespace content:
   ```bash
   curl -X POST https://<server>/api/v1/inbox \
     -H "Authorization: Bearer <token>" \
     -H "Content-Type: application/json" \
     -d '{"content":"   "}'
   ```

**Expected Result:**
- HTTP 400 Bad Request
- Error message: "Content must not be empty"

**Verification:**
- [ ] Response status is 400
- [ ] Helpful error message returned

## Platform-Specific Notes

### Web Client
- The inbox indicator appears at the bottom of the note list sidebar
- Content is always rendered as plain text for security (never Markdown or HTML)
- `showPreview` is forced to `false` when accepting items

### TUI Client
- Press `b` from the note list to open the inbox
- The inbox view is a modal overlay with list + preview panes
- Keybindings: Enter=accept, d=delete, A=accept all, D=delete all, j/k=navigate, Esc=close

## Known Issues

- Inbox items are stored unencrypted on the server (by design — they haven't been encrypted yet)
- Revoking a token does not delete existing inbox items
- No editing of inbox items — accept or delete only

## Cleanup

1. Delete any remaining test inbox items (Delete All in the inbox)
2. Revoke test inbox tokens
3. Reset admin quota settings to defaults if changed

## Test Results Log

| Date | Tester | Platform | Version | Pass/Fail | Notes |
|------|--------|----------|---------|-----------|-------|
| | | | | | |

## Related Documentation

- [Inbox API Documentation](../INBOX-API.md)
- [Sync Specification](../SYNC-SPEC.md)
- Server inbox tests: `server/tests/inbox_tests.rs`
- Web inbox tests: `src/lib/services/inboxService.test.ts`
- TUI inbox tests: `tui/tests/integration_test.rs` (inbox_tests module)
