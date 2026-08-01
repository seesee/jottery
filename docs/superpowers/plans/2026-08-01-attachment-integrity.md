# Estate-Wide Attachment Integrity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The server warns when pushed notes reference attachment data it does not hold; every client reacts by re-pushing blobs it has (estate self-heal) or logging/surfacing, and no client silently omits blobs at push time any more.

**Architecture:** One optional `attachmentWarnings` field on the push response, computed post-store by a single LEFT JOIN; four client reactions sharing the same semantics (repair-push held blobs once, log the rest, web also feeds Vault Health); TUI's whole-push abort softens to skip-with-warning.

**Tech Stack:** Axum/SQLx (server, runtime queries), TypeScript/vitest/MSW (web), Rust (TUI), Swift/XCTest (iOS), Kotlin/kotlinx.serialization/JUnit (Android).

**Spec:** `docs/superpowers/specs/2026-08-01-attachment-integrity-design.md`
**Beads:** jottery-3jqr (umbrella); children jottery-ytiq (server+web), jottery-39ik (TUI), jottery-v5vv (iOS), jottery-rkpe (Android)

## Global Constraints

- British English in user-facing text and comments.
- Response field name is exactly `attachmentWarnings`, items `{ "noteId": string, "attachmentIds": [string] }`; omitted (not `[]`) when empty.
- Never reject or mutate notes over missing attachment data.
- The guard query failing must not fail the push (log, omit field).
- Repair pushes fire once per sync run, no in-run retry.
- Commit per task with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.
- Delivery: tasks 1–4 on branch `feature/attachment-integrity-server-web` (one PR); task 5 on `feature/attachment-integrity-tui`; task 6 on `feature/attachment-integrity-ios`; task 7 on `feature/attachment-integrity-android` (each branched from main, own PR).

---

### Task 1: server — attachmentWarnings in push response

**Files:**
- Modify: `server/src/models/sync.rs:79-85` (SyncPushResponse), `server/src/api/sync.rs` (push handler, response at ~line 665)
- Test: `server/tests/attachment_and_version_tests.rs`

**Interfaces:**
- Produces: `SyncPushResponse.attachment_warnings: Option<Vec<AttachmentWarning>>` serialised as `attachmentWarnings`, `AttachmentWarning { note_id → noteId, attachment_ids → attachmentIds }`.

- [ ] **Step 1: Write failing tests** — append to `attachment_and_version_tests.rs`, reusing `create_test_app`/`create_test_user_and_device`/`push_note_with_attachment` helpers:

