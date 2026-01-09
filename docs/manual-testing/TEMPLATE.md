# Feature Name - Manual Testing Guide

> Brief description of what this feature does and why manual testing is valuable.

## Prerequisites

- [ ] Jottery server running locally or accessible remotely
- [ ] Web client accessible at `http://localhost:5173` (or production URL)
- [ ] TUI client compiled and available
- [ ] Test account credentials (if applicable)
- [ ] Any additional requirements...

## Test Environment Setup

### Option A: Local Development

```bash
# Terminal 1: Start server
cd server && cargo run

# Terminal 2: Start web client
npm run dev

# Terminal 3: Start TUI (optional)
cd tui && cargo run
```

### Option B: Production-like Environment

Describe any Docker or deployment setup needed.

## Test Scenarios

### Scenario 1: [Basic Functionality]

**Purpose:** Verify the core feature works as expected.

**Steps:**
1. Step one...
2. Step two...
3. Step three...

**Expected Result:**
- What you should see...
- What data should be created/modified...

**Verification:**
- [ ] Checkbox for tester to confirm

---

### Scenario 2: [Edge Case]

**Purpose:** Test boundary conditions or unusual inputs.

**Steps:**
1. Step one...
2. Step two...

**Expected Result:**
- Description of expected behaviour...

**Verification:**
- [ ] Checkbox for tester to confirm

---

### Scenario 3: [Error Handling]

**Purpose:** Verify graceful handling of errors.

**Steps:**
1. Step one...
2. Step two...

**Expected Result:**
- Error message should be displayed...
- No data loss should occur...

**Verification:**
- [ ] Checkbox for tester to confirm

## Platform-Specific Tests

### Web Client

Additional steps or considerations for the web client.

### TUI Client

Additional steps or considerations for the terminal UI.

## Known Issues

- List any known bugs or limitations that affect testing
- Reference issue numbers if applicable

## Cleanup

Steps to reset the environment after testing:

1. Delete test notes
2. Clear sync data if needed
3. Any other cleanup steps...

## Test Results Log

| Date | Tester | Platform | Version | Pass/Fail | Notes |
|------|--------|----------|---------|-----------|-------|
| | | | | | |

## Related Documentation

- Link to relevant specs or design docs
- Link to related automated tests
- Link to user documentation
