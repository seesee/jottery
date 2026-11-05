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
    pub archived: bool,
    pub archived_at: Option<DateTime<Utc>>,
    pub locked: bool,
    pub locked_at: Option<DateTime<Utc>>,
    pub deleted: bool,
    pub deleted_at: Option<DateTime<Utc>>,
    pub sync_hash: Option<String>,
    pub version: i32,
    pub word_wrap: bool,
    pub syntax_language: SyntaxLanguage,
    /// The note's type, if it's one this client has no variant for. Web has dozens. It's
    /// shown as plain text here and goes back to the server as it came.
    #[serde(default)]
    pub syntax_unknown: Option<String>,
    pub color: Option<String>,        // Semantic color name (unencrypted)
    // Hash chain fields for git-like conflict detection
    pub content_hash: Option<String>,      // SHA-256 of content + tags + attachments + change fields
    pub parent_hash: Option<String>,       // Hash of previous version (None for new notes)
    pub hash_chain: Option<Vec<String>>,   // Array of ancestor hashes (max 50)
    /// Whether another client shows this note's preview. This client has none, but it
    /// keeps the value so that its pushes don't erase it.
    #[serde(default)]
    pub show_preview: Option<bool>,
    /// A local change the server hasn't accepted yet
    #[serde(default)]
    pub needs_sync: bool,
    /// This device can vouch that `attachments` is the note's whole list
    #[serde(default)]
    pub attachments_complete: bool,
}

/// Syntax highlighting language options
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum SyntaxLanguage {
    Plain,
    Javascript,
    Typescript,
    Python,
    #[default]
    Markdown,
    Json,
    Html,
    Xml,
    Css,
    Sql,
    Bash,
    Perl,
    Calc,
    Outliner,
}


impl SyntaxLanguage {
    /// Read a type's name, which can be one of the many only other clients have. Returns
    /// how to show it here, and the name to keep if there's no variant for it.
    pub fn from_name(name: &str) -> (Self, Option<String>) {
        match name.parse() {
            Ok(language) => (language, None),
            Err(_) => (Self::Plain, Some(name.to_string())),
        }
    }

    /// Get the next syntax language in the cycle
    pub fn next(self) -> Self {
        match self {
            Self::Plain => Self::Javascript,
            Self::Javascript => Self::Typescript,
            Self::Typescript => Self::Python,
            Self::Python => Self::Markdown,
            Self::Markdown => Self::Json,
            Self::Json => Self::Html,
            Self::Html => Self::Xml,
            Self::Xml => Self::Css,
            Self::Css => Self::Sql,
            Self::Sql => Self::Bash,
            Self::Bash => Self::Perl,
            Self::Perl => Self::Calc,
            Self::Calc => Self::Outliner,
            Self::Outliner => Self::Plain,
        }
    }

    /// Get the previous syntax language in the cycle
    pub fn prev(self) -> Self {
        match self {
            Self::Plain => Self::Outliner,
            Self::Javascript => Self::Plain,
            Self::Typescript => Self::Javascript,
            Self::Python => Self::Typescript,
            Self::Markdown => Self::Python,
            Self::Json => Self::Markdown,
            Self::Html => Self::Json,
            Self::Xml => Self::Html,
            Self::Css => Self::Xml,
            Self::Sql => Self::Css,
            Self::Bash => Self::Sql,
            Self::Perl => Self::Bash,
            Self::Calc => Self::Perl,
            Self::Outliner => Self::Calc,
        }
    }
}

impl std::fmt::Display for SyntaxLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Plain => write!(f, "plain"),
            Self::Javascript => write!(f, "javascript"),
            Self::Typescript => write!(f, "typescript"),
            Self::Python => write!(f, "python"),
            Self::Markdown => write!(f, "markdown"),
            Self::Json => write!(f, "json"),
            Self::Html => write!(f, "html"),
            Self::Xml => write!(f, "xml"),
            Self::Css => write!(f, "css"),
            Self::Sql => write!(f, "sql"),
            Self::Bash => write!(f, "bash"),
            Self::Perl => write!(f, "perl"),
            Self::Calc => write!(f, "calc"),
            Self::Outliner => write!(f, "outliner"),
        }
    }
}