```rust
/// Push a note whose attachment ref has NO blob in the payload.
async fn push_note_with_dangling_ref(app: &mut axum::Router, api_key: &str) -> (String, String) {
    let note_id = uuid::Uuid::new_v4().to_string();
    let attachment_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let request = Request::builder()
        .uri("/api/v1/sync/push")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(json!({
            "notes": [{
                "id": note_id, "content": "Dangling ref note",
                "createdAt": now, "modifiedAt": now,
                "deleted": false, "deletedAt": null,
                "archived": false, "archivedAt": null,
                "tags": [],
                "attachments": [{ "id": attachment_id, "filename": "f.png", "mimeType": "image/png", "size": 9, "data": attachment_id }],
                "pinned": false, "version": 1, "wordWrap": null, "syntaxLanguage": null
            }],
            "attachments": [],
            "versions": []
        }).to_string()))
        .unwrap();
    let response = ServiceExt::<Request<Body>>::ready(&mut *app).await.unwrap().call(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    (note_id, attachment_id)
}

#[tokio::test]
async fn test_push_warns_when_attachment_data_missing() {
    let (mut app, pool) = create_test_app().await;
    let api_key = create_test_user_and_device(&pool).await;
    let (note_id, attachment_id) = push_note_with_dangling_ref(&mut app, &api_key).await;

    // Re-push the same note (still no blob) to read the warning from a response
    // (the first response already has it; use that)
    // -> assert on the FIRST response instead: restructure helper to return body
    // Simplest: repeat the push and inspect this response body.
    let now = chrono::Utc::now().to_rfc3339();
    let request = Request::builder()
        .uri("/api/v1/sync/push").method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(json!({
            "notes": [{
                "id": note_id, "content": "Dangling ref note v2",
                "createdAt": now, "modifiedAt": now,
                "deleted": false, "deletedAt": null, "archived": false, "archivedAt": null,
                "tags": [],
                "attachments": [{ "id": attachment_id, "filename": "f.png", "mimeType": "image/png", "size": 9, "data": attachment_id }],
                "pinned": false, "version": 2, "wordWrap": null, "syntaxLanguage": null
            }],
            "attachments": [], "versions": []
        }).to_string())).unwrap();
    let response = ServiceExt::<Request<Body>>::ready(&mut app).await.unwrap().call(request).await.unwrap();
    let body = parse_json_response(response.into_body()).await;
    let warnings = body["attachmentWarnings"].as_array().expect("warnings present");
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0]["noteId"], note_id);
    assert_eq!(warnings[0]["attachmentIds"].as_array().unwrap(), &vec![serde_json::json!(attachment_id)]);
    pool.close().await;
}

#[tokio::test]
async fn test_push_no_warning_when_data_present() {
    let (mut app, pool) = create_test_app().await;
    let api_key = create_test_user_and_device(&pool).await;
    let _ = push_note_with_attachment(&mut app, &api_key, "blob included").await;
    // push_note_with_attachment asserts 200; verify absence of warnings on a fresh push
    // of a fully-provisioned note by checking the LAST response body — restructure:
    // duplicate the request inline as above but include the blob, then:
    // assert!(body.get("attachmentWarnings").is_none());
    pool.close().await;
}

#[tokio::test]
async fn test_attachments_only_push_clears_warning() {
    let (mut app, pool) = create_test_app().await;
    let api_key = create_test_user_and_device(&pool).await;
    let (note_id, attachment_id) = push_note_with_dangling_ref(&mut app, &api_key).await;

    // Attachments-only repair push
    let blob_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"healed");
    let request = Request::builder()
        .uri("/api/v1/sync/push").method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(json!({
            "notes": [], "attachments": [{ "id": attachment_id, "data": blob_b64 }], "versions": []
        }).to_string())).unwrap();
    let response = ServiceExt::<Request<Body>>::ready(&mut app).await.unwrap().call(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json_response(response.into_body()).await;
    assert!(body.get("attachmentWarnings").is_none() || body["attachmentWarnings"].is_null());

    // Data now stored
    let stored: (i64,) = sqlx::query_as("SELECT length(data) FROM attachments_data WHERE id = ?")
        .bind(&attachment_id).fetch_one(&pool).await.unwrap();
    assert_eq!(stored.0, 6);
    let _ = note_id;
    pool.close().await;
}

#[tokio::test]
async fn test_attachments_only_push_rejects_foreign_attachment() {
    let (mut app, pool) = create_test_app().await;
    let owner_key = create_test_user_and_device(&pool).await;
    let other_key = create_test_user_and_device(&pool).await;
    let (_, attachment_id) = push_note_with_dangling_ref(&mut app, &owner_key).await;

    let blob_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"attack");
    let request = Request::builder()
        .uri("/api/v1/sync/push").method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", other_key))
        .body(Body::from(json!({
            "notes": [], "attachments": [{ "id": attachment_id, "data": blob_b64 }], "versions": []
        }).to_string())).unwrap();
    let response = ServiceExt::<Request<Body>>::ready(&mut app).await.unwrap().call(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK); // push succeeds, blob silently not attached

    let stored: Option<(i64,)> = sqlx::query_as("SELECT length(data) FROM attachments_data WHERE id = ?")
        .bind(&attachment_id).fetch_optional(&pool).await.unwrap();
    assert!(stored.is_none(), "foreign blob must not be stored");
    pool.close().await;
}
```

(While writing, flatten the two "restructure" comments into real inline requests — the sketches above show intent; the final test file must contain complete request bodies, no comments deferring work.)

- [ ] **Step 2: Run** — `cd server && cargo test --test attachment_and_version_tests` → FAIL (no `attachmentWarnings`, foreign blob currently stored).

- [ ] **Step 3: Implement** — in `server/src/models/sync.rs`:

```rust
#[derive(Debug, Serialize)]
pub struct AttachmentWarning {
    #[serde(rename = "noteId")]
    pub note_id: String,
    #[serde(rename = "attachmentIds")]
    pub attachment_ids: Vec<String>,
}
```

Add to `SyncPushResponse`:

```rust
#[serde(rename = "attachmentWarnings", skip_serializing_if = "Option::is_none")]
pub attachment_warnings: Option<Vec<AttachmentWarning>>,
```

In `server/src/api/sync.rs` push handler:

(a) **Ownership guard on the data upsert** (existing loop over `push_req.attachments`): before the `INSERT INTO attachments_data`, verify ownership with a runtime query; skip (with `tracing::warn!`) when the meta row is missing or belongs to another user:

```rust
let owned: Option<(String,)> = sqlx::query_as(
    "SELECT id FROM attachments_meta WHERE id = ? AND note_user_id = ?",
)
.bind(&attachment.id)
.bind(&client_info.user_id)
.fetch_optional(&state.pool)
.await?;
if owned.is_none() {
    tracing::warn!(
        "Skipping attachment data {} from client {}: no matching metadata for user {}",
        attachment.id, client_info.client_id, client_info.user_id
    );
    continue;
}
```

NOTE: the FK is (note_id, note_user_id) → notes, and SQLite enforcement may be off; the explicit check is the guard. Check whether the existing loop already errors on missing meta via FK — keep the explicit check regardless, it is the cross-user protection.

(b) **Warning computation**, after all notes and attachments are stored, before building the response (guard failure must not fail the push):

```rust
// Warn about accepted notes whose attachment refs have no stored data.
// Never fails the push: on query error, log and omit the field.
let attachment_warnings: Option<Vec<crate::models::AttachmentWarning>> = if accepted.is_empty() {
    None
} else {
    let ids: Vec<&str> = accepted.iter().map(|a| a.id.as_str()).collect();
    let mut qb: sqlx::QueryBuilder<'_, sqlx::Sqlite> = sqlx::QueryBuilder::new(
        "SELECT m.note_id, m.id FROM attachments_meta m \
         LEFT JOIN attachments_data d ON d.id = m.id \
         WHERE d.id IS NULL AND m.note_user_id = ",
    );
    qb.push_bind(&client_info.user_id);
    qb.push(" AND m.note_id IN (");
    let mut sep = qb.separated(", ");
    for id in &ids { sep.push_bind(*id); }
    sep.push_unseparated(")");
    match qb.build_query_as::<(String, String)>().fetch_all(&state.pool).await {
        Ok(rows) if !rows.is_empty() => {
            let mut by_note: std::collections::BTreeMap<String, Vec<String>> = Default::default();
            for (note_id, att_id) in rows {
                tracing::warn!(
                    "Note {} (user {}) references attachment {} with no stored data",
                    note_id, client_info.user_id, att_id
                );
                by_note.entry(note_id).or_default().push(att_id);
            }
            Some(by_note.into_iter()
                .map(|(note_id, attachment_ids)| crate::models::AttachmentWarning { note_id, attachment_ids })
                .collect())
        }
        Ok(_) => None,
        Err(e) => { tracing::error!("attachment warning query failed: {}", e); None }
    }
};
```

Add `attachment_warnings` to the `SyncPushResponse { ... }` construction.

- [ ] **Step 4: Run** — `cargo test --test attachment_and_version_tests` → PASS; full `cargo test` → PASS.
- [ ] **Step 5: Commit** — `feat(server): warn when pushed notes reference attachment data the server lacks`

---

### Task 2: web — types + reaction in syncService

**Files:**
- Modify: `src/lib/types/sync.ts:108-112`, `src/lib/services/syncService.ts` (`processPushBatch` ~588, `push` ~660, `collectAttachmentsForBatch` ~468)
- Test: `src/lib/services/syncService.test.ts`

**Interfaces:**
- Consumes: Task 1's response field.
- Produces: `AttachmentWarning { noteId: string; attachmentIds: string[] }` in `src/lib/types/sync.ts`; `syncService` fires at most one attachments-only repair push per sync run; `reportServerMissingAttachments(warnings: AttachmentWarning[])` on vaultHealthService (Task 3).

- [ ] **Step 1: Failing tests** in `syncService.test.ts` (MSW pattern from the known-attachment-IDs test):
  - *held blob*: create a note with attachment ref via `noteService.createNote` + `attachmentRepository.storeBlob('att-held', …)`, mark it needing sync, MSW push handler responds `{accepted: [...], rejected: [], attachmentWarnings: [{noteId, attachmentIds: ['att-held']}]}` for the FIRST push and records subsequent push bodies. Assert a second push request arrives containing `attachments: [{id: 'att-held', data: <base64 of the stored blob>}]` and `notes: []`.
  - *not held*: warning names `att-ghost` (no local blob); assert NO follow-up push occurs and (after Task 3) `vaultHealthService` recorded it.
  - *at most once*: warning repeated in the follow-up response must not trigger a third push.

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement**:
  - `src/lib/types/sync.ts`: `export interface AttachmentWarning { noteId: string; attachmentIds: string[]; }` and `attachmentWarnings?: AttachmentWarning[];` on `SyncPushResponse`.
  - `processPushBatch` returns collected warnings: `return { accepted: …, rejected: …, warnings: result.attachmentWarnings ?? [] }`.
  - In `push()`, accumulate warnings across batches; after the batch loop:

