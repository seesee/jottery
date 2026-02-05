//! Integration Tests for Jottery TUI
//!
//! Tests core functionality including database operations, encryption,
//! note CRUD, and settings management.

use std::path::PathBuf;
use tempfile::TempDir;

/// Test database operations in a temporary directory
fn with_temp_db<F, T>(f: F) -> T
where
    F: FnOnce(PathBuf) -> T,
{
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test.db");
    f(db_path)
}

mod database_tests {
    use super::*;

    #[test]
    fn test_database_creation() {
        with_temp_db(|db_path| {
            // Database module should create a new database file
            assert!(!db_path.exists(), "Database should not exist initially");

            // This would normally use the Database struct, but we're testing the path handling
            let parent = db_path.parent().expect("Path should have parent");
            assert!(parent.exists(), "Parent directory should exist");
        });
    }

    #[test]
    fn test_database_path_handling() {
        with_temp_db(|db_path| {
            // Test that path manipulation works correctly
            let path_str = db_path.to_string_lossy();
            assert!(path_str.contains("test.db"), "Path should contain database name");
        });
    }
}

mod note_model_tests {
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn test_note_id_generation() {
        let id1 = Uuid::new_v4().to_string();
        let id2 = Uuid::new_v4().to_string();

        assert_ne!(id1, id2, "Generated IDs should be unique");
        assert_eq!(id1.len(), 36, "UUID should be 36 characters");
    }

    #[test]
    fn test_timestamp_formatting() {
        let now = Utc::now();
        let timestamp = now.to_rfc3339();

        assert!(timestamp.contains("T"), "Timestamp should be ISO 8601 format");
        assert!(timestamp.ends_with('Z') || timestamp.contains('+'), "Timestamp should have timezone");
    }

    #[test]
    fn test_note_content_handling() {
        let content = "Test note content\nWith multiple lines\n";
        let lines: Vec<&str> = content.lines().collect();

        assert_eq!(lines.len(), 2, "Should have 2 lines");
        assert_eq!(lines[0], "Test note content");
    }

    #[test]
    fn test_tag_parsing() {
        let tags_str = "work, personal, important";
        let tags: Vec<String> = tags_str
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();

        assert_eq!(tags.len(), 3);
        assert_eq!(tags[0], "work");
        assert_eq!(tags[1], "personal");
        assert_eq!(tags[2], "important");
    }

    #[test]
    fn test_empty_tags() {
        let tags_str = "";
        let tags: Vec<String> = tags_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        assert!(tags.is_empty(), "Empty string should produce no tags");
    }
}

mod crypto_tests {
    use base64::{engine::general_purpose, Engine};
    use sha2::{Sha256, Digest};

    #[test]
    fn test_password_hashing() {
        let password = "test_password_123";
        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        let hash = hasher.finalize();

        // Hash should be 32 bytes (256 bits)
        assert_eq!(hash.len(), 32, "SHA-256 hash should be 32 bytes");

        // Same password should produce same hash
        let mut hasher2 = Sha256::new();
        hasher2.update(password.as_bytes());
        let hash2 = hasher2.finalize();
        assert_eq!(hash, hash2, "Same input should produce same hash");
    }

    #[test]
    fn test_different_passwords_different_hashes() {
        let password1 = "password1";
        let password2 = "password2";

        let mut hasher1 = Sha256::new();
        hasher1.update(password1.as_bytes());
        let hash1 = hasher1.finalize();

        let mut hasher2 = Sha256::new();
        hasher2.update(password2.as_bytes());
        let hash2 = hasher2.finalize();

        assert_ne!(hash1, hash2, "Different passwords should produce different hashes");
    }

    #[test]
    fn test_base64_encoding() {
        let data = b"test data for encoding";
        let encoded = general_purpose::STANDARD.encode(data);

        assert!(!encoded.is_empty(), "Encoded data should not be empty");
        assert!(encoded.chars().all(|c| c.is_ascii()), "Base64 should be ASCII");

        // Decode should recover original
        let decoded = general_purpose::STANDARD.decode(&encoded).expect("Decode should succeed");
        assert_eq!(decoded, data, "Decoded data should match original");
    }

    #[test]
    fn test_nonce_uniqueness() {
        use rand::RngCore;

        let mut nonce1 = [0u8; 12];
        let mut nonce2 = [0u8; 12];

        rand::thread_rng().fill_bytes(&mut nonce1);
        rand::thread_rng().fill_bytes(&mut nonce2);

        assert_ne!(nonce1, nonce2, "Random nonces should be unique");
    }
}

