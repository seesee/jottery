# Jottery Testing Documentation

This document outlines the comprehensive testing strategy for Jottery and current test coverage.

## Testing Stack

### Unit & Integration Tests
- **Vitest** - Fast, Vite-native test runner
- **@testing-library/svelte** - Component testing (Svelte 5 support pending)
- **MSW (Mock Service Worker)** - HTTP request mocking
- **fake-indexeddb** - IndexedDB mocking for repository tests
- **happy-dom** - Lightweight DOM implementation

### E2E Tests
- **Playwright** - Cross-browser end-to-end testing (Chromium, Firefox, WebKit)

## Current Test Coverage

### ✅ Phase 1: Foundation (Complete)
**Test Infrastructure**
- Vitest configuration with coverage reporting
- Test utilities and fixtures (`src/test/db-utils.ts`)
- MSW server setup for API mocking

**Cryptography Tests** (`src/lib/services/crypto.test.ts`)
- 37 tests passing
- Key derivation (PBKDF2, consistency, salt variations)
- Text encryption/decryption round-trips
- Binary encryption for attachments
- JSON encryption helpers
- Edge cases: unicode, large files, tampering detection

### ✅ Phase 2: Repository Layer (Complete)
All repository tests use real IndexedDB operations (via fake-indexeddb) and encryption.

**noteRepository.test.ts**
- 49 tests (1 skipped due to fake-indexeddb boolean index limitation)
- CRUD operations, soft delete/restore, queries, sync operations
- Edge cases: unicode, large content, attachments

**attachmentRepository.test.ts**
- 35 tests passing
- Blob storage, thumbnail storage, combined operations, utility functions

**versionRepository.test.ts**
- 30 tests passing
- Version creation with deduplication, retrieval, deletion, counting

**settingsRepository.test.ts**
- 67 tests passing
- Settings CRUD, all fields (theme, language, sync, etc.), edge cases

**encryptionRepository.test.ts**
- 56 tests passing
- Metadata CRUD, initialization checks, encryption workflows

**syncRepository.test.ts**
- 90 tests passing
- Global/note sync metadata, pending notes, conflict counting, batch operations

### ✅ Phase 3: Sync Service (Complete)
**syncService.test.ts**
- 16 tests passing, 6 skipped
- **Covered**: Registration, manual configuration, authorization, error handling
- **Skipped** (require store/search service setup):
  - Encrypt note content before push
  - Handle push with no notes
  - Pull and decrypt remote notes
  - Concurrent sync prevention
  - Full bidirectional sync workflow
  - Sync timestamp updates

### ⏸️ Phase 4: Component Tests (Blocked)
**Status**: Pending Svelte 5 tooling maturity

**Issue**: @testing-library/svelte does not fully support Svelte 5's new `mount()` API in test environments. Tests fail with:
```
Svelte error: lifecycle_function_unavailable
`mount(...)` is not available on the server
```

**Alternative Approaches**:
1. **Playwright Component Testing** - Browser-based component testing (recommended for Svelte 5)
2. **Wait for @testing-library/svelte** - Update when Svelte 5 support is stable
3. **Manual browser testing** - Current approach for component validation

**Components Requiring Tests** (when tooling ready):
- TagInput
- ConfirmModal
- Toast
- Header
- EditorPane (complex, many dependencies)
- Settings tabs
- Modals (VersionHistory, Documentation, Releases)

### ✅ Phase 5: E2E Tests (Complete)
**Playwright Configuration**: Configured for Chromium, Firefox, WebKit (Safari)

**Authentication Tests** (`e2e/auth.spec.ts`)
- 6 tests passing
- ✅ Password setup screen on first visit
- ✅ Create password and unlock app
- ✅ Password mismatch error during setup
- ✅ Lock and unlock app cycle
- ✅ Remember encryption after page reload
- ✅ Wrong password error message

**Note Operations Tests** (`e2e/notes.spec.ts`)
- 7 tests passing
- ✅ Create a new note
- ✅ Edit an existing note
- ✅ Add tags to a note
- ✅ Pin a note
- ✅ Delete a note
- ✅ Create multiple notes
- ✅ Select and switch between notes

**Smoke Tests** (`e2e/smoke.spec.ts`)
- 6 tests passing - Fast sanity checks
- ✅ App loads without crashing
- ✅ Can complete basic setup and unlock flow
- ✅ App reloads successfully
- ✅ Basic navigation elements are present
- ✅ JavaScript is working
- ✅ No critical console errors on load

**Workflow Tests** (`e2e/workflows.spec.ts`)
- 7 tests passing, 1 skipped
- **Search and Filtering**:
  - ✅ Search notes by text content
  - ✅ Handle empty search results gracefully
- **Tag Management**:
  - ✅ Create note with tags and filter by tag
  - ✅ Remove tags from note
- **Settings**:
  - ✅ Open settings modal
  - ✅ Change theme setting
  - ⏸️ Change language setting (single option in test env)
- **Complete User Journey**:
  - ✅ Full workflow: create, edit, search, delete

**Bulk Operations Tests** (`e2e/bulk-operations.spec.ts`)
- 8 tests passing
- ✅ Select and deselect multiple notes
- ✅ Select all and deselect all
- ✅ Bulk pin and unpin notes
- ✅ Bulk export to JSON
- ✅ Bulk delete notes
- ✅ Cancel bulk delete
- ✅ Combine notes with tags merged by default
- ✅ Combine notes without merging tags when checkbox unchecked

**Calculator Mode Tests** (`e2e/calc.spec.ts`)
- 8 tests passing
- ✅ Basic arithmetic result display
- ✅ Variables across lines
- ✅ Math function evaluation
- ✅ Comment handling
- ✅ Inline error display
- ✅ Unit conversions
- ✅ Result updates on editing
- ✅ Calc language persistence on note reopen

