# iOS Broken-Flows Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the two broken interaction flows from the August review: archived notes cannot be opened (jottery-1rqp), and tag edits made while previewing a markdown note are silently lost on note switch (jottery-6kbk).

**Architecture:** Two surgical view/model fixes: MainView's detail lookup gains archive awareness; NoteEditorView's tag/save wiring becomes branch-independent and flushes on note switch. Model-level behaviour pinned by unit tests; the pure view wiring verified by build + targeted reasoning (no UI-test target exists).

**Tech Stack:** Swift 6, SwiftUI, Swift Testing.

## Global Constraints

- Branch from current `main` AFTER PR #98 merges: `fix/ios-broken-flows`.
- TDD where a testable seam exists; where the defect is pure SwiftUI wiring, the test pins the nearest model-level contract and the report must say exactly what remains manually verified.
- Test command (from `ios-native/Jottery`): `xcodebuild -project Jottery.xcodeproj -scheme Jottery -destination 'platform=iOS Simulator,name=iPhone 17' test`
- No `project.pbxproj` changes. British English comments.

---

### Task 1: Archived notes open in the editor (read-only, as designed)

**Files:**
- Modify: `ios-native/Jottery/Jottery/App/JotteryApp.swift` (`MainView.body` detail lookup, ~:160-183)
- Test: `ios-native/Jottery/JotteryTests/ArchivedNoteLookupTests.swift`

**Interfaces:**
- Produces: `AppState.displayedNote(id: String) -> DecryptedNote?` — resolves from `notes` first, then `archivedNotes`. MainView uses it instead of `notes.first(where:)`.
- The editor already renders archived notes read-only via its `isReadOnly`/archived banner logic — verify that (`NoteEditorView` reads `note.archived`); if the banner logic keys off something else, adapt minimally and note it.

- [ ] **Step 1: Failing test** (`ArchivedNoteLookupTests.swift`, AppState temp-DB harness from `AppStateListRefreshTests.makeUnlockedState`):

```swift
@Test func displayedNoteResolvesArchivedNotes() throws {
    let state = try makeUnlockedState()
    let note = try #require(try state.createNote(content: "to archive"))
    try state.archiveNote(id: note.id)

    #expect(state.notes.first(where: { $0.id == note.id }) == nil)  // pins the trigger
    let resolved = try #require(state.displayedNote(id: note.id))
    #expect(resolved.content == "to archive")
    #expect(resolved.archived == true)
}

@Test func displayedNoteResolvesActiveNotes() throws {
    let state = try makeUnlockedState()
    let note = try #require(try state.createNote(content: "active"))
    #expect(state.displayedNote(id: note.id)?.content == "active")
}
```

- [ ] **Step 2: Verify failure** — no member `displayedNote`.
- [ ] **Step 3: Implement** — in `AppState`:

```swift
/// Resolve a note id for the detail pane, whichever list it lives in.
/// Archived notes are absent from `notes` (archiveNote removes them), so a
/// lookup limited to `notes` made archived rows unopenable (jottery-1rqp).
func displayedNote(id: String) -> DecryptedNote? {
    notes.first(where: { $0.id == id }) ?? archivedNotes.first(where: { $0.id == id })
}
```

In `MainView.body`, replace `appState.notes.first(where: { $0.id == noteId })` with `appState.displayedNote(id: noteId)`.

- [ ] **Step 4: Tests pass; full suite; confirm in the diff that the editor's read-only guard covers archived notes (cite the line in the report).**
- [ ] **Step 5: Commit** — `fix(ios): archived notes open from the archive list`

---

### Task 2: Tag edits persist regardless of editor mode and note switches

**Files:**
- Modify: `ios-native/Jottery/Jottery/Views/NoteEditorView.swift`
- Test: extends `ios-native/Jottery/JotteryTests/PendingEditorFlushTests.swift` only if a model seam is touched; otherwise document the wiring change + build evidence.

**Interfaces / behaviour contract:**
- `.onChange(of: content)` and `.onChange(of: tags)` → `scheduleSave()` must be attached ONCE, at a level that covers every branch (preview, calc, outliner, Runestone) — move them from the per-branch attachment points onto the surrounding container (the `Group`/`VStack` wrapping the branch `if`), and remove the duplicated per-branch copies.
- Note-switch flush: locate the existing `.onChange(of: note.id)` state-reset handler in `NoteEditorView` (the code that reseeds `content`/`tags` when the same view instance is reused for a different note — referenced by the comment at `AppState.swift:844`). Before it reseeds state from the new note, it must call `saveImmediately()`-equivalent logic for the OUTGOING note. Careful: after the reset, `note` already refers to the incoming note, so the flush must capture the outgoing values — restructure as: on change of `note.id`, first build the pending update from the previous state (the old `content`/`tags` @State values still present) against the OLD note id/record, save it, then reseed. If the handler receives `(oldValue, newValue)`, use the old id; the previous note's record can be fetched via `appState.notes/archivedNotes`. If `hasChanges` guards make this awkward (it compares against the NEW `note` after SwiftUI updated the property), compute change-ness against the old note explicitly.
- Guard rails: no save for read-only (locked/archived) notes; no save when nothing changed.

- [ ] **Step 1:** Read the current handler and write, in the report, the exact before/after sequence for: preview-mode tag edit → tap another note. Then make the change.
- [ ] **Step 2:** Build; run the full suite (no regressions).
- [ ] **Step 3:** Manual-verification note in the report: what a hand test on the simulator would do (edit tags in preview on note A, tap note B, reselect A, tags persisted). If you can drive this with the demo-seed harness + simctl screenshot to *show* the tag persisted in the list row (tags render in rows), do it and attach the verdict; if not practical, say so explicitly.
- [ ] **Step 4: Commit** — `fix(ios): tag edits persist in preview mode and across note switches`