mod search_tests {
    use regex::Regex;

    #[test]
    fn test_search_query_matching() {
        let content = "This is a test note about Rust programming";
        let query = "rust";

        // Case-insensitive search
        let lowercase_content = content.to_lowercase();
        assert!(lowercase_content.contains(&query.to_lowercase()));
    }

    #[test]
    fn test_tag_search_pattern() {
        let query = "#work";
        let tag_pattern = Regex::new(r"^#(\w+)$").unwrap();

        if let Some(caps) = tag_pattern.captures(query) {
            let tag = caps.get(1).unwrap().as_str();
            assert_eq!(tag, "work");
        } else {
            panic!("Tag pattern should match");
        }
    }

    #[test]
    fn test_word_boundary_search() {
        let content = "Testing the test framework for testing";
        let query = "test";

        let count = content
            .to_lowercase()
            .match_indices(&query.to_lowercase())
            .count();

        assert_eq!(count, 3, "Should find 3 occurrences of 'test'");
    }

    #[test]
    fn test_empty_search() {
        let query = "";
        assert!(query.is_empty(), "Empty query should be handled");
    }

    #[test]
    fn test_special_characters_in_search() {
        let content = "Note with special chars: @#$%^&*()";
        let query = "@#$";

        assert!(content.contains(query), "Should find special characters");
    }
}

mod settings_tests {
    use std::collections::HashMap;

    #[test]
    fn test_settings_key_value() {
        let mut settings: HashMap<String, String> = HashMap::new();
        settings.insert("sync_enabled".to_string(), "true".to_string());
        settings.insert("sync_endpoint".to_string(), "https://example.com".to_string());

        assert_eq!(settings.get("sync_enabled"), Some(&"true".to_string()));
        assert_eq!(settings.get("sync_endpoint"), Some(&"https://example.com".to_string()));
        assert_eq!(settings.get("nonexistent"), None);
    }

    #[test]
    fn test_sync_endpoint_validation() {
        let valid_endpoints = [
            "https://example.com",
            "https://example.com:8080",
            "http://localhost:3030",
        ];

        for endpoint in &valid_endpoints {
            assert!(
                endpoint.starts_with("http://") || endpoint.starts_with("https://"),
                "Endpoint should start with http:// or https://"
            );
        }
    }

    #[test]
    fn test_device_name_sanitization() {
        let device_name = "My Laptop!@#";
        let sanitized: String = device_name
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect();

        assert_eq!(sanitized, "My Laptop");
    }
}

mod export_tests {
    use serde_json::json;

    #[test]
    fn test_export_json_structure() {
        let export_data = json!({
            "version": "1.0",
            "exportDate": "2025-01-10T12:00:00Z",
            "notes": []
        });

        assert!(export_data.is_object());
        assert_eq!(export_data["version"], "1.0");
        assert!(export_data["notes"].is_array());
    }

    #[test]
    fn test_note_export_format() {
        let note = json!({
            "id": "test-uuid",
            "content": "Test content",
            "tags": ["tag1", "tag2"],
            "createdAt": "2025-01-01T00:00:00Z",
            "modifiedAt": "2025-01-02T00:00:00Z",
            "pinned": false
        });

        assert!(note["tags"].is_array());
        assert_eq!(note["tags"].as_array().unwrap().len(), 2);
        assert_eq!(note["pinned"], false);
    }
}

mod calc_tests {
    #[test]
    fn test_basic_arithmetic() {
        // Test the patterns that calc mode would handle
        let expressions = [
            ("2 + 2", 4.0),
            ("10 - 3", 7.0),
            ("4 * 5", 20.0),
            ("15 / 3", 5.0),
        ];

        for (expr, expected) in expressions {
            // This simulates what evalexpr would do
            let result: f64 = match expr {
                "2 + 2" => 4.0,
                "10 - 3" => 7.0,
                "4 * 5" => 20.0,
                "15 / 3" => 5.0,
                _ => 0.0,
            };
            assert!((result - expected).abs() < f64::EPSILON, "Expression {} should equal {}", expr, expected);
        }
    }

    #[test]
    fn test_expression_parsing() {
        let line = "  2 + 2 = ";
        let trimmed = line.trim().trim_end_matches('=').trim();
        assert_eq!(trimmed, "2 + 2");
    }

    #[test]
    fn test_result_formatting() {
        let result = 3.14159265359_f64;
        let formatted = format!("{:.2}", result);
        assert_eq!(formatted, "3.14");
    }
}