impl std::str::FromStr for SyntaxLanguage {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "plain" => Ok(Self::Plain),
            "javascript" | "js" => Ok(Self::Javascript),
            "typescript" | "ts" => Ok(Self::Typescript),
            "python" | "py" => Ok(Self::Python),
            "markdown" | "md" => Ok(Self::Markdown),
            "json" => Ok(Self::Json),
            "html" => Ok(Self::Html),
            "xml" => Ok(Self::Xml),
            "css" => Ok(Self::Css),
            "sql" => Ok(Self::Sql),
            "bash" | "sh" => Ok(Self::Bash),
            "perl" | "pl" => Ok(Self::Perl),
            "calc" => Ok(Self::Calc),
            "outliner" | "outline" => Ok(Self::Outliner),
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
            archived: false,
            archived_at: None,
            locked: false,
            locked_at: None,
            deleted: false,
            deleted_at: None,
            sync_hash: None,
            version: 1,
            word_wrap: true,
            syntax_language: SyntaxLanguage::Markdown,
            syntax_unknown: None,
            color: None,
            // Hash chain fields - computed on save
            content_hash: None,
            parent_hash: None,
            hash_chain: None,
            show_preview: None,
            needs_sync: true,
            attachments_complete: true,
        }
    }

    /// The note's type as other clients know it, whether or not this one does.
    pub fn syntax_name(&self) -> String {
        self.syntax_unknown.clone().unwrap_or_else(|| self.syntax_language.to_string())
    }

    /// Take a type by name, keeping one this client doesn't know.
    pub fn set_syntax_name(&mut self, name: &str) {
        (self.syntax_language, self.syntax_unknown) = SyntaxLanguage::from_name(name);
    }

    /// Choose a type here, which replaces any this client didn't know.
    pub fn set_syntax(&mut self, language: SyntaxLanguage) {
        self.syntax_language = language;
        self.syntax_unknown = None;
    }

    /// Update modified timestamp
    pub fn touch(&mut self) {
        self.modified_at = Utc::now();
        self.version += 1;
    }

    /// Mark note as deleted (soft delete)
    #[allow(dead_code)]
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

    /// Toggle locked status
    pub fn toggle_lock(&mut self) {
        self.locked = !self.locked;
        self.locked_at = if self.locked { Some(Utc::now()) } else { None };
        self.touch();
    }

    /// Check if note is blank (empty content, no tags, no attachments)
    pub fn is_blank(&self) -> bool {
        self.content.trim().is_empty() && self.tags.is_empty() && self.attachments.is_empty()
    }
}


/// The first `max_chars` characters of `text`.
///
/// Ids and timestamps arrive from servers and imports, so they can be shorter than
/// expected or hold multi-byte text. Slicing bytes would panic on either.
pub fn char_prefix(text: &str, max_chars: usize) -> &str {
    match text.char_indices().nth(max_chars) {
        Some((end, _)) => &text[..end],
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_prefix_never_splits_a_character() {
        assert_eq!(char_prefix("日本語のノート", 3), "日本語");
        assert_eq!(char_prefix("abc", 8), "abc");
        assert_eq!(char_prefix("", 8), "");
        assert_eq!(char_prefix("12345678-rest", 8), "12345678");
    }

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

    /// Web has over 40 types. One this client doesn't know must survive a round trip.
    #[test]
    fn a_type_this_client_does_not_know_is_kept_exactly() {
        let mut note = Note::new("fn main() {}".to_string());

        note.set_syntax_name("rust");

        assert_eq!(note.syntax_language, SyntaxLanguage::Plain);
        assert_eq!(note.syntax_name(), "rust");
    }

    #[test]
    fn choosing_a_type_here_replaces_one_this_client_did_not_know() {
        let mut note = Note::new("text".to_string());
        note.set_syntax_name("rust");

        note.set_syntax(SyntaxLanguage::Python);

        assert_eq!(note.syntax_name(), "python");
    }

    #[test]
    fn typescript_and_xml_are_known_here() {
        for name in ["typescript", "xml"] {
            let mut note = Note::new("text".to_string());
            note.set_syntax_name(name);
            assert_eq!(note.syntax_unknown, None);
            assert_eq!(note.syntax_name(), name);
        }
    }

    #[test]
    fn the_cycle_visits_every_type_in_both_directions() {
        let mut seen = vec![SyntaxLanguage::Plain];
        let mut language = SyntaxLanguage::Plain.next();
        while language != SyntaxLanguage::Plain {
            assert_eq!(language.prev().next(), language);
            seen.push(language);
            language = language.next();
        }
        assert_eq!(seen.len(), 14);
    }

    #[test]
    fn test_syntax_language_from_str() {
        assert_eq!("plain".parse::<SyntaxLanguage>().unwrap(), SyntaxLanguage::Plain);
        assert_eq!("javascript".parse::<SyntaxLanguage>().unwrap(), SyntaxLanguage::Javascript);
        assert_eq!("js".parse::<SyntaxLanguage>().unwrap(), SyntaxLanguage::Javascript);
        assert_eq!("python".parse::<SyntaxLanguage>().unwrap(), SyntaxLanguage::Python);
    }
}
