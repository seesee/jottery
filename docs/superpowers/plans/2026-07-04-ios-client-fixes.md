# iOS Client Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix three iOS client defects: FaceID prompting twice on app open, attachments not appearing in the editor after being added, and no way to share or paste content into a note.

**Architecture:** All work is in `ios-native/Jottery/`. The FaceID fix adds a re-entrancy guard to `KeyManager.attemptBiometricUnlock` plus a once-only auto-attempt in `UnlockScreen`. The attachment fix re-fetches the note from the repository after mutation so `DecryptedNote`'s `Equatable` (id/modifiedAt/version) sees a change and SwiftUI re-renders. Paste support adds a `PasteboardService` + a toolbar action (Runestone's internal `TextInputView` owns in-editor paste and only handles strings, so in-editor interception is impossible without forking). Share-in adds a `JotteryShare` app-extension target that stages items unencrypted in an app-group container (`group.com.jottery.ios`); the main app imports staged items into encrypted notes when unlocked. Because the extension never has the master key, staging is plaintext-on-disk protected by iOS file protection — documented trade-off.

**Tech Stack:** Swift 6 (strict concurrency), SwiftUI, GRDB, Swift Testing (`import Testing`), Xcode 26.6, iOS 17 deployment target.

## Global Constraints

- Branch: `feature/ios-client-fixes` (already created). Never commit to main.
- British English in all user-facing text, comments, and docs.
- Commit after each logical change (CLAUDE.md mandatory commit rules).
- No native browser-style blocking controls; SwiftUI UI conventions as in existing code.
- Bundle ids: app `com.jottery.ios`, team `QA72EA3LC2`, deployment target 17.0, `SWIFT_VERSION 6.0`, `SWIFT_STRICT_CONCURRENCY = complete`.
- pbxproj object-id convention: existing file uses hand-rolled ids (`A5000001` etc.); new ids use `E`-prefix (`E3000001`…) and `F`-prefix for the share extension.
- Build/test command (simulator): `xcodebuild -project ios-native/Jottery/Jottery.xcodeproj -scheme Jottery -destination 'platform=iOS Simulator,name=iPhone 17' build` / `… test`.

---

### Task 1: Wire the orphaned JotteryTests target into the Xcode project

`JotteryTests/CryptoServiceTests.swift` exists on disk but no test target exists in `project.pbxproj`. Every later task's TDD cycle depends on this.

**Files:**
- Modify: `ios-native/Jottery/Jottery.xcodeproj/project.pbxproj`
- Create: `ios-native/Jottery/Jottery.xcodeproj/xcshareddata/xcschemes/Jottery.xcscheme`

**Interfaces:**
- Produces: a runnable `JotteryTests` unit-test bundle (app-hosted, `@testable import Jottery`) and a shared `Jottery` scheme whose TestAction includes it.

- [ ] **Step 1: Add the test target to project.pbxproj**

Apply all of the following edits (section order matters only for readability; Xcode accepts any order within a section):

In `/* Begin PBXFileReference section */` add:
```
		E3000001 /* JotteryTests.xctest */ = {isa = PBXFileReference; explicitFileType = wrapper.cfbundle; includeInIndex = 0; path = JotteryTests.xctest; sourceTree = BUILT_PRODUCTS_DIR; };
```

In `/* Begin PBXFileSystemSynchronizedRootGroup section */` add:
```
		E6000001 /* JotteryTests */ = {
			isa = PBXFileSystemSynchronizedRootGroup;
			path = JotteryTests;
			sourceTree = "<group>";
		};
```

In the root group `A8000001` children, after `A6000001 /* Jottery */`, add:
```
				E6000001 /* JotteryTests */,
```

In the Products group `A9000001` children add:
```
				E3000001 /* JotteryTests.xctest */,
```

In `/* Begin PBXNativeTarget section */` add:
```
		E5000001 /* JotteryTests */ = {
			isa = PBXNativeTarget;
			buildConfigurationList = EA000001 /* Build configuration list for PBXNativeTarget "JotteryTests" */;
			buildPhases = (
				E7000001 /* Sources */,
			);
			buildRules = (
			);
			dependencies = (
				E8000001 /* PBXTargetDependency */,
			);
			fileSystemSynchronizedGroups = (
				E6000001 /* JotteryTests */,
			);
			name = JotteryTests;
			packageProductDependencies = (
			);
			productName = JotteryTests;
			productReference = E3000001 /* JotteryTests.xctest */;
			productType = "com.apple.product-type.bundle.unit-test";
		};
```

In the `PBXProject` section: inside `TargetAttributes` add
```
					E5000001 = {
						CreatedOnToolsVersion = 16.0;
						TestTargetID = A5000001;
					};
```
and in `targets = (` add `E5000001 /* JotteryTests */,` after the app target.

Add new sections (before `/* Begin XCBuildConfiguration section */`):
```
/* Begin PBXContainerItemProxy section */
		E9000001 /* PBXContainerItemProxy */ = {
			isa = PBXContainerItemProxy;
			containerPortal = AD000001 /* Project object */;
			proxyType = 1;
			remoteGlobalIDString = A5000001;
			remoteInfo = Jottery;
		};
/* End PBXContainerItemProxy section */

/* Begin PBXTargetDependency section */
		E8000001 /* PBXTargetDependency */ = {
			isa = PBXTargetDependency;
			target = A5000001 /* Jottery */;
			targetProxy = E9000001 /* PBXContainerItemProxy */;
		};
/* End PBXTargetDependency section */
```

In `/* Begin PBXSourcesBuildPhase section */` add:
```
		E7000001 /* Sources */ = {
			isa = PBXSourcesBuildPhase;
			buildActionMask = 2147483647;
			files = (
			);
			runOnlyForDeploymentPostprocessing = 0;
		};
```

In `/* Begin XCBuildConfiguration section */` add:
```
		EB000001 /* Debug */ = {
			isa = XCBuildConfiguration;
			buildSettings = {
				BUNDLE_LOADER = "$(TEST_HOST)";
				CODE_SIGN_STYLE = Automatic;
				CURRENT_PROJECT_VERSION = 10102;
				GENERATE_INFOPLIST_FILE = YES;
				MARKETING_VERSION = 1.1.2;
				PRODUCT_BUNDLE_IDENTIFIER = com.jottery.ios.tests;
				PRODUCT_NAME = "$(TARGET_NAME)";
				SWIFT_EMIT_LOC_STRINGS = NO;
				SWIFT_STRICT_CONCURRENCY = complete;
				SWIFT_VERSION = 6.0;
				TARGETED_DEVICE_FAMILY = "1,2";
				TEST_HOST = "$(BUILT_PRODUCTS_DIR)/Jottery.app/$(BUNDLE_EXECUTABLE_FOLDER_PATH)/Jottery";
			};
			name = Debug;
		};
		EB000002 /* Release */ = {
			isa = XCBuildConfiguration;
			buildSettings = {
				BUNDLE_LOADER = "$(TEST_HOST)";
				CODE_SIGN_STYLE = Automatic;
				CURRENT_PROJECT_VERSION = 10102;
				GENERATE_INFOPLIST_FILE = YES;
				MARKETING_VERSION = 1.1.2;
				PRODUCT_BUNDLE_IDENTIFIER = com.jottery.ios.tests;
				PRODUCT_NAME = "$(TARGET_NAME)";
				SWIFT_EMIT_LOC_STRINGS = NO;
				SWIFT_STRICT_CONCURRENCY = complete;
				SWIFT_VERSION = 6.0;
				TARGETED_DEVICE_FAMILY = "1,2";
				TEST_HOST = "$(BUILT_PRODUCTS_DIR)/Jottery.app/$(BUNDLE_EXECUTABLE_FOLDER_PATH)/Jottery";
			};
			name = Release;
		};
```

