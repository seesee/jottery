//! The search query language.
//!
//! One parser and one matcher, shared by the note list, the opposite-mode count and the
//! CLI. Every client follows the same rules, which are written down with test cases in
//! `docs/search-query-fixture.json`, so a saved search means the same thing everywhere.
//!
//! Supported:
//! - `#tag` has a tag containing this. `#a #b` needs both, `#a | #b` either
//! - `word` contains this. `a b` needs both, `a | b` either
//! - `"exact phrase"` contains these characters in this order
//! - `-word`, `-"a phrase"` doesn't contain this. `-#tag` doesn't have this tag
//! - `word*`, `*word` a word starts or ends with this. `*word*` contains it anywhere
//! - `has:attachment`, `archived`, `archived:false`
//! - `created:>DATE`, `created:<DATE`, `created:DATE..DATE`, and the same for `modified:`
//! - `words:>N`, `words:<N`, `words:N..N`
//! - `color:KEY`, `category:NAME`, either followed by `note` or `tag`

use unicode_normalization::UnicodeNormalization;

use crate::models::{Note, UserSettings};
use crate::ui::regex;

/// Search modifiers parsed from query string
#[derive(Debug, Default)]
struct SearchModifiers {
    has_attachment: bool,
    archived: Option<bool>,
    created_after: Option<String>,
    created_before: Option<String>,
    modified_after: Option<String>,
    modified_before: Option<String>,
    word_count_min: Option<usize>,
    word_count_max: Option<usize>,
    colors: Vec<String>,           // Color keys to match
    color_target: ColorTarget,      // Where to look for colors
}

#[derive(Debug, Clone, PartialEq, Default)]
enum ColorTarget {
    #[default]
    Both,   // Match colors on notes or tags (default)
    Note,   // Match only note colors
    Tag,    // Match only tag colors
}

/// One thing a note can match.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SearchTerm {
    /// Anywhere in the content or tags
    Contains(String),
    /// A word starts with this
    Prefix(String),
    /// A word ends with this
    Suffix(String),
    /// A tag contains this
    Tag(String),
}

/// A token of the query, once modifiers are out of the way.
#[derive(Debug, Clone, PartialEq, Eq)]
enum QueryToken {
    Or,
    Term(SearchTerm),
    ExcludeText(String),
    ExcludeTag(String),
}

/// A parsed query.
#[derive(Debug, Default)]
pub struct SearchQuery {
    /// Every group has to match, through any one of its terms
    groups: Vec<Vec<SearchTerm>>,
    exclude_text: Vec<String>,
    exclude_tags: Vec<String>,
    modifiers: SearchModifiers,
}

/// Fold case and normalise, the way every client compares text.
fn normalise(text: &str) -> String {
    text.nfc().collect::<String>().to_lowercase()
}

/// Classify a word by its wildcards. A word of only asterisks is a search for them.
fn classify_word(word: &str) -> SearchTerm {
    let value = normalise(word.trim_matches('*'));
    if value.is_empty() {
        return SearchTerm::Contains(word.to_string());
    }

    match (word.starts_with('*'), word.ends_with('*')) {
        (true, false) => SearchTerm::Suffix(value),
        (false, true) => SearchTerm::Prefix(value),
        _ => SearchTerm::Contains(value),
    }
}

fn tokenise_word(word: &str) -> QueryToken {
    if let Some(tag) = word.strip_prefix("-#").filter(|tag| !tag.is_empty()) {
        return QueryToken::ExcludeTag(normalise(tag));
    }
    if let Some(tag) = word.strip_prefix('#').filter(|tag| !tag.is_empty()) {
        return QueryToken::Term(SearchTerm::Tag(normalise(tag)));
    }
    if let Some(text) = word.strip_prefix('-').filter(|text| !text.is_empty()) {
        return QueryToken::ExcludeText(normalise(text));
    }
    QueryToken::Term(classify_word(word))
}

fn tokenise_query(query: &str) -> Vec<QueryToken> {
    let mut tokens = Vec::new();
    let mut rest = query.trim_start();

    while let Some(first) = rest.chars().next() {
        if first == '|' {
            tokens.push(QueryToken::Or);
            rest = rest[1..].trim_start();
            continue;
        }

        let negated_phrase = rest.starts_with("-\"");
        if first == '"' || negated_phrase {
            // An unclosed quote runs to the end of the query, so a phrase can be typed
            // and searched for before the closing quote is reached
            let body = &rest[if negated_phrase { 2 } else { 1 }..];
            let (phrase, after) = body.split_once('"').unwrap_or((body, ""));
            let value = normalise(phrase);

            if !value.is_empty() {
                tokens.push(if negated_phrase {
                    QueryToken::ExcludeText(value)
                } else {
                    QueryToken::Term(SearchTerm::Contains(value))
                });
            }
            rest = after.trim_start();
            continue;
        }

        let end = rest.find(|c: char| c.is_whitespace() || c == '|').unwrap_or(rest.len());
        tokens.push(tokenise_word(&rest[..end]));
        rest = rest[end..].trim_start();
    }

    tokens
}