**Recycle Bin Tests** (`e2e/recycle-bin.spec.ts`)
- 4 tests passing
- ✅ Restore note and immediately show in notes list
- ✅ Restore note and make it searchable
- ✅ Show empty recycle bin message
- ✅ Permanently delete note from recycle bin

### ✅ Phase 6: CI/CD Integration (Complete)
**GitHub Actions Workflow** (`.github/workflows/test.yml`)

**Jobs:**
1. **Unit & Integration Tests**
   - Runs all Vitest tests
   - Uploads coverage to Codecov
   - Runs on: push to main, pull requests

2. **E2E Tests (Chromium)**
   - Runs all Playwright E2E tests
   - Uploads test reports on failure
   - Runs on: push to main, pull requests

3. **Smoke Tests (Quick)**
   - Fast sanity checks only
   - Catches critical breakage early
   - Runs on: push to main, pull requests

4. **Type Check**
   - Runs svelte-check for TypeScript validation
   - Catches type errors before merge
   - Runs on: push to main, pull requests

**Triggers:**
- Push to `main` branch
- Pull requests to `main` branch

**Test Execution Time:**
- Smoke tests: ~10 seconds
- Unit tests: ~5-10 seconds
- E2E tests: ~45 seconds
- Type check: ~10 seconds
- **Total**: ~70 seconds

## Running Tests

### All Unit/Integration Tests
```bash
npm test
```

### Specific Test File
```bash
npm test -- src/lib/services/crypto.test.ts
```

### Watch Mode
```bash
npm test -- --watch
```

### Coverage Report
```bash
npm test -- --coverage
```

### E2E Tests
```bash
# All E2E tests
npx playwright test

# Smoke tests only (fast sanity checks)
npx playwright test e2e/smoke.spec.ts

# Workflow tests (search, settings, complete journeys)
npx playwright test e2e/workflows.spec.ts

# Specific E2E test file
npx playwright test e2e/auth.spec.ts

# Run in headed mode (see browser)
npx playwright test --headed

# Run only Chromium (skip Firefox/Safari)
npx playwright test --project=chromium
```

## Test Statistics

**Unit/Integration Tests**: 298 passing, 7 skipped (305 total)
**E2E Tests**: 45 tests × 3 browsers = 135 test runs
**Test Files**: 16 (9 unit/integration + 7 E2E)
**Coverage Goals**: 80% (lines, functions, branches, statements)

### Breakdown

**Unit/Integration Tests:**
- Crypto: 37 tests
- Note Repository: 49 tests (1 skipped)
- Attachment Repository: 35 tests
- Version Repository: 30 tests
- Settings Repository: 67 tests
- Encryption Repository: 56 tests
- Sync Repository: 90 tests
- Sync Service: 22 tests (6 skipped)

**E2E Tests:**
- Authentication: 6 tests
- Note Operations: 7 tests
- Smoke Tests: 6 tests (fast sanity checks)
- Workflow Tests: 7 tests (1 skipped)
- Bulk Operations: 8 tests
- Calculator Mode: 8 tests
- Recycle Bin: 4 tests

## Known Limitations

### 1. fake-indexeddb Boolean Index
**Issue**: fake-indexeddb doesn't support queries on boolean indices
**Impact**: 1 test skipped in noteRepository.test.ts (`getNotesNeedingSync()`)
**Workaround**: Test skipped with explanation; real IndexedDB works correctly
**Status**: Documented in test with `.skip()` and comment

### 2. Svelte 5 Component Testing
**Issue**: @testing-library/svelte doesn't support Svelte 5's mount() API in Node test environments
**Impact**: Phase 4 (Component Tests) blocked
**Workaround**: Manual browser testing, Playwright component tests (future)
**Status**: Documented; waiting for ecosystem maturity

### 3. Complex Sync Workflows
**Issue**: Testing full sync workflows requires Svelte stores, search service, and complex async coordination
**Impact**: 6 tests skipped in syncService.test.ts
**Workaround**: Core sync functionality tested; complex workflows tested manually
**Status**: Marked with `.skip()` and TODO comments

## Best Practices

### Test Isolation
- Each test has independent database setup/teardown
- No shared state between tests
- Tests can run in any order

### Encryption Testing
- All repository tests use real encryption
- Master key properly set up in `beforeEach`
- Cleaned up in `afterEach`

### Async Handling
- Proper `await` for all async operations
- No race conditions
- Timeouts for network mocks

### Mocking
- MSW for HTTP requests (not `fetch` mocks)
- Real IndexedDB operations (via fake-indexeddb)
- Minimal mocking for better confidence

## Future Work

1. **Svelte 5 Component Testing**
   - Monitor @testing-library/svelte updates
   - Consider Playwright Component Testing
   - Document migration path when ready

2. **Mobile Viewport Testing**
   - Add mobile viewport E2E tests
   - Touch interaction testing
   - Responsive layout verification

3. **TUI Testing**
   - Rust unit tests
   - Integration tests
   - CLI command testing
   - Sync compatibility tests

4. **Performance Benchmarks**
   - Baseline performance metrics
   - Large dataset testing
   - Memory usage monitoring

## Contributing

When adding tests:
1. Follow existing patterns (see `crypto.test.ts` for examples)
2. Use descriptive test names
3. Test both happy path and edge cases
4. Add comments for complex test logic
5. Update this document if adding new test categories
6. Aim for 80%+ coverage on new code

## References

- [Vitest Documentation](https://vitest.dev/)
- [Testing Library](https://testing-library.com/)
- [MSW Documentation](https://mswjs.io/)
- [Svelte Testing](https://testing-library.com/docs/svelte-testing-library/intro/)