In `/* Begin XCConfigurationList section */` add:
```
		EA000001 /* Build configuration list for PBXNativeTarget "JotteryTests" */ = {
			isa = XCConfigurationList;
			buildConfigurations = (
				EB000001 /* Debug */,
				EB000002 /* Release */,
			);
			defaultConfigurationIsVisible = 0;
			defaultConfigurationName = Release;
		};
```

- [ ] **Step 2: Create a shared scheme with a TestAction**

Create `ios-native/Jottery/Jottery.xcodeproj/xcshareddata/xcschemes/Jottery.xcscheme`:
```xml
<?xml version="1.0" encoding="UTF-8"?>
<Scheme LastUpgradeVersion = "2620" version = "1.7">
   <BuildAction parallelizeBuildables = "YES" buildImplicitDependencies = "YES">
      <BuildActionEntries>
         <BuildActionEntry buildForTesting = "YES" buildForRunning = "YES" buildForProfiling = "YES" buildForArchiving = "YES" buildForAnalyzing = "YES">
            <BuildableReference BuildableState = "buildable" BlueprintIdentifier = "A5000001" BuildableName = "Jottery.app" BlueprintName = "Jottery" ReferencedContainer = "container:Jottery.xcodeproj"/>
         </BuildActionEntry>
      </BuildActionEntries>
   </BuildAction>
   <TestAction buildConfiguration = "Debug" selectedDebuggerIdentifier = "Xcode.DebuggerFoundation.Debugger.LLDB" selectedLauncherIdentifier = "Xcode.DebuggerFoundation.Launcher.LLDB" shouldUseLaunchSchemeArgsEnv = "YES">
      <Testables>
         <TestableReference skipped = "NO">
            <BuildableReference BuildableState = "buildable" BlueprintIdentifier = "E5000001" BuildableName = "JotteryTests.xctest" BlueprintName = "JotteryTests" ReferencedContainer = "container:Jottery.xcodeproj"/>
         </TestableReference>
      </Testables>
   </TestAction>
   <LaunchAction buildConfiguration = "Debug" selectedDebuggerIdentifier = "Xcode.DebuggerFoundation.Debugger.LLDB" selectedLauncherIdentifier = "Xcode.DebuggerFoundation.Launcher.LLDB" launchStyle = "0" useCustomWorkingDirectory = "NO" ignoresPersistentStateOnLaunch = "NO" debugDocumentVersioning = "YES" debugServiceExtension = "internal" allowLocationSimulation = "YES">
      <BuildableProductRunnable runnableDebuggingMode = "0">
         <BuildableReference BuildableState = "buildable" BlueprintIdentifier = "A5000001" BuildableName = "Jottery.app" BlueprintName = "Jottery" ReferencedContainer = "container:Jottery.xcodeproj"/>
      </BuildableProductRunnable>
   </LaunchAction>
   <ArchiveAction buildConfiguration = "Release" revealArchiveInOrganizer = "YES"/>
</Scheme>
```

- [ ] **Step 3: Run the existing tests to verify the target works**

Run: `xcodebuild -project ios-native/Jottery/Jottery.xcodeproj -scheme Jottery -destination 'platform=iOS Simulator,name=iPhone 17' test 2>&1 | tail -20`
Expected: `** TEST SUCCEEDED **` with CryptoServiceTests passing.

- [ ] **Step 4: Commit**

```bash
git add ios-native/Jottery/Jottery.xcodeproj
git commit -m "test(ios): wire orphaned JotteryTests target into Xcode project"
```

---

### Task 2: Attachment add/remove doesn't refresh the editor (jottery-lm4k part 1)

`AppState.addAttachment` writes to the DB (repo bumps `modifiedAt` + `needsSync`, hence it syncs) but puts a stale in-memory copy into `notes[index]`. `DecryptedNote.==` compares only id/modifiedAt/version, so SwiftUI never re-renders `NoteEditorView`. Fix: re-fetch from the repository after mutation. Requires a testability refactor first because AppState's repos are `private(set)` and only wired inside `initialise()`.

**Files:**
- Modify: `ios-native/Jottery/Jottery/ViewModels/AppState.swift:165-199` (initialise refactor), `:583-648` (add/removeAttachment)
- Test: `ios-native/Jottery/JotteryTests/AppStateAttachmentTests.swift` (create)

**Interfaces:**
- Produces: `AppState.initialise(database: DatabaseManager) throws` — internal, used by all later AppState tests.
- Consumes: `DatabaseManager(path: String)` (exists), `KeyManager.unlockWithKeyData(Data)` (exists).

- [ ] **Step 1: Refactor initialise() to accept an injectable database**

Replace the body of `initialise()` in `AppState.swift` so the wiring lives in a new internal overload:

```swift
    func initialise() {
        guard db == nil else { return }  // Already initialised

        do {
            try initialise(database: DatabaseManager())
        } catch {
            // Database init failed — stay on first launch screen
            isFirstLaunch = true
        }
    }

    /// Wire repositories to a specific database. Internal so tests can
    /// inject a temporary database instead of the app-support default.
    func initialise(database: DatabaseManager) throws {
        // Wire auto-lock so the timer triggers a full app lock (UI + key wipe)
        keyManager.onAutoLock = { [weak self] in
            self?.lock()
        }

        self.db = database
        let verRepo = VersionRepository(db: database)
        self.versionRepo = verRepo
        self.noteRepo = NoteRepository(db: database, versionRepo: verRepo)
        self.encryptionRepo = EncryptionRepository(db: database)
        self.settingsRepo = SettingsRepository(db: database)
        self.syncRepo = SyncRepository(db: database)
        self.attachmentRepo = AttachmentRepository(db: database)
        self.savedSearchRepo = SavedSearchRepository(db: database)

        // Check if vault exists
        let hasVault = try encryptionRepo?.isVaultSetUp() ?? false
        isFirstLaunch = !hasVault

        // Load settings
        if let loaded = try settingsRepo?.get() {
            settings = loaded
            sortOrder = loaded.sort
            keyManager.autoLockTimeout = TimeInterval(loaded.autoLockTimeout * 60)
        }
    }
```

- [ ] **Step 2: Build to confirm the refactor compiles**

Run: `xcodebuild -project ios-native/Jottery/Jottery.xcodeproj -scheme Jottery -destination 'platform=iOS Simulator,name=iPhone 17' build 2>&1 | tail -3`
Expected: `** BUILD SUCCEEDED **`

- [ ] **Step 3: Commit the refactor**

```bash
git add ios-native/Jottery/Jottery/ViewModels/AppState.swift
git commit -m "refactor(ios): allow injecting a database into AppState for tests"
```

- [ ] **Step 4: Write the failing tests**

Create `ios-native/Jottery/JotteryTests/AppStateAttachmentTests.swift`:

