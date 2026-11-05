//! Checks at the sync boundary.
//!
//! The server stores ciphertext and isn't trusted, so everything it sends is checked
//! when it arrives. What fails is dropped and the rest of the sync carries on: this
//! device's copy stays as it was, which can be put right, whereas taking in a bad
//! entry can't be undone.

use serde::de::{Deserialize, DeserializeOwned, Deserializer};

use super::sync::{AttachmentRef, InboxItem, SyncNote, SyncNoteVersion};
use crate::crypto::MAX_HASH_CHAIN_LENGTH;

/// Longer than any field a client writes, so only a hostile value reaches it
const MAX_FIELD_LENGTH: usize = 256;
/// A base64 SHA-256 is 44 characters and a hex one 64
const MAX_HASH_LENGTH: usize = 128;
/// Well past any client's own limit
const MAX_ATTACHMENT_BYTES: i64 = 1024 * 1024 * 1024;

/// A list read one entry at a time, so that an entry that won't parse costs that entry
/// and not the whole response.
#[derive(Debug, Clone)]
pub struct Lenient<T> {
    pub items: Vec<T>,
    pub dropped: usize,
}

impl<T> Default for Lenient<T> {
    fn default() -> Self {
        Self { items: Vec::new(), dropped: 0 }
    }
}

impl<'de, T: DeserializeOwned> Deserialize<'de> for Lenient<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut list = Self::default();

        // A null is as good as an empty list. Anything else that isn't a list fails.
        for value in Option::<Vec<serde_json::Value>>::deserialize(deserializer)?.unwrap_or_default() {
            match serde_json::from_value(value) {
                Ok(item) => list.items.push(item),
                Err(e) => {
                    tracing::warn!("Dropping an entry from the server that doesn't parse: {e}");
                    list.dropped += 1;
                }
            }
        }

        Ok(list)
    }
}

/// Keep the entries that pass `check`, counting the rest.
pub fn keep_valid<T>(items: Vec<T>, check: impl Fn(&T) -> bool, dropped: &mut usize) -> Vec<T> {
    let before = items.len();
    let kept: Vec<T> = items.into_iter().filter(check).collect();
    *dropped += before - kept.len();
    kept
}

/// Whether this is a UUID in the usual hyphenated form, in either case. iOS writes
/// capitals and the others don't, so ids are compared as they were sent.
pub fn is_uuid(value: &str) -> bool {
    value.len() == 36 && uuid::Uuid::try_parse(value).is_ok()
}

/// Whether this could be a mime type. Empty is allowed: a browser that can't tell a
/// file's type leaves it empty.
fn is_mime_type(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    if value.len() > MAX_FIELD_LENGTH {
        return false;
    }

    let essence = value.split(';').next().unwrap_or_default().trim();
    let is_token = |part: &str| {
        !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric() || "!#$&^_.+-".contains(c))
    };
    let parameters_are_plain = value.chars().all(|c| c.is_ascii() && !c.is_ascii_control());

    parameters_are_plain && essence.split_once('/').is_some_and(|(kind, subtype)| is_token(kind) && is_token(subtype))
}

fn is_hash(value: &Option<String>) -> bool {
    value.as_ref().is_none_or(|hash| hash.len() <= MAX_HASH_LENGTH)
}

fn is_short(value: &Option<String>) -> bool {
    value.as_ref().is_none_or(|text| text.len() <= MAX_FIELD_LENGTH)
}

impl AttachmentRef {
    pub fn is_valid(&self) -> bool {
        is_uuid(&self.id)
            && self.data.len() <= MAX_FIELD_LENGTH
            && (0..=MAX_ATTACHMENT_BYTES).contains(&self.size)
            && is_mime_type(&self.mime_type)
    }
}

impl SyncNote {
    pub fn is_valid(&self) -> bool {
        is_uuid(&self.id)
            && self.attachments.iter().all(AttachmentRef::is_valid)
            && is_short(&self.syntax_language)
            && is_short(&self.color)
            && is_hash(&self.content_hash)
            && is_hash(&self.parent_hash)
    }

