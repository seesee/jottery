# Sync Conflict Resolution - Manual Testing Guide

This guide covers testing the sync conflict detection and resolution feature. A conflict occurs when the same note is modified on multiple devices before syncing, and the server rejects a push because it has a newer version.

## Prerequisites

- [ ] Jottery server running and accessible
- [ ] Two sync-enabled clients (any combination of Web/TUI)
- [ ] Both clients registered to the same user account
- [ ] Test password known for unlocking clients

## Understanding Conflicts

### What Triggers a Conflict?

1. Device A and Device B both have note X (synced)
2. Device A modifies note X offline (or before syncing)
3. Device B modifies note X and syncs successfully
4. Device A tries to sync - server rejects because Device B's version is newer
5. Device A now has a conflict to resolve

### Conflict Resolution Options

| Option | Behaviour |
|--------|-----------|
| **Keep Mine** | Discards server version, pushes local version on next sync |
| **Keep Server** | Replaces local content with server version |
| **Keep Both** | Creates a copy of server version with `conflict-copy` tag |

## Test Environment Setup

### Option A: Two Browser Windows (Easiest)

```bash
# Terminal 1: Start server
cd server && cargo run

# Terminal 2: Start web client
npm run dev
```

1. Open `http://localhost:5173` in Chrome
2. Open `http://localhost:5173` in Firefox (or Chrome Incognito)
3. Set up the same password in both
4. Configure sync in Settings with the same server endpoint
5. Register both as devices under the same user account

### Option B: Web + TUI

```bash
# Terminal 1: Start server
cd server && cargo run

# Terminal 2: Start web client
npm run dev

# Terminal 3: Start TUI
cd tui && cargo run
```

### Option C: Two TUI Instances

Run two TUI instances with different database paths:

```bash
# Terminal 1
JOTTERY_DB_PATH=~/.jottery-test1.db cargo run

# Terminal 2
JOTTERY_DB_PATH=~/.jottery-test2.db cargo run
```

---

## Test Scenarios

### Scenario 1: Triggering a Basic Conflict

**Purpose:** Create a sync conflict and verify it's detected.

**Steps:**

1. **On Device A (e.g., Chrome):**
   - Create a new note: "Original content from Device A"
   - Add a tag: `test`
   - Wait for auto-save (3 seconds)
   - Sync to server (Settings → Sync, or press `y` in TUI)
   - Verify sync succeeds

2. **On Device B (e.g., Firefox):**
   - Sync to pull the note
   - Verify note appears with "Original content from Device A"
   - **Go offline** or disable sync temporarily
   - Edit the note: "Modified content from Device B"
   - Wait for auto-save

3. **On Device A:**
   - **Go offline** or disable sync temporarily
   - Edit the same note: "Modified content from Device A"
   - Wait for auto-save