```swift
import CryptoKit
import Foundation
import Testing

@testable import Jottery

@MainActor
struct AppStateAttachmentTests {

    /// AppState wired to a throwaway on-disk database with an in-memory key.
    private func makeUnlockedState() throws -> AppState {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let state = AppState()
        try state.initialise(database: DatabaseManager(path: dir.appendingPathComponent("test.db").path))
        state.keyManager.unlockWithKeyData(Data(repeating: 7, count: 32))
        state.isLocked = false
        return state
    }

    private func makeTempFile(named name: String) throws -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("\(UUID().uuidString)-\(name)")
        try Data([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]).write(to: url)
        return url
    }

    @Test func addAttachmentRefreshesInMemoryNote() throws {
        let state = try makeUnlockedState()
        let created = try #require(try state.createNote(content: "hello"))
        let before = try #require(state.notes.first { $0.id == created.id })

        let file = try makeTempFile(named: "pasted.png")
        try state.addAttachment(to: created.id, url: file, filename: "pasted.png", mimeType: "image/png")

        let after = try #require(state.notes.first { $0.id == created.id })
        #expect(after.attachments.count == 1)
        #expect(after.attachments.first?.filename == "pasted.png")
        // SwiftUI diffs NoteEditorView's `note` input via DecryptedNote's
        // Equatable — the updated copy must compare as changed or the
        // attachment list never re-renders.
        #expect(after != before)
    }

    @Test func removeAttachmentRefreshesInMemoryNote() throws {
        let state = try makeUnlockedState()
        let created = try #require(try state.createNote(content: "hello"))
        let file = try makeTempFile(named: "doc.pdf")
        try state.addAttachment(to: created.id, url: file, filename: "doc.pdf", mimeType: "application/pdf")

        let before = try #require(state.notes.first { $0.id == created.id })
        let attachmentId = try #require(before.attachments.first?.id)

        try state.removeAttachment(from: created.id, attachmentId: attachmentId)

        let after = try #require(state.notes.first { $0.id == created.id })
        #expect(after.attachments.isEmpty)
        #expect(after != before)
    }
}
```

- [ ] **Step 5: Run tests to verify they fail on the equality assertions**

Run: `xcodebuild -project ios-native/Jottery/Jottery.xcodeproj -scheme Jottery -destination 'platform=iOS Simulator,name=iPhone 17' test 2>&1 | grep -E "Test .* (passed|failed)|TEST"`
Expected: both new tests FAIL at `#expect(after != before)` (the in-memory copy compares equal); CryptoServiceTests still pass.

- [ ] **Step 6: Fix addAttachment/removeAttachment to re-fetch from the repository**

In `AppState.swift`, replace the tail of `addAttachment` (the `// Update note (in-memory copy uses plaintext filename for display)` block through the end of the function) with:

```swift
        // Save with encrypted filename in the raw record
        try noteRepo.addAttachment(noteId: noteId, ref: ref)

        // Re-fetch so the in-memory copy carries the bumped modifiedAt —
        // DecryptedNote's Equatable ignores `attachments`, so a stale
        // modifiedAt would stop SwiftUI re-rendering the editor.
        refreshNoteFromStore(id: noteId)
```

(The `var updated = note` / `updated.attachments.append(...)` block and the trailing `notes[index] = updated` are deleted. The `guard let note = try noteRepo.get(...)` at the top of the function becomes `guard try noteRepo.get(id: noteId, key: key) != nil else { return }` since `note` is no longer used — or simpler, keep reading it but ignore; prefer the guard form.)

Replace the tail of `removeAttachment` (`// Update in-memory` block) with:

```swift
        refreshNoteFromStore(id: noteId)
```

Add the shared helper after `removeAttachment`:

```swift
    /// Replace the in-memory copy of a note with the current database state.
    private func refreshNoteFromStore(id: String) {
        guard let noteRepo, let key = keyManager.masterKey else { return }
        guard let fresh = try? noteRepo.get(id: id, key: key) else { return }
        if let index = notes.firstIndex(where: { $0.id == id }) {
            notes[index] = fresh
        }
        if let index = archivedNotes.firstIndex(where: { $0.id == id }) {
            archivedNotes[index] = fresh
        }
    }
```

- [ ] **Step 7: Run tests to verify they pass**

Run: same test command as Step 5.
Expected: all tests PASS.

- [ ] **Step 8: Commit**

```bash
git add ios-native/Jottery/Jottery/ViewModels/AppState.swift ios-native/Jottery/JotteryTests/AppStateAttachmentTests.swift
git commit -m "fix(ios): refresh in-memory note after attachment changes so the editor re-renders"
```

---

### Task 3: FaceID prompts twice on app open (jottery-wkj8)

The prompt is raised implicitly by `SecItemCopyMatching` on the `.biometryCurrentSet` keychain item; `UnlockScreen.onAppear` fires `biometricUnlock()` with no guard, and the appear lifecycle runs twice on cold launch (Setup→Unlock branch swap + scene-phase churn from the FaceID sheet itself).

**Files:**
- Modify: `ios-native/Jottery/Jottery/Crypto/KeyManager.swift:73-83`
- Modify: `ios-native/Jottery/Jottery/Views/UnlockScreen.swift:73-78`
- Test: `ios-native/Jottery/JotteryTests/KeyManagerTests.swift` (create)

**Interfaces:**
- Produces: `KeyManager.attemptBiometricUnlock(retrieve:)` — same public behaviour, default argument keeps existing call sites (`UnlockScreen.swift:126`) source-compatible.

- [ ] **Step 1: Write the failing test**

Create `ios-native/Jottery/JotteryTests/KeyManagerTests.swift`:

```swift
import Foundation
import Testing

@testable import Jottery

@MainActor
struct KeyManagerTests {

    @Test func concurrentBiometricUnlockAttemptsOnlyTriggerOnePrompt() async {
        let manager = KeyManager()
        // Each retriever invocation stands in for one Face ID prompt.
        let counter = PromptCounter()
        let slowRetrieve: @Sendable () async throws -> Data = {
            await counter.increment()
            try await Task.sleep(for: .milliseconds(200))
            return Data(repeating: 7, count: 32)
        }

        // Two overlapping attempts — mirrors UnlockScreen.onAppear firing twice.
        async let first = manager.attemptBiometricUnlock(retrieve: slowRetrieve)
        async let second = manager.attemptBiometricUnlock(retrieve: slowRetrieve)
        let results = await [first, second]

        #expect(await counter.count == 1)          // one prompt, not two
        #expect(results.contains(true))            // the real attempt succeeded
        #expect(manager.isUnlocked)
    }

    @Test func failedAttemptClearsInFlightFlag() async {
        let manager = KeyManager()
        let failing: @Sendable () async throws -> Data = {
            throw KeyManagerError.noKeyLoaded
        }
        let result = await manager.attemptBiometricUnlock(retrieve: failing)
        #expect(result == false)

        // A later attempt must not be blocked by a stuck flag.
        let succeeding: @Sendable () async throws -> Data = {
            Data(repeating: 7, count: 32)
        }
        let retry = await manager.attemptBiometricUnlock(retrieve: succeeding)
        #expect(retry == true)
    }
}

private actor PromptCounter {
    var count = 0
    func increment() { count += 1 }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: the standard test command.
Expected: FAIL to compile (`attemptBiometricUnlock` has no `retrieve` parameter) — that is the failing state for a signature-level change; note it and proceed.

- [ ] **Step 3: Add the re-entrancy guard to KeyManager**

Replace `attemptBiometricUnlock` in `KeyManager.swift`:

```swift
    /// Guards against overlapping biometric prompts — the keychain read
    /// below implicitly presents Face ID, so two concurrent calls would
    /// queue two prompts (seen when UnlockScreen's appear lifecycle
    /// runs twice on cold launch).
    private var isBiometricUnlockInFlight = false

    /// Attempt to retrieve the master key via biometric authentication.
    /// Returns `true` if successful. `retrieve` is injectable for tests.
    func attemptBiometricUnlock(
        retrieve: @Sendable () async throws -> Data = { try await KeychainService.retrieveBiometricKey() }
    ) async -> Bool {
        guard !isBiometricUnlockInFlight else { return false }
        isBiometricUnlockInFlight = true
        defer { isBiometricUnlockInFlight = false }
        do {
            let keyData = try await retrieve()
            unlockWithKeyData(keyData)
            return true
        } catch {
            return false
        }
    }