    /// Keep only what a hash chain can hold.
    pub fn sanitised(mut self) -> Self {
        if let Some(chain) = self.hash_chain.as_mut() {
            chain.retain(|hash| hash.len() <= MAX_HASH_LENGTH);
            chain.truncate(MAX_HASH_CHAIN_LENGTH);
        }
        self
    }
}

impl SyncNoteVersion {
    pub fn is_valid(&self) -> bool {
        is_uuid(&self.note_id)
            && self.version_key.starts_with(&self.note_id)
            && self.version_key.len() <= MAX_FIELD_LENGTH
            && self.reason.len() <= MAX_FIELD_LENGTH
            && self.attachments.iter().all(AttachmentRef::is_valid)
            && is_short(&self.syntax_language)
            && is_short(&self.color)
            && is_hash(&self.content_hash)
            && is_hash(&self.parent_hash)
    }
}

impl InboxItem {
    /// An inbox id goes into a URL when the item is deleted, so it has to be an id.
    pub fn is_valid(&self) -> bool {
        is_uuid(&self.id) && self.created_at.parse::<chrono::DateTime<chrono::Utc>>().is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::sync::SyncPullResponse;
    use serde_json::json;

    const NOTE_ID: &str = "6f1c3a52-0d1e-4b8a-9c3e-2f6a1c0e7a11";
    const ATTACHMENT_ID: &str = "0b9e4d6c-7a21-4f3e-8c55-1d2e3f4a5b6c";

    fn note() -> serde_json::Value {
        json!({
            "id": NOTE_ID,
            "createdAt": "2026-01-01T00:00:00Z",
            "modifiedAt": "2026-01-01T00:00:00Z",
            "content": "{}",
            "tags": [],
            "attachments": [],
            "pinned": false,
            "archived": false,
            "locked": false,
            "deleted": false,
            "version": 1,
        })
    }

    fn pull(body: serde_json::Value) -> SyncPullResponse {
        let mut response = json!({
            "notes": [], "deletions": [], "attachments": [], "versions": [],
            "syncedAt": "2026-01-01T00:00:00Z", "hasMore": false,
        });
        for (key, value) in body.as_object().unwrap() {
            response[key] = value.clone();
        }
        SyncPullResponse::parse(&response.to_string()).unwrap()
    }

    #[test]
    fn a_uuid_is_accepted_in_either_case_and_nothing_else_is() {
        assert!(is_uuid(NOTE_ID));
        assert!(is_uuid(&NOTE_ID.to_uppercase()));

        assert!(!is_uuid(""));
        assert!(!is_uuid("note-1"));
        assert!(!is_uuid("../../sync/device"));
        assert!(!is_uuid(&NOTE_ID.replace('-', "")));
        assert!(!is_uuid(&format!("{{{NOTE_ID}}}")));
        assert!(!is_uuid(&format!("urn:uuid:{NOTE_ID}")));
    }

    #[test]
    fn a_mime_type_has_to_look_like_one() {
        assert!(is_mime_type(""));
        assert!(is_mime_type("text/plain"));
        assert!(is_mime_type("text/plain; charset=utf-8"));
        assert!(is_mime_type("application/vnd.openxmlformats-officedocument.wordprocessingml.document"));

        assert!(!is_mime_type("text"));
        assert!(!is_mime_type("text/"));
        assert!(!is_mime_type("text/plain\u{1b}[31m"));
        assert!(!is_mime_type("text/plain\nX-Injected: 1"));
        assert!(!is_mime_type(&format!("text/{}", "a".repeat(300))));
    }

    #[test]
    fn a_note_with_a_bad_id_is_dropped_and_its_neighbours_kept() {
        let mut bad = note();
        bad["id"] = json!("../../etc/passwd");

        let response = pull(json!({ "notes": [bad, note()] }));

        assert_eq!(response.notes.len(), 1);
        assert_eq!(response.notes[0].id, NOTE_ID);
        assert_eq!(response.dropped, 1);
    }

    /// This used to fail the whole pull, and with it every other change on the page.
    #[test]
    fn a_note_that_will_not_parse_is_dropped_and_its_neighbours_kept() {
        let mut bad_timestamp = note();
        bad_timestamp["modifiedAt"] = json!("the day before yesterday");
        let mut wrong_type = note();
        wrong_type["pinned"] = json!("very");
        let mut missing_field = note();
        missing_field.as_object_mut().unwrap().remove("content");

        let response = pull(json!({ "notes": [bad_timestamp, note(), wrong_type, missing_field, 42] }));

        assert_eq!(response.notes.len(), 1);
        assert_eq!(response.dropped, 4);
    }

    #[test]
    fn a_bad_attachment_reference_drops_the_note_rather_than_part_of_it() {
        let reference = |id: &str, mime: &str, size: i64| json!({
            "id": id, "filename": "x", "mimeType": mime, "size": size, "data": id,
        });
        let with = |reference: serde_json::Value| {
            let mut note = note();
            note["attachments"] = json!([reference]);
            note
        };

        let response = pull(json!({ "notes": [
            with(reference(ATTACHMENT_ID, "text/plain", 3)),
            with(reference("not-an-id", "text/plain", 3)),
            with(reference(ATTACHMENT_ID, "text/plain\u{7}", 3)),
            with(reference(ATTACHMENT_ID, "text/plain", -1)),
            with(reference(ATTACHMENT_ID, "text/plain", i64::MAX)),
        ] }));

        assert_eq!(response.notes.len(), 1);
        assert_eq!(response.dropped, 4);
    }

    #[test]
    fn a_deletion_for_something_that_is_not_a_note_id_is_dropped() {
        let response = pull(json!({ "deletions": [
            { "id": NOTE_ID, "deletedAt": "2026-01-01T00:00:00Z" },
            { "id": "%", "deletedAt": "2026-01-01T00:00:00Z" },
            { "id": ATTACHMENT_ID, "deletedAt": "whenever" },
        ] }));

        assert_eq!(response.deletions.len(), 1);
        assert_eq!(response.dropped, 2);
    }

    #[test]
    fn a_version_of_something_that_is_not_a_note_id_is_dropped() {
        let version = |note_id: &str, key: &str| json!({
            "versionKey": key, "noteId": note_id, "version": 1,
            "createdAt": "2026-01-01T00:00:00Z", "syncedAt": "2026-01-01T00:00:00Z",
            "content": "{}", "tags": [], "attachments": [], "reason": "sync",
        });

        let response = pull(json!({ "versions": [
            version(NOTE_ID, &format!("{NOTE_ID}:1")),
            version("not-an-id", "not-an-id:1"),
            version(NOTE_ID, &format!("{ATTACHMENT_ID}:1")),
        ] }));

        assert_eq!(response.versions.len(), 1);
        assert_eq!(response.dropped, 2);
    }

    #[test]
    fn an_inbox_item_with_a_path_for_an_id_is_dropped() {
        let item = |id: &str| json!({
            "id": id, "content": "text", "tags": [], "createdAt": "2026-01-01T00:00:00Z", "sizeBytes": 4,
        });

        let response = pull(json!({ "inboxItems": [item(NOTE_ID), item("../sync/device")] }));

        assert_eq!(response.inbox_items.unwrap().len(), 1);
        assert_eq!(response.dropped, 1);
    }

    #[test]
    fn a_hash_chain_is_cut_to_what_it_can_hold() {
        let mut long = note();
        long["hashChain"] = json!(vec!["h"; MAX_HASH_CHAIN_LENGTH + 10]);
        let mut oversized = note();
        oversized["hashChain"] = json!(["h", "x".repeat(MAX_HASH_LENGTH + 1)]);

        let response = pull(json!({ "notes": [long, oversized] }));

        assert_eq!(response.notes[0].hash_chain.as_ref().unwrap().len(), MAX_HASH_CHAIN_LENGTH);
        assert_eq!(response.notes[1].hash_chain, Some(vec!["h".to_string()]));
    }

    #[test]
    fn a_response_that_is_not_a_pull_at_all_is_an_error() {
        assert!(SyncPullResponse::parse("[]").is_err());
        assert!(SyncPullResponse::parse(r#"{"syncedAt":"never"}"#).is_err());
    }
}