impl SearchQuery {
    /// Parse a query. A modifier is taken out first, then what's left is tokenised.
    pub fn parse(query: &str, settings: &UserSettings) -> Self {
        let lowered = query.to_lowercase();
        let modifiers = parse_search_modifiers(&lowered, settings);
        let remaining = remove_modifiers_from_query(&lowered);

        let mut parsed = Self { modifiers, ..Default::default() };
        let mut join_to_last_group = false;

        for token in tokenise_query(&remaining) {
            match token {
                QueryToken::Or => join_to_last_group = !parsed.groups.is_empty(),
                QueryToken::Term(term) => {
                    match parsed.groups.last_mut().filter(|_| join_to_last_group) {
                        Some(group) => group.push(term),
                        None => parsed.groups.push(vec![term]),
                    }
                    join_to_last_group = false;
                }
                // An exclusion always applies, so it ends any OR group
                QueryToken::ExcludeText(value) => {
                    parsed.exclude_text.push(value);
                    join_to_last_group = false;
                }
                QueryToken::ExcludeTag(value) => {
                    parsed.exclude_tags.push(value);
                    join_to_last_group = false;
                }
            }
        }

        parsed
    }

    /// What the query says about the archive, if anything.
    pub fn archived(&self) -> Option<bool> {
        self.modifiers.archived
    }

    /// Whether a note matches everything except the archive, which the caller decides
    /// from `archived` and the mode it's in.
    pub fn matches(&self, note: &Note, settings: &UserSettings) -> bool {
        self.matches_modifiers(note)
            && self.matches_colour(note, settings)
            && self.matches_text(note)
    }

    fn matches_modifiers(&self, note: &Note) -> bool {
        let modifiers = &self.modifiers;

        if modifiers.has_attachment && note.attachments.is_empty() {
            return false;
        }

        let created = note.created_at.format("%Y-%m-%d").to_string();
        let modified = note.modified_at.format("%Y-%m-%d").to_string();

        if modifiers.created_after.as_ref().is_some_and(|date| created < *date)
            || modifiers.created_before.as_ref().is_some_and(|date| created > *date)
            || modifiers.modified_after.as_ref().is_some_and(|date| modified < *date)
            || modifiers.modified_before.as_ref().is_some_and(|date| modified > *date)
        {
            return false;
        }

        if modifiers.word_count_min.is_some() || modifiers.word_count_max.is_some() {
            let word_count = note.content.split_whitespace().count();
            if modifiers.word_count_min.is_some_and(|min| word_count < min)
                || modifiers.word_count_max.is_some_and(|max| word_count > max)
            {
                return false;
            }
        }

        true
    }

    fn matches_colour(&self, note: &Note, settings: &UserSettings) -> bool {
        let colours = &self.modifiers.colors;
        if colours.is_empty() {
            return true;
        }

        let is_wanted = |colour: &str| colours.iter().any(|key| key.eq_ignore_ascii_case(colour));
        let note_has_colour = note.color.as_deref().is_some_and(is_wanted);
        let tag_has_colour = || note.tags.iter().any(|tag| settings.get_tag_color(tag).is_some_and(|c| is_wanted(&c)));

        match self.modifiers.color_target {
            ColorTarget::Note => note_has_colour,
            ColorTarget::Tag => tag_has_colour(),
            ColorTarget::Both => note_has_colour || tag_has_colour(),
        }
    }

    fn matches_text(&self, note: &Note) -> bool {
        let content = normalise(&note.content);
        let tags: Vec<String> = note.tags.iter().map(|tag| normalise(tag)).collect();

        if self.exclude_text.iter().any(|term| content.contains(term.as_str()))
            || self.exclude_tags.iter().any(|term| tags.iter().any(|tag| tag.contains(term.as_str())))
        {
            return false;
        }

        let text = format!("{content} {}", tags.join(" "));
        let words: Vec<&str> = regex!(r"[\s\p{P}]+").split(&text).filter(|word| !word.is_empty()).collect();

        self.groups.iter().all(|group| group.iter().any(|term| match term {
            SearchTerm::Contains(value) => text.contains(value.as_str()),
            SearchTerm::Prefix(value) => words.iter().any(|word| word.starts_with(value.as_str())),
            SearchTerm::Suffix(value) => words.iter().any(|word| word.ends_with(value.as_str())),
            SearchTerm::Tag(value) => tags.iter().any(|tag| tag.contains(value.as_str())),
        }))
    }
}

