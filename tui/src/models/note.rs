use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a note in the system
/// Content, tags, and attachments are encrypted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub synced_at: Option<DateTime<Utc>>,
    pub content: String,              // Encrypted note content
    pub tags: Vec<String>,            // Encrypted array of tags
    pub attachments: Vec<Attachment>, // Array of attachment references
    pub pinned: bool,
    pub deleted: bool,
    pub deleted_at: Option<DateTime<Utc>>,
    pub sync_hash: Option<String>,
    pub version: i32,
    pub word_wrap: bool,
    pub syntax_language: SyntaxLanguage,
}

/// Syntax highlighting language options
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SyntaxLanguage {
    Plain,
    Javascript,
    Python,
    Markdown,
    Json,
    Html,
    Css,
    Sql,
    Bash,
}

impl Default for SyntaxLanguage {
    fn default() -> Self {
        Self::Plain
    }
}

impl std::fmt::Display for SyntaxLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Plain => write!(f, "plain"),
            Self::Javascript => write!(f, "javascript"),
            Self::Python => write!(f, "python"),
            Self::Markdown => write!(f, "markdown"),
            Self::Json => write!(f, "json"),
            Self::Html => write!(f, "html"),
            Self::Css => write!(f, "css"),
            Self::Sql => write!(f, "sql"),
            Self::Bash => write!(f, "bash"),
        }
    }
}

impl std::str::FromStr for SyntaxLanguage {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "plain" => Ok(Self::Plain),
            "javascript" | "js" => Ok(Self::Javascript),
            "python" | "py" => Ok(Self::Python),
            "markdown" | "md" => Ok(Self::Markdown),
            "json" => Ok(Self::Json),
            "html" => Ok(Self::Html),
            "css" => Ok(Self::Css),
            "sql" => Ok(Self::Sql),
            "bash" | "sh" => Ok(Self::Bash),
            _ => Err(format!("Unknown syntax language: {}", s)),
        }
    }
}

/// Represents a file attachment
/// Filename is encrypted, data is a reference to encrypted blob store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: String,
    pub filename: String,            // Original filename (encrypted)
    pub mime_type: String,
    pub size: i64,
    pub data: String,                // Reference to encrypted blob store
    pub thumbnail_data: Option<String>, // Optional thumbnail for images
}

impl Note {
    /// Create a new note with default values
    pub fn new(content: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            created_at: now,
            modified_at: now,
            synced_at: None,
            content,
            tags: Vec::new(),
            attachments: Vec::new(),
            pinned: false,
            deleted: false,
            deleted_at: None,
            sync_hash: None,
            version: 1,
            word_wrap: true,
            syntax_language: SyntaxLanguage::Plain,
        }
    }

    /// Update modified timestamp
    pub fn touch(&mut self) {
        self.modified_at = Utc::now();
        self.version += 1;
    }

    /// Mark note as deleted (soft delete)
    pub fn mark_deleted(&mut self) {
        self.deleted = true;
        self.deleted_at = Some(Utc::now());
        self.touch();
    }

    /// Restore a deleted note
    pub fn restore(&mut self) {
        self.deleted = false;
        self.deleted_at = None;
        self.touch();
    }

    /// Toggle pinned status
    pub fn toggle_pin(&mut self) {
        self.pinned = !self.pinned;
        self.touch();
    }
}

impl Attachment {
    /// Create a new attachment reference
    pub fn new(filename: String, mime_type: String, size: i64, data: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            filename,
            mime_type,
            size,
            data,
            thumbnail_data: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_note_creation() {
        let note = Note::new("Test content".to_string());
        assert_eq!(note.content, "Test content");
        assert_eq!(note.version, 1);
        assert!(!note.deleted);
        assert!(!note.pinned);
        assert_eq!(note.tags.len(), 0);
    }

    #[test]
    fn test_note_touch() {
        let mut note = Note::new("Test".to_string());
        let original_modified = note.modified_at;
        std::thread::sleep(std::time::Duration::from_millis(10));
        note.touch();
        assert!(note.modified_at > original_modified);
        assert_eq!(note.version, 2);
    }

    #[test]
    fn test_note_delete_restore() {
        let mut note = Note::new("Test".to_string());
        assert!(!note.deleted);
        assert!(note.deleted_at.is_none());

        note.mark_deleted();
        assert!(note.deleted);
        assert!(note.deleted_at.is_some());

        note.restore();
        assert!(!note.deleted);
        assert!(note.deleted_at.is_none());
    }

    #[test]
    fn test_syntax_language_from_str() {
        assert_eq!("plain".parse::<SyntaxLanguage>().unwrap(), SyntaxLanguage::Plain);
        assert_eq!("javascript".parse::<SyntaxLanguage>().unwrap(), SyntaxLanguage::Javascript);
        assert_eq!("js".parse::<SyntaxLanguage>().unwrap(), SyntaxLanguage::Javascript);
        assert_eq!("python".parse::<SyntaxLanguage>().unwrap(), SyntaxLanguage::Python);
    }
}