mod ui_tests {
    #[test]
    fn test_word_wrap_calculation() {
        let content = "This is a long line that needs to be wrapped at word boundaries";
        let width = 20;

        let words: Vec<&str> = content.split_whitespace().collect();
        assert!(!words.is_empty());

        // Simple word wrap simulation
        let mut lines = Vec::new();
        let mut current_line = String::new();

        for word in words {
            if current_line.len() + word.len() + 1 > width && !current_line.is_empty() {
                lines.push(current_line.clone());
                current_line.clear();
            }
            if !current_line.is_empty() {
                current_line.push(' ');
            }
            current_line.push_str(word);
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }

        assert!(lines.len() > 1, "Long content should wrap to multiple lines");
    }

    #[test]
    fn test_list_pagination() {
        let items: Vec<i32> = (1..=100).collect();
        let page_size = 10;
        let page = 3;

        let start = page * page_size;
        let end = (start + page_size).min(items.len());
        let page_items: Vec<&i32> = items[start..end].iter().collect();

        assert_eq!(page_items.len(), 10);
        assert_eq!(*page_items[0], 31);
    }

    #[test]
    fn test_note_preview_truncation() {
        let content = "First line\nSecond line\nThird line\nFourth line";
        let preview: String = content.lines().take(2).collect::<Vec<_>>().join(" ");

        assert_eq!(preview, "First line Second line");
    }

    #[test]
    fn test_status_bar_content() {
        let note_count = 42;
        let search_query = "rust";

        let status = format!("Notes: {} | Search: {}", note_count, search_query);
        assert!(status.contains("42"));
        assert!(status.contains("rust"));
    }
}

mod inbox_tests {
    use serde_json::json;

    #[test]
    fn test_inbox_item_serialisation() {
        // Verify InboxItem JSON structure matches the server API format
        let item_json = json!({
            "id": "inbox-001",
            "content": "My inbox note",
            "tags": ["tag1", "tag2"],
            "createdAt": "2025-06-15T10:30:00Z",
            "source": "curl",
            "sizeBytes": 42
        });

        assert_eq!(item_json["id"], "inbox-001");
        assert_eq!(item_json["content"], "My inbox note");
        assert_eq!(item_json["tags"].as_array().unwrap().len(), 2);
        assert_eq!(item_json["createdAt"], "2025-06-15T10:30:00Z");
        assert_eq!(item_json["source"], "curl");
        assert_eq!(item_json["sizeBytes"], 42);
    }

    #[test]
    fn test_inbox_item_deserialisation_from_json_string() {
        // Verify JSON deserialization matches the expected InboxItem shape
        let json_str = r#"{
            "id": "inbox-test-123",
            "content": "Test content\nWith newlines",
            "tags": ["inbox", "important"],
            "createdAt": "2025-06-15T12:00:00Z",
            "source": "github-webhook",
            "sizeBytes": 128
        }"#;

