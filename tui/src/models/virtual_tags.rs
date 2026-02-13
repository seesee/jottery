//! Virtual Tag System
//!
//! Virtual tags are special tags with prefixes that have semantic meaning.
//! They are stored with their prefix but displayed differently in the UI.
//!
//! Current virtual tags:
//! - t: or title: (title) - Custom note title
//!
//! Future virtual tags could include:
//! - d: (due date)
//! - p: (priority)
//! - etc.

use rust_i18n::t;

/// The canonical prefix for title tags (used when creating new tags)
pub const TITLE_TAG_PREFIX: &str = "t:";

/// All valid prefixes for title tags
pub const TITLE_TAG_PREFIXES: &[&str] = &["t:", "title:"];

/// Check if a tag is a title tag (matches any title prefix)
pub fn is_title_tag(tag: &str) -> bool {
    TITLE_TAG_PREFIXES.iter().any(|prefix| tag.starts_with(prefix))
}

/// Get the title value from a title tag, stripping the prefix
pub fn get_title_from_tag(tag: &str) -> Option<&str> {
    for prefix in TITLE_TAG_PREFIXES {
        if let Some(value) = tag.strip_prefix(prefix) {
            return Some(value);
        }
    }
    None
}

/// Check if a tag is a virtual tag
pub fn is_virtual_tag(tag: &str) -> bool {
    is_title_tag(tag)
}

/// Get regular (non-virtual) tags
pub fn get_regular_tags(tags: &[String]) -> Vec<&String> {
    tags.iter().filter(|t| !is_virtual_tag(t)).collect()
}

/// Get all virtual tags
pub fn get_virtual_tags(tags: &[String]) -> Vec<&String> {
    tags.iter().filter(|t| is_virtual_tag(t)).collect()
}

/// Extract title from note, checking for t: or title: tag first, then first non-empty line
pub fn get_note_title(content: &str, tags: &[String]) -> String {
    // Check for title tag first (supports t: and title: prefixes)
    if let Some(title_tag) = tags.iter().find(|t| is_title_tag(t)) {
        if let Some(title) = get_title_from_tag(title_tag) {
            let title = title.trim();
            if !title.is_empty() {
                return title.to_string();
            }
        }
    }

    // Fall back to first non-empty line
    content
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| t!("note.untitled").to_string())
}

/// Check if note has custom title tag
pub fn has_custom_title(tags: &[String]) -> bool {
    tags.iter().any(|t| is_title_tag(t))
}

/// Get the title tag value (without prefix), or None
pub fn get_title_tag_value(tags: &[String]) -> Option<&str> {
    tags.iter()
        .find(|t| is_title_tag(t))
        .and_then(|t| get_title_from_tag(t))
}

/// Set title tag, removing existing title tags (both t: and title: prefixes).
/// Returns new tags. Always uses the canonical 't:' prefix for new tags.
pub fn set_title_tag(tags: &[String], title: &str) -> Vec<String> {
    let mut filtered: Vec<String> = tags
        .iter()
        .filter(|t| !is_title_tag(t))
        .cloned()
        .collect();

    if !title.trim().is_empty() {
        filtered.push(format!("{}{}", TITLE_TAG_PREFIX, title.trim()));
    }

    filtered
}