/// Parse search modifiers from a lowercased query.
fn parse_search_modifiers(query: &str, settings: &UserSettings) -> SearchModifiers {
    let mut modifiers = SearchModifiers::default();

    // has:attachment
    if query.contains("has:attachment") {
        modifiers.has_attachment = true;
    }

    // archived:true, or archived alone
    if let Some(caps) = regex!(r"\barchived?:(true|false|yes|no|1|0)\b").captures(query) {
        modifiers.archived = Some(matches!(&caps[1], "true" | "yes" | "1"));
    } else if regex!(r"\barchived?\b").is_match(query) {
        modifiers.archived = Some(true);
    }

    // created:>DATE
    if let Some(caps) = regex!(r"created:>(\d{4}-\d{2}-\d{2})").captures(query)
    {
        modifiers.created_after = Some(caps[1].to_string());
    }

    // created:<DATE
    if let Some(caps) = regex!(r"created:<(\d{4}-\d{2}-\d{2})").captures(query)
    {
        modifiers.created_before = Some(caps[1].to_string());
    }

    // created:DATE..DATE (range)
    if let Some(caps) = regex!(r"created:(\d{4}-\d{2}-\d{2})\.\.(\d{4}-\d{2}-\d{2})").captures(query)
    {
        modifiers.created_after = Some(caps[1].to_string());
        modifiers.created_before = Some(caps[2].to_string());
    }

    // modified:>DATE
    if let Some(caps) = regex!(r"modified:>(\d{4}-\d{2}-\d{2})").captures(query)
    {
        modifiers.modified_after = Some(caps[1].to_string());
    }

    // modified:<DATE
    if let Some(caps) = regex!(r"modified:<(\d{4}-\d{2}-\d{2})").captures(query)
    {
        modifiers.modified_before = Some(caps[1].to_string());
    }

    // modified:DATE..DATE (range)
    if let Some(caps) = regex!(r"modified:(\d{4}-\d{2}-\d{2})\.\.(\d{4}-\d{2}-\d{2})").captures(query)
    {
        modifiers.modified_after = Some(caps[1].to_string());
        modifiers.modified_before = Some(caps[2].to_string());
    }

    // words:>N
    if let Some(caps) = regex!(r"words:>(\d+)").captures(query)
    {
        modifiers.word_count_min = caps[1].parse().ok();
    }

    // words:<N
    if let Some(caps) = regex!(r"words:<(\d+)").captures(query)
    {
        modifiers.word_count_max = caps[1].parse().ok();
    }

    // words:N..N (range)
    if let Some(caps) = regex!(r"words:(\d+)\.\.(\d+)").captures(query)
    {
        modifiers.word_count_min = caps[1].parse().ok();
        modifiers.word_count_max = caps[2].parse().ok();
    }

    // color:red note (notes with red color)
    if let Some(caps) = regex!(r"(?i)\bcolou?r:([a-z0-9#]+)\s+note\b").captures(query)
    {
        modifiers.colors.push(caps[1].to_lowercase());
        modifiers.color_target = ColorTarget::Note;
    }

    // color:red tag (notes with red-colored tags)
    if let Some(caps) = regex!(r"(?i)\bcolou?r:([a-z0-9#]+)\s+tag\b").captures(query)
    {
        modifiers.colors.push(caps[1].to_lowercase());
        modifiers.color_target = ColorTarget::Tag;
    }

    // color:red (both notes and tags)
    if modifiers.colors.is_empty() {
        if let Some(caps) = regex!(r"(?i)\bcolou?r:([a-z0-9#]+)\b").captures(query)
        {
            modifiers.colors.push(caps[1].to_lowercase());
            modifiers.color_target = ColorTarget::Both;
        }
    }

    // category:important note (notes with category named "important")
    if let Some(caps) = regex!(r"(?i)\bcategory:([\w-]+)\s+note\b").captures(query)
    {
        let category_name = caps[1].trim();
        // Look up color key by display name
        if let Some(color_key) = settings.get_color_key_by_display_name(category_name) {
            modifiers.colors.push(color_key);
            modifiers.color_target = ColorTarget::Note;
        }
    }

    // category:important tag (notes with tags in category named "important")
    if let Some(caps) = regex!(r"(?i)\bcategory:([\w-]+)\s+tag\b").captures(query)
    {
        let category_name = caps[1].trim();
        if let Some(color_key) = settings.get_color_key_by_display_name(category_name) {
            modifiers.colors.push(color_key);
            modifiers.color_target = ColorTarget::Tag;
        }
    }

    // category:important (both notes and tags)
    if modifiers.colors.is_empty() {
        if let Some(caps) = regex!(r"(?i)\bcategory:([\w-]+)\b").captures(query)
        {
            let category_name = caps[1].trim();
            if let Some(color_key) = settings.get_color_key_by_display_name(category_name) {
                modifiers.colors.push(color_key);
                modifiers.color_target = ColorTarget::Both;
            }
        }
    }

    modifiers
}