```typescript
if (allWarnings.length > 0) {
  await this.handleAttachmentWarnings(endpoint, apiKey, allWarnings);
}
```

  - New private method:

```typescript
/** React to server attachment warnings: re-push blobs we hold (once, no
 *  in-run retry), surface the rest in vault health. */
private async handleAttachmentWarnings(
  endpoint: string,
  apiKey: string,
  warnings: AttachmentWarning[]
): Promise<void> {
  const repairAttachments: SyncAttachmentData[] = [];
  const unhealable: AttachmentWarning[] = [];
  for (const warning of warnings) {
    const missingHere: string[] = [];
    for (const attachmentId of warning.attachmentIds) {
      const blob = await attachmentRepository.getBlob(attachmentId);
      if (blob) {
        repairAttachments.push({ id: attachmentId, data: arrayBufferToBase64(blob) });
        console.warn(`[SyncService] Server missing attachment ${attachmentId} (note ${warning.noteId}) — re-uploading`);
      } else {
        missingHere.push(attachmentId);
        console.warn(`[SyncService] Server missing attachment ${attachmentId} (note ${warning.noteId}) and it is not on this device`);
      }
    }
    if (missingHere.length > 0) {
      unhealable.push({ noteId: warning.noteId, attachmentIds: missingHere });
    }
  }
  if (repairAttachments.length > 0) {
    try {
      await pushToServer(endpoint, apiKey, { notes: [], attachments: repairAttachments, versions: [] });
      console.log(`[SyncService] Re-uploaded ${repairAttachments.length} attachment(s) the server was missing`);
    } catch (error) {
      console.error('[SyncService] Attachment repair push failed:', error);
    }
  }
  if (unhealable.length > 0) {
    reportServerMissingAttachments(unhealable);
  }
}
```

  Note: the repair push response is NOT fed back into `handleAttachmentWarnings` (prevents loops). Check `SyncPushRequest` type for whether `versions` is optional; match its shape.
  - `collectAttachmentsForBatch`: replace the silent `if (blob)` skip:

```typescript
const blob = await attachmentRepository.getBlob(attachment.data);
if (blob) {
  attachmentMap.set(attachment.id, arrayBufferToBase64(blob));
} else {
  console.warn(`[SyncService] Attachment ${attachment.id} (note ${note.id}) has no local blob — pushing note without it; the server will warn if it has no copy either`);
}
```

- [ ] **Step 4: Run** — `npx vitest run src/lib/services/syncService.test.ts` → PASS.
- [ ] **Step 5: Commit** — `feat(web): react to server attachment warnings with repair push`

---

### Task 3: web — Vault Health surfacing of server-missing attachments

**Files:**
- Modify: `src/lib/services/vaultHealthService.ts`, `src/lib/components/settings/VaultHealthPanel.svelte`, `src/locales/*.json`
- Test: `src/lib/services/vaultHealthService.test.ts`

**Interfaces:**
- Produces: `serverMissingAttachments: Writable<AttachmentWarning[]>` and `reportServerMissingAttachments(warnings)` (deduplicating by attachment id, cleared by `resetVaultHealth`).

- [ ] **Step 1: Failing tests** — record two warnings sharing an attachment id → store holds one entry per attachment; reset clears.
- [ ] **Step 2: Run to verify failure.**
- [ ] **Step 3: Implement** — writable store + report/reset wiring in `vaultHealthService.ts`; panel gains a third list ("Missing on server") rendering note link (reuse `onOpenNote`) + attachment id + hint text `vaultHealth.serverMissingHint`: "The server has no copy of this attachment either. Open the note and remove the attachment, or push from a device that still has the file." New i18n keys `vaultHealth.serverMissingTitle` / `serverMissingHint` translated in all 15 locales (same mechanism as before).
- [ ] **Step 4: Run** — vitest all + svelte-check → green.
- [ ] **Step 5: Commit** — `feat(web): surface server-missing attachments in vault health`

---

### Task 4: server+web PR

