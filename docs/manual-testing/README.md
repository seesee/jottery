# Manual Testing Guide

This directory contains step-by-step instructions for manually testing Jottery features. While automated tests cover the majority of functionality, manual testing is essential for:

- Verifying UI/UX flows work as expected
- Testing edge cases that are difficult to automate
- Validating multi-device sync scenarios
- Confirming cross-platform consistency (Web, TUI)

## Directory Structure

```
docs/manual-testing/
├── README.md                    # This file
├── TEMPLATE.md                  # Template for new test documents
└── sync/
    └── conflict-resolution.md   # Sync conflict detection and resolution
```

## How to Use These Guides

Each testing document follows a consistent format:

1. **Prerequisites** - What you need before starting
2. **Setup** - How to prepare the test environment
3. **Test Scenarios** - Step-by-step test cases
4. **Expected Results** - What should happen
5. **Cleanup** - How to reset after testing

## Test Environment Options

### Local Development
```bash
# Start web client
npm run dev

# Start server (in server/ directory)
cargo run

# Start TUI (in tui/ directory)
cargo run
```

### With Docker (if configured)
```bash
docker-compose up
```

## Feature Test Documents

| Feature | Document | Components |
|---------|----------|------------|
| Sync Conflict Resolution | [sync/conflict-resolution.md](sync/conflict-resolution.md) | Server, Web, TUI |

## Contributing New Tests

When adding manual tests for a new feature:

1. Copy `TEMPLATE.md` to the appropriate subdirectory
2. Follow the established format
3. Include screenshots or diagrams where helpful
4. Test on all relevant platforms (Web, TUI)
5. Document any known issues or limitations

## Related Documentation

- [TESTING.md](../../TESTING.md) - Automated test documentation
- [SYNC-SPEC.md](../SYNC-SPEC.md) - Sync protocol specification
- [README.md](../../README.md) - Project overview