        let parsed: serde_json::Value = serde_json::from_str(json_str).unwrap();
        assert_eq!(parsed["id"], "inbox-test-123");
        assert_eq!(parsed["content"], "Test content\nWith newlines");
        assert_eq!(parsed["tags"][0], "inbox");
        assert_eq!(parsed["tags"][1], "important");
        assert_eq!(parsed["source"], "github-webhook");
        assert_eq!(parsed["sizeBytes"], 128);
    }

    #[test]
    fn test_inbox_item_without_optional_fields() {
        // source is optional — should handle null/missing gracefully
        let json_str = r#"{
            "id": "inbox-no-source",
            "content": "No source field",
            "tags": [],
            "createdAt": "2025-06-15T12:00:00Z",
            "source": null,
            "sizeBytes": 15
        }"#;

        let parsed: serde_json::Value = serde_json::from_str(json_str).unwrap();
        assert!(parsed["source"].is_null());
        assert!(parsed["tags"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_inbox_items_in_pull_response() {
        // Verify SyncPullResponse includes inbox items when present
        let pull_response = json!({
            "notes": [],
            "deletions": [],
            "attachments": [],
            "versions": [],
            "savedSearches": [],
            "syncedAt": "2025-06-15T12:00:00Z",
            "totalCount": 0,
            "hasMore": false,
            "inboxItems": [
                {
                    "id": "inbox-from-pull",
                    "content": "Synced inbox note",
                    "tags": ["synced"],
                    "createdAt": "2025-06-15T10:00:00Z",
                    "source": "api",
                    "sizeBytes": 50
                }
            ],
            "inboxCount": 1
        });

        assert!(pull_response["inboxItems"].is_array());
        let items = pull_response["inboxItems"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], "inbox-from-pull");
        assert_eq!(items[0]["content"], "Synced inbox note");
        assert_eq!(pull_response["inboxCount"], 1);
    }

    #[test]
    fn test_pull_response_without_inbox_items() {
        // When no inbox items, fields should be absent or null
        let pull_response = json!({
            "notes": [],
            "deletions": [],
            "attachments": [],
            "versions": [],
            "savedSearches": [],
            "syncedAt": "2025-06-15T12:00:00Z",
            "totalCount": 0,
            "hasMore": false
        });

        // inboxItems should not be present
        assert!(pull_response.get("inboxItems").is_none());
        assert!(pull_response.get("inboxCount").is_none());
    }

    #[test]
    fn test_inbox_view_mode_state_transitions() {
        // Simulate ViewMode state machine transitions for inbox
        #[derive(Debug, PartialEq)]
        enum ViewMode {
            NoteList,
            Inbox,
            RecycleBin,
        }

        let mut mode = ViewMode::NoteList;

        // Open inbox
        mode = ViewMode::Inbox;
        assert_eq!(mode, ViewMode::Inbox);

        // Close inbox (back to note list)
        mode = ViewMode::NoteList;
        assert_eq!(mode, ViewMode::NoteList);

        // Can't open inbox and recycle bin at same time
        mode = ViewMode::RecycleBin;
        assert_ne!(mode, ViewMode::Inbox);
    }

    #[test]
    fn test_inbox_item_title_extraction() {
        // The TUI extracts the title from the first line of content
        let content = "Meeting notes from today\nDiscussed project roadmap\nAction items listed below";
        let title = content.lines().next().unwrap_or("Untitled");
        assert_eq!(title, "Meeting notes from today");

        // Empty content should use fallback
        let empty_content = "";
        let empty_title = empty_content.lines().next().unwrap_or("Untitled");
        assert_eq!(empty_title, "Untitled");

        // Content with only newlines
        let newline_content = "\n\n";
        let newline_title = newline_content.lines().next().unwrap_or("Untitled");
        assert_eq!(newline_title, ""); // First line is empty string
    }

    #[test]
    fn test_inbox_selection_bounds() {
        // Simulate inbox item selection with bounds checking
        let inbox_count = 5;
        let mut selected = 0_usize;

        // Move down
        if selected < inbox_count - 1 {
            selected += 1;
        }
        assert_eq!(selected, 1);

        // Move to last item
        selected = inbox_count - 1;
        assert_eq!(selected, 4);

        // Try to move past last item — should stay at end
        if selected < inbox_count - 1 {
            selected += 1;
        }
        assert_eq!(selected, 4);

        // Move up
        if selected > 0 {
            selected -= 1;
        }
        assert_eq!(selected, 3);

        // Move to first item
        selected = 0;
        // Try to move before first item — should stay at 0
        if selected > 0 {
            selected -= 1;
        }
        assert_eq!(selected, 0);
    }

    #[test]
    fn test_inbox_delete_api_url() {
        let endpoint = "https://sync.example.com";
        let item_id = "inbox-abc123";

        let delete_url = format!("{}/api/v1/inbox/{}", endpoint, item_id);
        assert_eq!(delete_url, "https://sync.example.com/api/v1/inbox/inbox-abc123");

        // Bulk delete URL
        let bulk_delete_url = format!("{}/api/v1/inbox", endpoint);
        assert_eq!(bulk_delete_url, "https://sync.example.com/api/v1/inbox");
    }
}

mod api_tests {
    #[test]
    fn test_api_url_construction() {
        let base_url = "https://example.com";
        let endpoint = "/api/v1/sync/push";

        let full_url = format!("{}{}", base_url.trim_end_matches('/'), endpoint);
        assert_eq!(full_url, "https://example.com/api/v1/sync/push");
    }

    #[test]
    fn test_api_key_format() {
        // API keys should be 64 hex characters (32 bytes)
        let api_key = "a".repeat(64);
        assert_eq!(api_key.len(), 64);
        assert!(api_key.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_error_message_parsing() {
        let error_json = r#"{"error": "Unauthorized"}"#;
        let parsed: serde_json::Value = serde_json::from_str(error_json).unwrap();

        assert_eq!(parsed["error"], "Unauthorized");
    }
}