```

- [ ] **Step 4: Guard the automatic attempt in UnlockScreen**

In `UnlockScreen.swift` add a state flag and gate `onAppear`:

```swift
    @State private var didAutoAttemptBiometric = false
```

```swift
        .onAppear {
            // Attempt biometric unlock automatically — once per lock cycle.
            // onAppear can fire more than once (Setup→Unlock branch swap,
            // scene-phase churn from the Face ID sheet itself).
            if appState.keyManager.isBiometricEnabled && !didAutoAttemptBiometric {
                didAutoAttemptBiometric = true
                biometricUnlock()
            }
        }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: the standard test command.
Expected: all tests PASS.

- [ ] **Step 6: Commit**

```bash
git add ios-native/Jottery/Jottery/Crypto/KeyManager.swift ios-native/Jottery/Jottery/Views/UnlockScreen.swift ios-native/Jottery/JotteryTests/KeyManagerTests.swift
git commit -m "fix(ios): stop Face ID prompting twice on app open (jottery-wkj8)"
```

---

### Task 4: Paste an image/data into a note (jottery-lm4k part 2)

Runestone's internal `TextInputView` owns paste (`canPerformAction` requires `hasStrings`; `paste()` handles strings only), so in-editor image paste cannot be intercepted. Add a `PasteboardService` and a "Paste from Clipboard" action in the editor toolbar menu, shown only when the clipboard holds attachable content.

**Files:**
- Create: `ios-native/Jottery/Jottery/Services/PasteboardService.swift`
- Modify: `ios-native/Jottery/Jottery/ViewModels/AppState.swift` (`addAttachment` data overload)
- Modify: `ios-native/Jottery/Jottery/Views/NoteEditorView.swift:201-214` (toolbar)
- Modify: `ios-native/Jottery/Jottery/Extensions/Strings.swift:97-98` (new key), `ios-native/Jottery/Jottery/Localizable.xcstrings`
- Test: `ios-native/Jottery/JotteryTests/PasteboardServiceTests.swift` (create)