/// Remove title tag. Returns new tags.
pub fn remove_title_tag(tags: &[String]) -> Vec<String> {
    tags.iter()
        .filter(|t| !is_title_tag(t))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_title_tag() {
        // t: prefix
        assert!(is_title_tag("t:My Title"));
        assert!(is_title_tag("t:"));
        // title: prefix
        assert!(is_title_tag("title:My Title"));
        assert!(is_title_tag("title:"));
        // Not title tags
        assert!(!is_title_tag("project"));
        assert!(!is_title_tag("title"));
        assert!(!is_title_tag("t"));
    }

    #[test]
    fn test_is_virtual_tag() {
        assert!(is_virtual_tag("t:My Title"));
        assert!(is_virtual_tag("title:My Title"));
        assert!(!is_virtual_tag("project"));
    }

    #[test]
    fn test_get_title_from_tag() {
        assert_eq!(get_title_from_tag("t:My Title"), Some("My Title"));
        assert_eq!(get_title_from_tag("title:My Title"), Some("My Title"));
        assert_eq!(get_title_from_tag("t:"), Some(""));
        assert_eq!(get_title_from_tag("project"), None);
    }

    #[test]
    fn test_get_regular_tags() {
        let tags = vec![
            "project".to_string(),
            "t:My Title".to_string(),
            "work".to_string(),
        ];
        let regular = get_regular_tags(&tags);
        assert_eq!(regular.len(), 2);
        assert_eq!(regular[0], "project");
        assert_eq!(regular[1], "work");

        // Also works with title: prefix
        let tags2 = vec![
            "project".to_string(),
            "title:My Title".to_string(),
        ];
        let regular2 = get_regular_tags(&tags2);
        assert_eq!(regular2.len(), 1);
    }

    #[test]
    fn test_get_note_title_with_t_prefix() {
        let content = "First line content";
        let tags = vec!["t:Custom Title".to_string(), "work".to_string()];
        assert_eq!(get_note_title(content, &tags), "Custom Title");
    }

    #[test]
    fn test_get_note_title_with_title_prefix() {
        let content = "First line content";
        let tags = vec!["title:Custom Title".to_string(), "work".to_string()];
        assert_eq!(get_note_title(content, &tags), "Custom Title");
    }

    #[test]
    fn test_get_note_title_without_tag() {
        let content = "First line content\nSecond line";
        let tags = vec!["work".to_string()];
        assert_eq!(get_note_title(content, &tags), "First line content");
    }

    #[test]
    fn test_get_note_title_empty_tag() {
        let content = "First line content";
        let tags = vec!["t:".to_string()];
        assert_eq!(get_note_title(content, &tags), "First line content");
    }

    #[test]
    fn test_has_custom_title() {
        let tags_with_t = vec!["t:Title".to_string(), "work".to_string()];
        let tags_with_title = vec!["title:Title".to_string(), "work".to_string()];
        let tags_without = vec!["work".to_string()];
        assert!(has_custom_title(&tags_with_t));
        assert!(has_custom_title(&tags_with_title));
        assert!(!has_custom_title(&tags_without));
    }

    #[test]
    fn test_set_title_tag_replaces_t_prefix() {
        let tags = vec!["project".to_string(), "t:Old".to_string()];
        let new_tags = set_title_tag(&tags, "New Title");
        assert_eq!(new_tags.len(), 2);
        assert!(new_tags.contains(&"project".to_string()));
        assert!(new_tags.contains(&"t:New Title".to_string()));
    }

    #[test]
    fn test_set_title_tag_replaces_title_prefix() {
        let tags = vec!["project".to_string(), "title:Old".to_string()];
        let new_tags = set_title_tag(&tags, "New Title");
        assert_eq!(new_tags.len(), 2);
        assert!(new_tags.contains(&"project".to_string()));
        // Always uses canonical t: prefix
        assert!(new_tags.contains(&"t:New Title".to_string()));
    }

    #[test]
    fn test_set_title_tag_empty() {
        let tags = vec!["project".to_string(), "t:Old".to_string()];
        let new_tags = set_title_tag(&tags, "");
        assert_eq!(new_tags.len(), 1);
        assert_eq!(new_tags[0], "project");
    }

    #[test]
    fn test_remove_title_tag() {
        let tags = vec!["project".to_string(), "t:Title".to_string()];
        let new_tags = remove_title_tag(&tags);
        assert_eq!(new_tags.len(), 1);
        assert_eq!(new_tags[0], "project");

        // Also removes title: prefix
        let tags2 = vec!["project".to_string(), "title:Title".to_string()];
        let new_tags2 = remove_title_tag(&tags2);
        assert_eq!(new_tags2.len(), 1);
    }
}
