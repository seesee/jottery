//! Virtual Tag System
//!
//! Virtual tags are special tags with prefixes that have semantic meaning.
//! They are stored with their prefix but displayed differently in the UI.
//!
//! Current virtual tags:
//! - t: (title) - Custom note title
//!
//! Future virtual tags could include:
//! - d: (due date)
//! - p: (priority)
//! - etc.

use rust_i18n::t;

/// The prefix for title tags
pub const TITLE_TAG_PREFIX: &str = "t:";

/// Check if a tag is a virtual tag
pub fn is_virtual_tag(tag: &str) -> bool {
    tag.starts_with(TITLE_TAG_PREFIX)
}

/// Get regular (non-virtual) tags
pub fn get_regular_tags(tags: &[String]) -> Vec<&String> {
    tags.iter().filter(|t| !is_virtual_tag(t)).collect()
}

/// Get all virtual tags
pub fn get_virtual_tags(tags: &[String]) -> Vec<&String> {
    tags.iter().filter(|t| is_virtual_tag(t)).collect()
}

/// Extract title from note, checking for t: tag first, then first non-empty line
pub fn get_note_title(content: &str, tags: &[String]) -> String {
    // Check for title tag first
    if let Some(title_tag) = tags.iter().find(|t| t.starts_with(TITLE_TAG_PREFIX)) {
        let title = title_tag.strip_prefix(TITLE_TAG_PREFIX).unwrap_or("").trim();
        if !title.is_empty() {
            return title.to_string();
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
    tags.iter().any(|t| t.starts_with(TITLE_TAG_PREFIX))
}

/// Get the title tag value (without prefix), or None
pub fn get_title_tag_value(tags: &[String]) -> Option<&str> {
    tags.iter()
        .find(|t| t.starts_with(TITLE_TAG_PREFIX))
        .map(|t| t.strip_prefix(TITLE_TAG_PREFIX).unwrap_or(""))
}

/// Set title tag, removing existing. Returns new tags.
pub fn set_title_tag(tags: &[String], title: &str) -> Vec<String> {
    let mut filtered: Vec<String> = tags
        .iter()
        .filter(|t| !t.starts_with(TITLE_TAG_PREFIX))
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
        .filter(|t| !t.starts_with(TITLE_TAG_PREFIX))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_virtual_tag() {
        assert!(is_virtual_tag("t:My Title"));
        assert!(is_virtual_tag("t:"));
        assert!(!is_virtual_tag("project"));
        assert!(!is_virtual_tag("title"));
        assert!(!is_virtual_tag("t"));
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
    }

    #[test]
    fn test_get_note_title_with_tag() {
        let content = "First line content";
        let tags = vec!["t:Custom Title".to_string(), "work".to_string()];
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
        let tags_with = vec!["t:Title".to_string(), "work".to_string()];
        let tags_without = vec!["work".to_string()];
        assert!(has_custom_title(&tags_with));
        assert!(!has_custom_title(&tags_without));
    }

    #[test]
    fn test_set_title_tag() {
        let tags = vec!["project".to_string(), "t:Old".to_string()];
        let new_tags = set_title_tag(&tags, "New Title");
        assert_eq!(new_tags.len(), 2);
        assert!(new_tags.contains(&"project".to_string()));
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
    }
}