/// Remove search modifiers from query string, leaving only text/tag search terms
fn remove_modifiers_from_query(query: &str) -> String {
    // Remove all modifier patterns
    let patterns = [
        regex!(r"has:attachment"),
        regex!(r"\barchived?:(true|false|yes|no|1|0)\b"),
        regex!(r"\barchived?\b"),
        regex!(r"created:>\d{4}-\d{2}-\d{2}"),
        regex!(r"created:<\d{4}-\d{2}-\d{2}"),
        regex!(r"created:\d{4}-\d{2}-\d{2}\.\.\d{4}-\d{2}-\d{2}"),
        regex!(r"modified:>\d{4}-\d{2}-\d{2}"),
        regex!(r"modified:<\d{4}-\d{2}-\d{2}"),
        regex!(r"modified:\d{4}-\d{2}-\d{2}\.\.\d{4}-\d{2}-\d{2}"),
        regex!(r"words:>\d+"),
        regex!(r"words:<\d+"),
        regex!(r"words:\d+\.\.\d+"),
        regex!(r"(?i)\bcolou?r:[a-z0-9#]+\s+note\b"),
        regex!(r"(?i)\bcolou?r:[a-z0-9#]+\s+tag\b"),
        regex!(r"(?i)\bcolou?r:[a-z0-9#]+\b"),
        regex!(r"(?i)\bcategory:[\w-]+\s+note\b"),
        regex!(r"(?i)\bcategory:[\w-]+\s+tag\b"),
        regex!(r"(?i)\bcategory:[\w-]+\b"),
    ];

    let mut result = query.to_string();
    for pattern in patterns {
        result = pattern.replace_all(&result, "").to_string();
    }

    // Clean up extra whitespace
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::settings::{ColorDefinition, ColorPalette};
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Fixture {
        notes: Vec<FixtureNote>,
        cases: Vec<FixtureCase>,
    }

    #[derive(Deserialize)]
    struct FixtureNote {
        id: String,
        content: String,
        tags: Vec<String>,
    }

    #[derive(Deserialize)]
    struct FixtureCase {
        query: String,
        expected: Vec<String>,
    }

    /// The cases every client runs, so a saved search means the same thing everywhere.
    #[test]
    fn the_shared_query_fixture_passes() {
        let fixture: Fixture = serde_json::from_str(include_str!("../../docs/search-query-fixture.json")).unwrap();
        let settings = UserSettings::default();

        let notes: Vec<(String, Note)> = fixture.notes.iter()
            .map(|entry| {
                let mut note = Note::new(entry.content.clone());
                note.tags = entry.tags.clone();
                (entry.id.clone(), note)
            })
            .collect();

        let mut failures = Vec::new();
        for case in &fixture.cases {
            let query = SearchQuery::parse(&case.query, &settings);
            let mut found: Vec<&str> = notes.iter()
                .filter(|(_, note)| query.matches(note, &settings))
                .map(|(id, _)| id.as_str())
                .collect();
            found.sort();

            if found != case.expected {
                failures.push(format!("{:?}: found {found:?}, expected {:?}", case.query, case.expected));
            }
        }

        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    fn note_with(content: &str, tags: &[&str]) -> Note {
        let mut note = Note::new(content.to_string());
        note.tags = tags.iter().map(|tag| tag.to_string()).collect();
        note
    }

    fn matches(query: &str, note: &Note) -> bool {
        let settings = UserSettings::default();
        SearchQuery::parse(query, &settings).matches(note, &settings)
    }

    #[test]
    fn decomposed_text_matches_as_if_it_were_composed() {
        assert!(matches("café", &note_with("un cafe\u{301}", &[])));
    }

    #[test]
    fn archived_is_a_modifier_not_a_word() {
        let settings = UserSettings::default();

        assert_eq!(SearchQuery::parse("archived", &settings).archived(), Some(true));
        assert_eq!(SearchQuery::parse("archived:false dog", &settings).archived(), Some(false));
        assert_eq!(SearchQuery::parse("archive:yes", &settings).archived(), Some(true));
        assert_eq!(SearchQuery::parse("dog", &settings).archived(), None);

        assert!(matches("archived dog", &note_with("a dog", &[])));
    }

    #[test]
    fn modifiers_and_text_combine() {
        let mut note = note_with("one two three", &["work"]);
        note.created_at = "2024-06-01T00:00:00Z".parse().unwrap();

        assert!(matches("#work created:>2024-01-01", &note));
        assert!(!matches("#work created:>2025-01-01", &note));
        assert!(matches("words:>2 two", &note));
        assert!(!matches("words:<2 two", &note));
        assert!(!matches("has:attachment", &note));
    }

    fn settings_with_colours() -> UserSettings {
        let mut settings = UserSettings::default();
        let mut palette = ColorPalette::new();
        palette.insert("red".to_string(), ColorDefinition {
            light: "#ff0000".to_string(),
            dark: "#ff0000".to_string(),
            display_name: Some("Important".to_string()),
        });
        settings.color_palette = Some(palette);
        settings
    }

    #[test]
    fn a_colour_on_a_note_or_a_tag_counts() {
        let mut settings = settings_with_colours();
        settings.tag_colors = Some([("urgent".to_string(), "red".to_string())].into());

        let mut coloured = note_with("text", &[]);
        coloured.color = Some("red".to_string());
        let tagged = note_with("text", &["urgent"]);
        let plain = note_with("text", &[]);

        let query = SearchQuery::parse("color:red", &settings);
        assert!(query.matches(&coloured, &settings));
        assert!(query.matches(&tagged, &settings));
        assert!(!query.matches(&plain, &settings));

        let query = SearchQuery::parse("color:red note", &settings);
        assert!(query.matches(&coloured, &settings));
        assert!(!query.matches(&tagged, &settings));

        let query = SearchQuery::parse("color:red tag", &settings);
        assert!(!query.matches(&coloured, &settings));
        assert!(query.matches(&tagged, &settings));
    }

    #[test]
    fn test_parse_color_search_note_qualifier() {
        let modifiers = parse_search_modifiers("color:blue note", &settings_with_colours());

        assert_eq!(modifiers.colors, vec!["blue"]);
        assert_eq!(modifiers.color_target, ColorTarget::Note);
    }

    #[test]
    fn test_parse_category_search_simple() {
        let modifiers = parse_search_modifiers("category:important", &settings_with_colours());

        // "important" should map to "red" via display name
        assert_eq!(modifiers.colors, vec!["red"]);
        assert_eq!(modifiers.color_target, ColorTarget::Both);
    }

    #[test]
    fn test_parse_category_search_note_qualifier() {
        let modifiers = parse_search_modifiers("category:important note", &settings_with_colours());

        assert_eq!(modifiers.colors, vec!["red"]);
        assert_eq!(modifiers.color_target, ColorTarget::Note);
    }

    #[test]
    fn test_parse_category_search_tag_qualifier() {
        let modifiers = parse_search_modifiers("category:important tag", &settings_with_colours());

        assert_eq!(modifiers.colors, vec!["red"]);
        assert_eq!(modifiers.color_target, ColorTarget::Tag);
    }

    #[test]
    fn test_parse_category_search_invalid() {
        let modifiers = parse_search_modifiers("category:nonexistent", &settings_with_colours());

        // Should not find any colors for nonexistent category
        assert!(modifiers.colors.is_empty());
    }

    #[test]
    fn test_parse_single_color_modifier() {
        let modifiers = parse_search_modifiers("color:red some text", &settings_with_colours());

        // Should capture the color
        assert_eq!(modifiers.colors.len(), 1);
        assert_eq!(modifiers.colors[0], "red");
        assert_eq!(modifiers.color_target, ColorTarget::Both);
    }

    #[test]
    fn test_remove_modifiers_from_query() {
        let cleaned = remove_modifiers_from_query("color:red important meeting");
        assert!(!cleaned.contains("color:red"));
        assert!(cleaned.contains("important"));
        assert!(cleaned.contains("meeting"));
    }

    #[test]
    fn test_remove_category_modifiers_from_query() {
        let cleaned = remove_modifiers_from_query("category:important urgent task");
        assert!(!cleaned.contains("category:important"));
        assert!(cleaned.contains("urgent"));
        assert!(cleaned.contains("task"));
    }
}