- [ ] Full gates: `npx vitest run`, `npx svelte-check`, `cd server && cargo test`, e2e smoke+settings.
- [ ] `bd close jottery-ytiq`; `bd export -o .beads/issues.jsonl && git add … && git commit` in ONE Bash invocation (hook clobbers between commands).
- [ ] Push branch, open PR titled `feat: attachment integrity — server accept+warn guard and web self-heal`, body links spec + umbrella bead. Watch CI, merge when green (standing instruction).

---

### Task 5: TUI — skip-with-warning + reaction (branch `feature/attachment-integrity-tui` off updated main)

**Files:**
- Modify: `tui/src/models/sync.rs:150-156`, `tui/src/ui/operations/sync.rs` (~295 blob gather; ~359 response handling)
- Test: unit test module in `tui/src/models/sync.rs` (deserialisation) — the blob-gather change is exercised by `tui/tests/integration_test.rs` if it covers sync; otherwise compile + `cargo test` in `tui/`.

**Interfaces:**
- Consumes: `attachmentWarnings` field.
- Produces: `SyncPushResponse.attachment_warnings: Vec<AttachmentWarning>` (serde default), `AttachmentWarning { note_id, attachment_ids }` camelCase-renamed.

- [ ] **Step 1: Failing test** — in `tui/src/models/sync.rs` tests module:

```rust
#[test]
fn push_response_decodes_with_and_without_attachment_warnings() {
    let with: SyncPushResponse = serde_json::from_str(
        r#"{"accepted":[],"rejected":[],"errors":[],"attachmentWarnings":[{"noteId":"n1","attachmentIds":["a1"]}]}"#,
    ).unwrap();
    assert_eq!(with.attachment_warnings.len(), 1);
    assert_eq!(with.attachment_warnings[0].attachment_ids, vec!["a1"]);

    let without: SyncPushResponse = serde_json::from_str(
        r#"{"accepted":[],"rejected":[],"errors":[]}"#,
    ).unwrap();
    assert!(without.attachment_warnings.is_empty());
}
```

- [ ] **Step 2: Run** — `cd tui && cargo test push_response_decodes` → FAIL.
- [ ] **Step 3: Implement**:

```rust
/// Attachment reference the server has no data for (vault integrity)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentWarning {
    pub note_id: String,
    pub attachment_ids: Vec<String>,
}
```

`SyncPushResponse` gains `#[serde(rename = "attachmentWarnings", default)] pub attachment_warnings: Vec<AttachmentWarning>,`.

In `ui/operations/sync.rs`: (a) blob gather — replace `.context("Attachment {} not found")?` abort with filter_map + `log::warn!`/eprintln pattern used elsewhere in the file (check how the TUI logs; use the same mechanism):

```rust
let sync_attachments: Vec<SyncAttachment> = attachment_ids_to_push.iter().filter_map(|att_id| {
    match attachment_repo.get(att_id, key) {
        Ok(Some((_f, _m, _s, encrypted_data))) => {
            let encrypted_blob = app.crypto.encrypt_binary(&encrypted_data, key).ok()?;
            let data = serde_json::to_vec(&encrypted_blob).ok()?;
            Some(SyncAttachment { id: att_id.clone(), data: general_purpose::STANDARD.encode(data) })
        }
        _ => {
            // Missing blob no longer aborts the push: the note syncs without it
            // and the server warns; another device holding the blob can heal it
            log::warn!("Attachment {} has no local blob — pushing without it", att_id);
            None
        }
    }
}).collect();
```