4. **On Device B:**
   - Re-enable sync
   - Sync to server
   - Verify sync succeeds (Device B's version is now on server)

5. **On Device A:**
   - Re-enable sync
   - Attempt to sync
   - **Conflict should be detected**

**Expected Result:**
- Sync completes but note shows conflict indicator (amber warning icon)
- Note list item displays the conflict icon next to the note
- Toast notification may appear indicating conflict detected

**Verification:**
- [ ] Conflict indicator visible on affected note
- [ ] Note content still shows Device A's local version
- [ ] Clicking conflict indicator opens resolution modal

---

### Scenario 2: Resolve with "Keep Mine"

**Purpose:** Verify local version is preserved and pushed to server.

**Prerequisites:** Complete Scenario 1 (conflict exists on Device A)

**Steps:**

1. **On Device A:**
   - Click the conflict indicator (amber icon) on the note
   - Conflict resolution modal should open
   - Verify both versions are displayed:
     - Left panel: "Your Version" with local content
     - Right panel: "Server Version" with Device B's content
   - Click **"Keep Mine"** button

2. **Verify resolution:**
   - Modal closes
   - Conflict indicator disappears
   - Sync again

3. **On Device B:**
   - Sync to pull changes
   - Verify note now has Device A's content

**Expected Result:**
- Device A's content ("Modified content from Device A") is preserved
- After sync, server has Device A's version
- Device B receives Device A's version on next pull

**Verification:**
- [ ] Conflict indicator removed after resolution
- [ ] Local content unchanged
- [ ] Server updated with local version after sync
- [ ] Other devices receive the updated version

---

### Scenario 3: Resolve with "Keep Server"

**Purpose:** Verify server version replaces local content.

**Prerequisites:** Trigger a new conflict (repeat Scenario 1 steps)

**Steps:**

1. **On Device A (with conflict):**
   - Click the conflict indicator
   - Review both versions in the modal
   - Click **"Keep Server"** button

2. **Verify resolution:**
   - Modal closes
   - Conflict indicator disappears
   - Note content updates to server version

**Expected Result:**
- Local content replaced with server version
- No sync needed (already matches server)
- Note marked as synced

**Verification:**
- [ ] Conflict indicator removed
- [ ] Note content now matches server version
- [ ] No pending sync required

---

### Scenario 4: Resolve with "Keep Both"

**Purpose:** Verify both versions are preserved as separate notes.

**Prerequisites:** Trigger a new conflict (repeat Scenario 1 steps)

**Steps:**

1. **On Device A (with conflict):**
   - Click the conflict indicator
   - Review both versions in the modal
   - Click **"Keep Both"** button

2. **Verify resolution:**
   - Modal closes
   - Original note retains local content
   - **New note created** with server content
   - New note has `conflict-copy` tag

3. **Check note list:**
   - Should see two notes now
   - One with local content
   - One with server content and `conflict-copy` tag

**Expected Result:**
- Original note preserved with local content
- New note created from server version
- New note tagged with `conflict-copy`
- Both notes sync on next sync operation

**Verification:**
- [ ] Conflict indicator removed from original note
- [ ] New note appears in note list
- [ ] New note has `conflict-copy` tag
- [ ] Both notes have correct content

---

### Scenario 5: Multiple Concurrent Conflicts

**Purpose:** Verify handling of conflicts on multiple notes.

**Steps:**

1. Create 3 notes and sync across both devices
2. Modify all 3 notes on Device B and sync
3. Modify all 3 notes differently on Device A
4. Sync Device A - all 3 should show conflicts
5. Resolve each with a different strategy:
   - Note 1: Keep Mine
   - Note 2: Keep Server
   - Note 3: Keep Both

**Expected Result:**
- Each conflict resolved independently
- Correct resolution applied to each note
- No data loss

**Verification:**
- [ ] All conflicts detected
- [ ] Each resolution works correctly
- [ ] Final state matches expectations

---

### Scenario 6: Conflict with Attachments

**Purpose:** Verify attachment metadata is preserved in conflict data.

**Steps:**

1. Create note with attachment on Device A
2. Sync to Device B
3. On Device B: modify note content, sync
4. On Device A: modify note content (different), try to sync
5. Open conflict modal

**Expected Result:**
- Conflict modal shows attachment info if server version has attachments
- Resolution preserves attachments appropriately

**Verification:**
- [ ] Attachment metadata visible in conflict info
- [ ] Attachments preserved after resolution

---

### Scenario 7: Cancel/Dismiss Conflict Modal

**Purpose:** Verify conflict persists when modal is dismissed without resolution.

**Steps:**

1. Trigger a conflict
2. Open conflict modal
3. Press Escape or click Cancel
4. Check that conflict indicator remains
5. Reopen modal, verify data still present

**Expected Result:**
- Modal closes without resolution
- Conflict indicator remains visible
- Can reopen and resolve later

**Verification:**
- [ ] Conflict not auto-resolved on dismiss
- [ ] Data preserved for later resolution

---

## Platform-Specific Tests

### Web Client

**Keyboard Shortcuts:**
- Escape closes conflict modal

**Visual Elements:**
- Conflict indicator: Amber warning triangle icon
- Modal: Side-by-side comparison view
- Buttons: Blue (Keep Mine), Purple (Keep Server), Grey (Keep Both)

### TUI Client

**Keyboard Shortcuts:**
- `x` on conflicting note opens resolution view
- `1` = Keep Mine
- `2` = Keep Server
- `3` = Keep Both
- `Escape` = Cancel/close

**Visual Elements:**
- Conflict indicator: `!` or warning symbol in note list
- Resolution view: Split-pane vertical layout

---

## Simulating Offline Conditions

### Browser (Web)
1. Open DevTools (F12)
2. Network tab → Check "Offline"
3. Or disable sync endpoint in Settings

### TUI
1. Disable sync in settings
2. Or block server port with firewall

### Alternative: Direct Database Modification

For testing without two clients, you can inject conflict data directly:

```javascript
// In browser console after unlocking app
// This simulates server having a different version

// 1. Get a note ID from the notes store
const db = await indexedDB.open('jottery');
// ... (complex - use the app's sync service instead)
```

---

## Troubleshooting

### Conflict Not Detected

- Verify both devices are syncing to same server
- Check that modifications have different timestamps
- Ensure server is running and accessible
- Check browser console for sync errors

### Modal Not Opening

- Check browser console for JavaScript errors
- Verify note has `conflict` status in sync metadata
- Try refreshing the page

### Resolution Not Working

- Check network connectivity
- Verify server is running
- Look for errors in server logs

---

## Known Issues

- Large notes may cause modal scrolling issues
- Conflict copy tag is English-only ("conflict-copy")

---

## Cleanup

After testing:

1. Delete test notes from both devices
2. If using test databases, delete them:
   ```bash
   rm ~/.jottery-test1.db ~/.jottery-test2.db
   ```
3. Clear browser storage if needed (DevTools → Application → Clear Storage)

---

## Test Results Log

| Date | Tester | Platform | Version | Scenario | Pass/Fail | Notes |
|------|--------|----------|---------|----------|-----------|-------|
| | | | | | | |

---

## Related Documentation

- [SYNC-SPEC.md](../../SYNC-SPEC.md) - Sync protocol specification
- [conflictService.test.ts](../../../src/lib/services/conflictService.test.ts) - Unit tests
- [conflict-resolution.spec.ts](../../../e2e/conflict-resolution.spec.ts) - E2E tests