**Interfaces:**
- Produces: `AppState.addAttachment(to noteId: String, data: Data, filename: String, mimeType: String) throws` (used again by Task 5's share import); `PasteboardService.hasAttachableContent: Bool`, `PasteboardService.readItems(now: Date) -> [PasteboardService.PastedItem]` where `PastedItem = (data: Data, filename: String, mimeType: String)` as a struct.

- [ ] **Step 1: Write the failing tests**

Create `ios-native/Jottery/JotteryTests/PasteboardServiceTests.swift`:

```swift
import Foundation
import Testing
import UIKit

@testable import Jottery

@MainActor
struct PasteboardServiceTests {

    private func makeTestImage() -> UIImage {
        let renderer = UIGraphicsImageRenderer(size: CGSize(width: 4, height: 4))
        return renderer.image { ctx in
            UIColor.red.setFill()
            ctx.fill(CGRect(x: 0, y: 0, width: 4, height: 4))
        }
    }

    @Test func readsImageFromPasteboard() {
        UIPasteboard.general.items = []
        UIPasteboard.general.image = makeTestImage()

        #expect(PasteboardService.hasAttachableContent)
        let items = PasteboardService.readItems(now: Date(timeIntervalSince1970: 1_751_600_000))
        #expect(items.count == 1)
        let item = try? #require(items.first)
        #expect(item?.mimeType == "image/png")
        #expect(item?.filename.hasPrefix("pasted-") == true)
        #expect(item?.filename.hasSuffix(".png") == true)
        #expect((item?.data.isEmpty) == false)
    }

    @Test func emptyPasteboardHasNoAttachableContent() {
        UIPasteboard.general.items = []
        #expect(!PasteboardService.hasAttachableContent)
        #expect(PasteboardService.readItems().isEmpty)
    }

    @Test func plainTextIsNotAttachable() {
        UIPasteboard.general.items = []
        UIPasteboard.general.string = "just text"
        // Text pastes natively into the editor — the attachment path must not claim it.
        #expect(!PasteboardService.hasAttachableContent)
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: the standard test command.
Expected: FAIL to compile (`PasteboardService` does not exist).

- [ ] **Step 3: Implement PasteboardService**

Create `ios-native/Jottery/Jottery/Services/PasteboardService.swift`:

```swift
import UIKit
import UniformTypeIdentifiers

/// Reads attachable content (images, PDFs) from the general pasteboard.
///
/// Runestone's editor only pastes strings, so images copied from other
/// apps are attached via the editor toolbar instead.
@MainActor
enum PasteboardService {

    struct PastedItem {
        let data: Data
        let filename: String
        let mimeType: String
    }

    /// Whether the pasteboard holds content that can be attached to a note.
    /// Only inspects types — does not read data, so no paste banner is shown.
    static var hasAttachableContent: Bool {
        let pasteboard = UIPasteboard.general
        return pasteboard.hasImages
            || pasteboard.contains(pasteboardTypes: [UTType.pdf.identifier])
    }

    /// Extract attachable items. Reading contents may show the iOS paste banner.
    static func readItems(now: Date = Date()) -> [PastedItem] {
        let pasteboard = UIPasteboard.general
        var items: [PastedItem] = []

        let formatter = DateFormatter()
        formatter.dateFormat = "yyyyMMdd-HHmmss"
        formatter.locale = Locale(identifier: "en_GB_POSIX")
        let stamp = formatter.string(from: now)

        for (index, _) in pasteboard.items.enumerated() {
            let suffix = pasteboard.items.count > 1 ? "-\(index + 1)" : ""

            if let data = pasteboard.data(forPasteboardType: UTType.png.identifier, inItemSet: IndexSet(integer: index))?.first {
                items.append(PastedItem(data: data, filename: "pasted-\(stamp)\(suffix).png", mimeType: "image/png"))
            } else if let data = pasteboard.data(forPasteboardType: UTType.jpeg.identifier, inItemSet: IndexSet(integer: index))?.first {
                items.append(PastedItem(data: data, filename: "pasted-\(stamp)\(suffix).jpg", mimeType: "image/jpeg"))
            } else if let images = pasteboard.images, index < images.count, let data = images[index].pngData() {
                // Image in a non-PNG/JPEG representation — re-encode as PNG
                items.append(PastedItem(data: data, filename: "pasted-\(stamp)\(suffix).png", mimeType: "image/png"))
            } else if let data = pasteboard.data(forPasteboardType: UTType.pdf.identifier, inItemSet: IndexSet(integer: index))?.first {
                items.append(PastedItem(data: data, filename: "pasted-\(stamp)\(suffix).pdf", mimeType: "application/pdf"))
            }
        }
        return items
    }
}
```

- [ ] **Step 4: Add the Data overload to AppState.addAttachment**

In `AppState.swift`, change the existing URL-based function to delegate and add the Data variant. The existing body moves into the new function with `let fileData = try Data(contentsOf: url)` removed (the parameter provides it):

```swift
    func addAttachment(to noteId: String, url: URL, filename: String, mimeType: String) throws {
        try addAttachment(to: noteId, data: try Data(contentsOf: url), filename: filename, mimeType: mimeType)
    }

    func addAttachment(to noteId: String, data fileData: Data, filename: String, mimeType: String) throws {
        guard let noteRepo, let attachmentRepo, let key = keyManager.masterKey else { return }
        guard try noteRepo.get(id: noteId, key: key) != nil else { return }

        // Encrypt the file data
        let encrypted = try CryptoService.encrypt(fileData, key: key)
        ... (existing body unchanged from here, ending with refreshNoteFromStore(id: noteId))
    }
```

- [ ] **Step 5: Add the toolbar action**

In `Strings.swift` after `editorAddPhoto`:
```swift
    static var editorPasteFromClipboard: String { String(localized: "editor.pasteFromClipboard") }
```

In `Localizable.xcstrings`, add to the `"strings"` object (keep alphabetical placement near other `editor.` keys):
```json
    "editor.pasteFromClipboard" : {
      "extractionState" : "manual",
      "localizations" : {
        "en-GB" : {
          "stringUnit" : {
            "state" : "translated",
            "value" : "Paste from Clipboard"
          }
        }
      }
    },
```

In `NoteEditorView.swift`, inside the `if !isReadOnly {` toolbar block after the Add Photo button:

```swift
                        if PasteboardService.hasAttachableContent {
                            Button {
                                for item in PasteboardService.readItems() {
                                    try? appState.addAttachment(
                                        to: note.id,
                                        data: item.data,
                                        filename: item.filename,
                                        mimeType: item.mimeType
                                    )
                                }
                            } label: {
                                Label(L.editorPasteFromClipboard, systemImage: "doc.on.clipboard")
                            }
                        }
```

- [ ] **Step 6: Run tests to verify they pass**

Run: the standard test command.
Expected: all tests PASS.

- [ ] **Step 7: Commit**

```bash
git add ios-native/Jottery/Jottery/Services/PasteboardService.swift ios-native/Jottery/Jottery/ViewModels/AppState.swift ios-native/Jottery/Jottery/Views/NoteEditorView.swift ios-native/Jottery/Jottery/Extensions/Strings.swift ios-native/Jottery/Jottery/Localizable.xcstrings ios-native/Jottery/JotteryTests/PasteboardServiceTests.swift
git commit -m "feat(ios): paste images and PDFs from the clipboard as note attachments"
```

---

### Task 5: SharedInboxStore — app-group hand-off store (jottery-3uwd part 1)

File-based staging area shared between the future extension and the main app. Pure Foundation, fully unit-testable via an injectable root directory.

**Files:**
- Create: `ios-native/Jottery/Shared/SharedInboxStore.swift`
- Modify: `ios-native/Jottery/Jottery.xcodeproj/project.pbxproj` (add `Shared/` synchronized group to the app target; the extension target picks it up in Task 6)
- Test: `ios-native/Jottery/JotteryTests/SharedInboxStoreTests.swift` (create)

**Interfaces:**
- Produces:
  - `SharedInboxStore.appGroupIdentifier == "group.com.jottery.ios"`
  - `SharedInboxStore.Manifest: Codable` with `createdAt: Date`, `text: String?`, `urls: [String]`, `files: [File]`; `File` has `storedName`, `filename`, `mimeType` (all String)
  - `static func write(text: String?, urls: [String], files: [(data: Data, filename: String, mimeType: String)], in root: URL? = nil) throws`
  - `static func pendingItems(in root: URL? = nil) -> [(directory: URL, manifest: Manifest)]`
  - `static func remove(_ directory: URL)`
  - `root == nil` resolves the app-group container; tests pass a temp directory.

- [ ] **Step 1: Write the failing tests**

Create `ios-native/Jottery/JotteryTests/SharedInboxStoreTests.swift`:

```swift
import Foundation
import Testing

@testable import Jottery

struct SharedInboxStoreTests {

    private func makeRoot() throws -> URL {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("shared-inbox-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        return root
    }

    @Test func writeThenReadRoundTrip() throws {
        let root = try makeRoot()
        let imageData = Data([0x89, 0x50, 0x4E, 0x47])

        try SharedInboxStore.write(
            text: "Shared note text",
            urls: ["https://example.com"],
            files: [(data: imageData, filename: "photo.png", mimeType: "image/png")],
            in: root
        )

        let items = SharedInboxStore.pendingItems(in: root)
        #expect(items.count == 1)
        let item = try #require(items.first)
        #expect(item.manifest.text == "Shared note text")
        #expect(item.manifest.urls == ["https://example.com"])
        #expect(item.manifest.files.count == 1)
        let file = try #require(item.manifest.files.first)
        #expect(file.filename == "photo.png")
        #expect(file.mimeType == "image/png")
        let storedData = try Data(contentsOf: item.directory.appendingPathComponent(file.storedName))
        #expect(storedData == imageData)
    }

    @Test func removeDeletesTheItem() throws {
        let root = try makeRoot()
        try SharedInboxStore.write(text: "bye", urls: [], files: [], in: root)
        let item = try #require(SharedInboxStore.pendingItems(in: root).first)

        SharedInboxStore.remove(item.directory)

        #expect(SharedInboxStore.pendingItems(in: root).isEmpty)
    }

    @Test func pendingItemsIgnoresCorruptEntries() throws {
        let root = try makeRoot()
        let junk = root.appendingPathComponent("junk", isDirectory: true)
        try FileManager.default.createDirectory(at: junk, withIntermediateDirectories: true)
        try Data("not json".utf8).write(to: junk.appendingPathComponent("manifest.json"))

        #expect(SharedInboxStore.pendingItems(in: root).isEmpty)
    }
}
```

- [ ] **Step 2: Run tests to verify they fail to compile**

Run: the standard test command. Expected: compile FAILURE (`SharedInboxStore` unknown).

- [ ] **Step 3: Implement SharedInboxStore**

Create `ios-native/Jottery/Shared/SharedInboxStore.swift`:

```swift
import Foundation

/// File-based hand-off between the share extension and the main app.
///
/// The extension never holds the master key, so items are staged
/// unencrypted in the app-group container (guarded by iOS file
/// protection) and converted into encrypted notes when the main app
/// next runs unlocked.
enum SharedInboxStore {

    static let appGroupIdentifier = "group.com.jottery.ios"

    struct Manifest: Codable {
        var createdAt: Date
        var text: String?
        var urls: [String]
        var files: [File]

        struct File: Codable {
            var storedName: String   // on-disk name inside the item directory
            var filename: String     // original filename for the attachment
            var mimeType: String
        }
    }

    /// The staging directory inside the app-group container.
    static func defaultRoot() -> URL? {
        FileManager.default
            .containerURL(forSecurityApplicationGroupIdentifier: appGroupIdentifier)?
            .appendingPathComponent("SharedInbox", isDirectory: true)
    }

    /// Stage one shared item. `root` is injectable for tests.
    static func write(
        text: String?,
        urls: [String],
        files: [(data: Data, filename: String, mimeType: String)],
        in root: URL? = nil
    ) throws {
        guard let root = root ?? defaultRoot() else { return }
        let directory = root.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)

        var manifestFiles: [Manifest.File] = []
        for (index, file) in files.enumerated() {
            let ext = (file.filename as NSString).pathExtension
            let storedName = ext.isEmpty ? "file-\(index)" : "file-\(index).\(ext)"
            try file.data.write(
                to: directory.appendingPathComponent(storedName),
                options: [.atomic, .completeFileProtection]
            )
            manifestFiles.append(Manifest.File(storedName: storedName, filename: file.filename, mimeType: file.mimeType))
        }

        let manifest = Manifest(createdAt: Date(), text: text, urls: urls, files: manifestFiles)
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        try (try encoder.encode(manifest)).write(
            to: directory.appendingPathComponent("manifest.json"),
            options: [.atomic, .completeFileProtection]
        )
    }

    /// All staged items, oldest first. Entries without a readable manifest are skipped.
    static func pendingItems(in root: URL? = nil) -> [(directory: URL, manifest: Manifest)] {
        guard let root = root ?? defaultRoot(),
              let entries = try? FileManager.default.contentsOfDirectory(
                  at: root, includingPropertiesForKeys: nil
              ) else { return [] }

        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return entries
            .compactMap { directory -> (URL, Manifest)? in
                guard let data = try? Data(contentsOf: directory.appendingPathComponent("manifest.json")),
                      let manifest = try? decoder.decode(Manifest.self, from: data) else { return nil }
                return (directory, manifest)
            }
            .sorted { $0.1.createdAt < $1.1.createdAt }
    }

    /// Delete a staged item after import (or on user rejection).
    static func remove(_ directory: URL) {
        try? FileManager.default.removeItem(at: directory)
    }
}
```

- [ ] **Step 4: Add the Shared group to the pbxproj (app target)**

In `/* Begin PBXFileSystemSynchronizedRootGroup section */` add:
```
		F6000001 /* Shared */ = {
			isa = PBXFileSystemSynchronizedRootGroup;
			path = Shared;
			sourceTree = "<group>";
		};
```
Add `F6000001 /* Shared */,` to the root group `A8000001` children and to the app target `A5000001`'s `fileSystemSynchronizedGroups`.

- [ ] **Step 5: Run tests to verify they pass**

Run: the standard test command. Expected: all tests PASS.

- [ ] **Step 6: Commit**

```bash
git add ios-native/Jottery/Shared ios-native/Jottery/JotteryTests/SharedInboxStoreTests.swift ios-native/Jottery/Jottery.xcodeproj/project.pbxproj
git commit -m "feat(ios): add SharedInboxStore app-group staging for share-in items"
```

---

### Task 6: Import staged shared items into notes (jottery-3uwd part 2)

**Files:**
- Modify: `ios-native/Jottery/Jottery/ViewModels/AppState.swift` (new `importSharedInboxItems`, hooks in `unlock` and `handleScenePhaseChange`)
- Modify: `ios-native/Jottery/Jottery/Views/UnlockScreen.swift` (hook in biometric success path)
- Test: extend `ios-native/Jottery/JotteryTests/AppStateAttachmentTests.swift` with a new test struct or add `AppStateSharedImportTests.swift` (create)

**Interfaces:**
- Produces: `AppState.importSharedInboxItems(from root: URL? = nil)` — idempotent, safe to call whenever unlocked.
- Consumes: `SharedInboxStore` (Task 5), `addAttachment(to:data:filename:mimeType:)` (Task 4), `createNote(content:tags:)` (exists).

- [ ] **Step 1: Write the failing test**

Create `ios-native/Jottery/JotteryTests/AppStateSharedImportTests.swift`:

```swift
import CryptoKit
import Foundation
import Testing

@testable import Jottery

@MainActor
struct AppStateSharedImportTests {

    private func makeUnlockedState() throws -> AppState {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-import-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let state = AppState()
        try state.initialise(database: DatabaseManager(path: dir.appendingPathComponent("test.db").path))
        state.keyManager.unlockWithKeyData(Data(repeating: 7, count: 32))
        state.isLocked = false
        return state
    }

    @Test func importsStagedItemAsNoteWithAttachment() throws {
        let state = try makeUnlockedState()
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("import-root-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        try SharedInboxStore.write(
            text: "Shared from Safari",
            urls: ["https://example.com/article"],
            files: [(data: Data([0x89, 0x50, 0x4E, 0x47]), filename: "photo.png", mimeType: "image/png")],
            in: root
        )

        state.importSharedInboxItems(from: root)

        let note = try #require(state.notes.first)
        #expect(note.content.contains("Shared from Safari"))
        #expect(note.content.contains("https://example.com/article"))
        #expect(note.attachments.count == 1)
        #expect(note.attachments.first?.filename == "photo.png")
        // Item is consumed — a second import must not duplicate it.
        state.importSharedInboxItems(from: root)
        #expect(state.notes.count == 1)
    }

    @Test func importSkipsWhileLocked() throws {
        let state = try makeUnlockedState()
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("import-root-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        try SharedInboxStore.write(text: "held", urls: [], files: [], in: root)

        state.isLocked = true
        state.importSharedInboxItems(from: root)

        #expect(state.notes.isEmpty)
        #expect(SharedInboxStore.pendingItems(in: root).count == 1)  // still staged
    }
}
```

- [ ] **Step 2: Run tests to verify they fail to compile**

Run: the standard test command. Expected: compile FAILURE (`importSharedInboxItems` unknown).

- [ ] **Step 3: Implement importSharedInboxItems**

In `AppState.swift`, after the `// MARK: - Inbox` block:

```swift
    // MARK: - Shared Inbox (share-extension hand-off)

    /// Convert items staged by the share extension into encrypted notes.
    /// Safe to call on every unlock/foreground — consumed items are removed.
    func importSharedInboxItems(from root: URL? = nil) {
        guard !isLocked, keyManager.masterKey != nil, noteRepo != nil else { return }

        for item in SharedInboxStore.pendingItems(in: root) {
            var lines: [String] = []
            if let text = item.manifest.text, !text.isEmpty {
                lines.append(text)
            }
            lines.append(contentsOf: item.manifest.urls)
            let content = lines.joined(separator: "\n")

            do {
                guard let note = try createNote(content: content, tags: ["shared"]) else { continue }
                for file in item.manifest.files {
                    let fileURL = item.directory.appendingPathComponent(file.storedName)
                    let data = try Data(contentsOf: fileURL)
                    try addAttachment(to: note.id, data: data, filename: file.filename, mimeType: file.mimeType)
                }
                SharedInboxStore.remove(item.directory)
            } catch {
                // Leave the item staged so a later import can retry.
                print("[SharedInbox] import failed: \(error)")
            }
        }
    }
```

Note: `createNote` sets `selectedNoteId` — imports should not steal selection. Capture and restore around the loop:

```swift
        let previousSelection = selectedNoteId
        defer { selectedNoteId = previousSelection }
```
(placed just before the `for` loop, inside the function after the guard).

- [ ] **Step 4: Hook the import into unlock paths and foregrounding**

In `AppState.unlock(password:)` after `scheduleSearchWarmUp()` (line ~366): add
```swift
        importSharedInboxItems()
```

In `AppState.handleScenePhaseChange`, `.active` case, after `keyManager.recordActivity()`: add
```swift
            importSharedInboxItems()
```

In `UnlockScreen.biometricUnlock()` success branch after `appState.scheduleSearchWarmUp()`: add
```swift
                appState.importSharedInboxItems()
```

- [ ] **Step 5: Run tests to verify they pass**

Run: the standard test command. Expected: all tests PASS.

- [ ] **Step 6: Commit**

```bash
git add ios-native/Jottery/Jottery/ViewModels/AppState.swift ios-native/Jottery/Jottery/Views/UnlockScreen.swift ios-native/Jottery/JotteryTests/AppStateSharedImportTests.swift
git commit -m "feat(ios): import share-extension staged items into encrypted notes"
```

---

### Task 7: JotteryShare extension target (jottery-3uwd part 3)

New app-extension target with a `ShareViewController` that stages shared content via `SharedInboxStore`, plus app-group entitlements for both targets.

**Files:**
- Create: `ios-native/Jottery/JotteryShare/ShareViewController.swift`
- Create: `ios-native/Jottery/JotteryShare-Info.plist` (project-dir level, mirrors existing `Info.plist` placement so the synchronized group stays sources-only)
- Create: `ios-native/Jottery/Jottery.entitlements`, `ios-native/Jottery/JotteryShare.entitlements`
- Modify: `ios-native/Jottery/Jottery.xcodeproj/project.pbxproj`

**Interfaces:**
- Consumes: `SharedInboxStore.write(text:urls:files:in:)` (Task 5).
- Produces: `JotteryShare.appex` embedded in `Jottery.app`; both targets entitled to `group.com.jottery.ios`.

- [ ] **Step 1: Create entitlements files**

`ios-native/Jottery/Jottery.entitlements` and `ios-native/Jottery/JotteryShare.entitlements` (identical content):
```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>com.apple.security.application-groups</key>
	<array>
		<string>group.com.jottery.ios</string>
	</array>
</dict>
</plist>
```

- [ ] **Step 2: Create the extension Info.plist**

`ios-native/Jottery/JotteryShare-Info.plist`:
```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>NSExtension</key>
	<dict>
		<key>NSExtensionAttributes</key>
		<dict>
			<key>NSExtensionActivationRule</key>
			<dict>
				<key>NSExtensionActivationSupportsImageWithMaxCount</key>
				<integer>10</integer>
				<key>NSExtensionActivationSupportsFileWithMaxCount</key>
				<integer>10</integer>
				<key>NSExtensionActivationSupportsText</key>
				<true/>
				<key>NSExtensionActivationSupportsWebURLWithMaxCount</key>
				<integer>1</integer>
			</dict>
		</dict>
		<key>NSExtensionPointIdentifier</key>
		<string>com.apple.share-services</string>
		<key>NSExtensionPrincipalClass</key>
		<string>$(PRODUCT_MODULE_NAME).ShareViewController</string>
	</dict>
</dict>
</plist>
```

- [ ] **Step 3: Implement ShareViewController**

Create `ios-native/Jottery/JotteryShare/ShareViewController.swift`:

```swift
import UIKit
import UniformTypeIdentifiers

/// Receives shared content from the system share sheet and stages it in
/// the app-group container. The main app converts staged items into
/// encrypted notes on its next unlocked run (`AppState.importSharedInboxItems`).
final class ShareViewController: UIViewController {

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .clear

        // Minimal confirmation chrome — the work is quick, so a small
        // translucent card is shown while providers load.
        let card = UIVisualEffectView(effect: UIBlurEffect(style: .systemMaterial))
        card.layer.cornerRadius = 16
        card.clipsToBounds = true
        card.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(card)

        let label = UILabel()
        label.text = String(localized: "share.savingToJottery", defaultValue: "Saving to Jottery…")
        label.font = .preferredFont(forTextStyle: .headline)
        label.translatesAutoresizingMaskIntoConstraints = false
        card.contentView.addSubview(label)

        NSLayoutConstraint.activate([
            card.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            card.centerYAnchor.constraint(equalTo: view.centerYAnchor),
            label.topAnchor.constraint(equalTo: card.contentView.topAnchor, constant: 20),
            label.bottomAnchor.constraint(equalTo: card.contentView.bottomAnchor, constant: -20),
            label.leadingAnchor.constraint(equalTo: card.contentView.leadingAnchor, constant: 24),
            label.trailingAnchor.constraint(equalTo: card.contentView.trailingAnchor, constant: -24),
        ])

        Task { await ingestAndComplete() }
    }

    private func ingestAndComplete() async {
        var text: String?
        var urls: [String] = []
        var files: [(data: Data, filename: String, mimeType: String)] = []

        let providers = (extensionContext?.inputItems ?? [])
            .compactMap { $0 as? NSExtensionItem }
            .flatMap { $0.attachments ?? [] }

        for provider in providers {
            if provider.hasItemConformingToTypeIdentifier(UTType.image.identifier) {
                if let file = await loadFile(from: provider, type: .image, fallbackName: "shared-image") {
                    files.append(file)
                }
            } else if provider.hasItemConformingToTypeIdentifier(UTType.fileURL.identifier) {
                if let file = await loadFile(from: provider, type: .data, fallbackName: "shared-file") {
                    files.append(file)
                }
            } else if provider.hasItemConformingToTypeIdentifier(UTType.url.identifier) {
                if let url = await loadItem(from: provider, type: .url) as? URL {
                    urls.append(url.absoluteString)
                }
            } else if provider.hasItemConformingToTypeIdentifier(UTType.plainText.identifier) {
                if let string = await loadItem(from: provider, type: .plainText) as? String {
                    text = [text, string].compactMap { $0 }.joined(separator: "\n")
                }
            }
        }

        if text != nil || !urls.isEmpty || !files.isEmpty {
            try? SharedInboxStore.write(text: text, urls: urls, files: files)
        }
        extensionContext?.completeRequest(returningItems: nil)
    }

    /// Load a provider as an on-disk file representation and read it.
    private func loadFile(
        from provider: NSItemProvider, type: UTType, fallbackName: String
    ) async -> (data: Data, filename: String, mimeType: String)? {
        await withCheckedContinuation { continuation in
            provider.loadFileRepresentation(forTypeIdentifier: type.identifier) { url, _ in
                guard let url, let data = try? Data(contentsOf: url) else {
                    continuation.resume(returning: nil)
                    return
                }
                let filename = url.lastPathComponent.isEmpty ? fallbackName : url.lastPathComponent
                let mimeType = UTType(filenameExtension: url.pathExtension)?.preferredMIMEType
                    ?? "application/octet-stream"
                continuation.resume(returning: (data, filename, mimeType))
            }
        }
    }

    /// Load a provider's item object (URL or String).
    private func loadItem(from provider: NSItemProvider, type: UTType) async -> (any NSSecureCoding)? {
        await withCheckedContinuation { continuation in
            provider.loadItem(forTypeIdentifier: type.identifier, options: nil) { item, _ in
                continuation.resume(returning: item)
            }
        }
    }
}
```

(Adjust for Swift 6 sendability as needed at compile time — e.g. capture results via `nonisolated(unsafe)` locals or `@Sendable` continuation payloads; the compiler is the arbiter here.)

- [ ] **Step 4: Add the extension target to project.pbxproj**

All new ids `F`-prefixed:

PBXBuildFile section:
```
		F0000001 /* JotteryShare.appex in Embed Foundation Extensions */ = {isa = PBXBuildFile; fileRef = F3000001 /* JotteryShare.appex */; settings = {ATTRIBUTES = (RemoveHeadersOnCopy, ); }; };
```

PBXFileReference section:
```
		F3000001 /* JotteryShare.appex */ = {isa = PBXFileReference; explicitFileType = "wrapper.app-extension"; includeInIndex = 0; path = JotteryShare.appex; sourceTree = BUILT_PRODUCTS_DIR; };
```

PBXFileSystemSynchronizedRootGroup section:
```
		F6000002 /* JotteryShare */ = {
			isa = PBXFileSystemSynchronizedRootGroup;
			path = JotteryShare;
			sourceTree = "<group>";
		};
```

Root group `A8000001` children: add `F6000002 /* JotteryShare */,`. Products group: add `F3000001 /* JotteryShare.appex */,`.

PBXNativeTarget section:
```
		F5000001 /* JotteryShare */ = {
			isa = PBXNativeTarget;
			buildConfigurationList = FA000001 /* Build configuration list for PBXNativeTarget "JotteryShare" */;
			buildPhases = (
				F7000001 /* Sources */,
			);
			buildRules = (
			);
			dependencies = (
			);
			fileSystemSynchronizedGroups = (
				F6000002 /* JotteryShare */,
				F6000001 /* Shared */,
			);
			name = JotteryShare;
			packageProductDependencies = (
			);
			productName = JotteryShare;
			productReference = F3000001 /* JotteryShare.appex */;
			productType = "com.apple.product-type.app-extension";
		};
```

App target `A5000001`: add to `buildPhases` (after Resources) `F1000001 /* Embed Foundation Extensions */,`; add to `dependencies` `F8000001 /* PBXTargetDependency */,`.

New PBXCopyFilesBuildPhase section entry:
```
		F1000001 /* Embed Foundation Extensions */ = {
			isa = PBXCopyFilesBuildPhase;
			buildActionMask = 2147483647;
			dstPath = "";
			dstSubfolderSpec = 13;
			files = (
				F0000001 /* JotteryShare.appex in Embed Foundation Extensions */,
			);
			name = "Embed Foundation Extensions";
			runOnlyForDeploymentPostprocessing = 0;
		};
```

PBXContainerItemProxy + PBXTargetDependency (extension depends on nothing; the APP depends on the extension):
```
		F9000001 /* PBXContainerItemProxy */ = {
			isa = PBXContainerItemProxy;
			containerPortal = AD000001 /* Project object */;
			proxyType = 1;
			remoteGlobalIDString = F5000001;
			remoteInfo = JotteryShare;
		};
		F8000001 /* PBXTargetDependency */ = {
			isa = PBXTargetDependency;
			target = F5000001 /* JotteryShare */;
			targetProxy = F9000001 /* PBXContainerItemProxy */;
		};
```

PBXSourcesBuildPhase:
```
		F7000001 /* Sources */ = {
			isa = PBXSourcesBuildPhase;
			buildActionMask = 2147483647;
			files = (
			);
			runOnlyForDeploymentPostprocessing = 0;
		};
```

XCBuildConfiguration (Debug FB000001 / Release FB000002):
```
		FB000001 /* Debug */ = {
			isa = XCBuildConfiguration;
			buildSettings = {
				CODE_SIGN_ENTITLEMENTS = JotteryShare.entitlements;
				CODE_SIGN_STYLE = Automatic;
				CURRENT_PROJECT_VERSION = 10102;
				DEVELOPMENT_TEAM = QA72EA3LC2;
				GENERATE_INFOPLIST_FILE = YES;
				INFOPLIST_FILE = "JotteryShare-Info.plist";
				INFOPLIST_KEY_CFBundleDisplayName = Jottery;
				LD_RUNPATH_SEARCH_PATHS = (
					"$(inherited)",
					"@executable_path/Frameworks",
					"@executable_path/../../Frameworks",
				);
				MARKETING_VERSION = 1.1.2;
				PRODUCT_BUNDLE_IDENTIFIER = com.jottery.ios.JotteryShare;
				PRODUCT_NAME = "$(TARGET_NAME)";
				SKIP_INSTALL = YES;
				SUPPORTED_PLATFORMS = "iphoneos iphonesimulator";
				SWIFT_EMIT_LOC_STRINGS = YES;
				SWIFT_STRICT_CONCURRENCY = complete;
				SWIFT_VERSION = 6.0;
				TARGETED_DEVICE_FAMILY = "1,2";
			};
			name = Debug;
		};
```
(FB000002 /* Release */ identical.)

XCConfigurationList:
```
		FA000001 /* Build configuration list for PBXNativeTarget "JotteryShare" */ = {
			isa = XCConfigurationList;
			buildConfigurations = (
				FB000001 /* Debug */,
				FB000002 /* Release */,
			);
			defaultConfigurationIsVisible = 0;
			defaultConfigurationName = Release;
		};
```

PBXProject: add `F5000001 /* JotteryShare */,` to `targets`, and to `TargetAttributes`:
```
					F5000001 = {
						CreatedOnToolsVersion = 16.0;
					};
```

App target build configurations `B2000001` and `B3000001`: add
```
				CODE_SIGN_ENTITLEMENTS = Jottery.entitlements;
```

- [ ] **Step 5: Build app + extension for the simulator**

Run: `xcodebuild -project ios-native/Jottery/Jottery.xcodeproj -scheme Jottery -destination 'platform=iOS Simulator,name=iPhone 17' build 2>&1 | tail -5`
Expected: `** BUILD SUCCEEDED **` — the appex builds and embeds. Fix any Swift 6 concurrency errors in ShareViewController as they surface.

- [ ] **Step 6: Run the full test suite**

Run: the standard test command. Expected: all tests PASS.

- [ ] **Step 7: Commit**

```bash
git add ios-native/Jottery/JotteryShare ios-native/Jottery/JotteryShare-Info.plist ios-native/Jottery/Jottery.entitlements ios-native/Jottery/JotteryShare.entitlements ios-native/Jottery/Jottery.xcodeproj/project.pbxproj
git commit -m "feat(ios): add JotteryShare extension so content can be shared into notes (jottery-3uwd)"
```

---

### Task 8: Final verification, issue closure, PR

- [ ] **Step 1: Full clean test run**

Run: `xcodebuild -project ios-native/Jottery/Jottery.xcodeproj -scheme Jottery -destination 'platform=iOS Simulator,name=iPhone 17' test 2>&1 | tail -20`
Expected: `** TEST SUCCEEDED **`.

- [ ] **Step 2: Release-configuration build check**

Run: `xcodebuild -project ios-native/Jottery/Jottery.xcodeproj -scheme Jottery -configuration Release -destination 'platform=iOS Simulator,name=iPhone 17' build 2>&1 | tail -3`
Expected: `** BUILD SUCCEEDED **`.

- [ ] **Step 3: Close beads issues, sync, push, open PR**

```bash
bd close jottery-wkj8 --reason="Re-entrancy guard in KeyManager + once-only auto-attempt in UnlockScreen"
bd close jottery-lm4k --reason="In-memory note re-fetched after attachment changes; clipboard paste action added"
bd close jottery-3uwd --reason="JotteryShare extension + app-group staging + unlock-time import"
bd sync
git push -u origin feature/ios-client-fixes
gh pr create --title "fix(ios): Face ID double prompt, attachment refresh, clipboard paste, share-in extension" --body "<summary + manual test checklist>"
```

- [ ] **Step 4: Report manual-verification checklist to the user**

Device-only checks Claude cannot run: single Face ID prompt on cold launch and re-foreground; share sheet shows Jottery from Photos/Safari; shared item becomes a note on next unlock; paste action attaches a copied image. Also note: first device build needs Xcode to register `group.com.jottery.ios` with the team (automatic signing does this when the project is opened and built once in Xcode).