(Confirm the TUI's logging facility — grep `log::warn!` in tui/src; if it uses a custom logger/status line, match it.) (b) response handling after `push_response.rejected` loop:

```rust
for warning in &push_response.attachment_warnings {
    for att_id in &warning.attachment_ids {
        match attachment_repo.get(att_id, key) {
            Ok(Some((_f, _m, _s, encrypted_data))) => { /* collect into repair_attachments */ }
            _ => log::warn!(
                "Server has no data for attachment {} (note {}) and neither does this device",
                att_id, warning.note_id
            ),
        }
    }
}
// if repair_attachments non-empty: one attachments-only push, errors logged not fatal
```

Full repair-push block mirrors the existing push request construction with `notes: vec![]`.

- [ ] **Step 4: Run** — `cd tui && cargo test` → PASS; `cargo clippy --no-deps 2>&1 | tail` clean-ish (no new warnings).
- [ ] **Step 5: Commit + PR** — `feat(tui): skip missing blobs with a warning and heal server-missing attachments`; close jottery-39ik with the beads export in the same invocation; PR, CI, merge on green.

---

### Task 6: iOS — push-skip logging + reaction (branch `feature/attachment-integrity-ios`)

**Files:**
- Modify: `ios-native/Jottery/Jottery/Models/SyncModels.swift:35-39`, `ios-native/Jottery/Jottery/Services/SyncService.swift` (~150 gather, ~198 response handling)
- Test: `ios-native/Jottery/JotteryTests/` new `SyncAttachmentWarningTests.swift`

**Interfaces:**
- Produces: `struct AttachmentWarning: Codable { let noteId: String; let attachmentIds: [String] }`; `SyncPushResponse.attachmentWarnings: [AttachmentWarning]?`; `SyncService.collectRepairAttachments(for:) -> [SyncAttachment]` (internal, testable).

- [ ] **Step 1: Failing tests** — decode with/without the field; `collectRepairAttachments` returns SyncAttachment for a blob the repo holds and skips (logging) one it does not. Follow existing JotteryTests patterns (see `AppStateAttachmentTests.swift` for repo setup/mocking conventions; read it first and mirror).
- [ ] **Step 2: Run** — per memory `project-ios-build-test.md` invocation (`xcodebuild test` with the JotteryTests target) → new tests FAIL.
- [ ] **Step 3: Implement** — model fields; in `push()`: replace `if let blobData = try? attachmentRepo.getBlob(id: ref.data)` with do/catch that logs `Log.warn`-equivalent (grep `Log.` for available levels) on miss and continues; after response handling, call the reaction:

```swift
if let warnings = response.attachmentWarnings, !warnings.isEmpty {
    let repairs = collectRepairAttachments(for: warnings)
    if !repairs.isEmpty {
        let repairRequest = SyncPushRequest(notes: [], attachments: repairs, versions: [], deletions: nil, savedSearches: nil)
        do { _ = try await syncClient.push(repairRequest) }
        catch { Log.debug("[Sync] attachment repair push failed — \(error)") }
    }
}
```

`collectRepairAttachments` mirrors the gather loop (held → SyncAttachment(id:, data: base64); missing → log). Match `SyncPushRequest`'s actual initialiser signature.

- [ ] **Step 4: Run** — full JotteryTests → PASS.
- [ ] **Step 5: Commit + PR** — `feat(ios): log skipped blobs and heal server-missing attachments`; close jottery-v5vv; PR, CI, merge on green.

---

### Task 7: Android — push-skip logging + reaction (branch `feature/attachment-integrity-android`)

**Files:**
- Modify: `android-native/app/src/main/java/com/jottery/android/model/SyncModels.kt:62-66`, `android-native/app/src/main/java/com/jottery/android/service/SyncService.kt` (~161 gather + response handling)
- Test: `android-native/app/src/test/java/com/jottery/android/service/` new `AttachmentWarningTest.kt` (mirror `SearchServiceTest.kt` setup)

**Interfaces:**
- Produces: `@Serializable data class AttachmentWarning(val noteId: String, val attachmentIds: List<String>)`; `SyncPushResponse.attachmentWarnings: List<AttachmentWarning>? = null`; `SyncService.collectRepairAttachments(warnings): List<SyncAttachment>` (internal/testable).

- [ ] **Step 1: Failing tests** — kotlinx decode with/without field (`Json { ignoreUnknownKeys = true }`); `collectRepairAttachments` held/missing paths with a fake `AttachmentRepository`.
- [ ] **Step 2: Run** — `cd android-native && ./gradlew testDebugUnitTest --tests '*AttachmentWarning*'` → FAIL.
- [ ] **Step 3: Implement** — model change; gather loop: `?: continue` becomes `?: run { Log.w(TAG, "Attachment ${ref.id} (note ${note.id}) has no local blob — pushing note without it"); continue... }` (Kotlin: use `if (blob == null) { Log.w(...); continue }` inside the loop — `continue` in `run{}` is invalid); response handling calls `collectRepairAttachments` and fires one repair push in a try/catch that logs failures. Match how `SyncService.kt` names its logger/TAG.
- [ ] **Step 4: Run** — `./gradlew testDebugUnitTest` → PASS.
- [ ] **Step 5: Commit + PR** — `feat(android): log skipped blobs and heal server-missing attachments`; close jottery-rkpe; PR, CI, merge on green.

---

### Task 8: close-out

- [ ] After all four PRs merged: `bd close jottery-3jqr` (umbrella) + beads export/commit on a final chore branch or the last PR.
- [ ] Report deployment note: server guard activates on next release tag + redeploy (Chris deploys).
