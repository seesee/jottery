//! Helper functions for markdown processing
//!
//! This module provides utilities for stripping and rendering markdown
//! content in the terminal using pulldown-cmark and ratatui.

use std::fs::File;
use std::io::Write;
use std::sync::{Arc, Mutex};
use ratatui::text::Line;

/// Result of rendering markdown, includes lines and link positions
#[derive(Debug, Default)]
pub struct MarkdownRenderResult {
    /// Rendered lines
    pub lines: Vec<Line<'static>>,
    /// Note link positions: (note_id, line_index, char_start, char_end)
    /// line_index is relative to the rendered lines (0-based)
    pub note_links: Vec<(String, usize, usize, usize)>,
}

/// Strip markdown formatting from text (for display in note list and filenames)
pub fn strip_markdown(text: &str) -> String {
    let mut result = text.to_string();

    // Remove markdown headers (# ## ### etc.) at the start of the line
    if result.starts_with('#') {
        let mut chars = result.chars();
        // Skip all # characters
        while matches!(chars.as_str().chars().next(), Some('#')) {
            chars.next();
        }
        result = chars.as_str().trim_start().to_string();
    }

    // Remove horizontal rules (---, ***, ___)
    result = result.replace("---", "").replace("***", "").replace("___", "");

    // Remove blockquote markers (> text)
    if result.starts_with('>') {
        result = result.trim_start_matches('>').trim_start().to_string();
    }

    // Remove list markers (-, *, +, 1., 2., etc.)
    let list_marker_patterns = [
        "- ", "* ", "+ ",
        "1. ", "2. ", "3. ", "4. ", "5. ", "6. ", "7. ", "8. ", "9. ",
    ];
    for marker in &list_marker_patterns {
        if result.starts_with(marker) {
            result = result.trim_start_matches(marker).to_string();
            break;
        }
    }

    // Remove task list markers (- [ ] or - [x])
    if result.starts_with("- [ ] ") || result.starts_with("- [x] ") {
        result = result.replacen("- [ ] ", "", 1).replacen("- [x] ", "", 1);
    }

    // Remove bold (**text** or __text__)
    result = result.replace("**", "").replace("__", "");

    // Remove strikethrough (~~text~~)
    result = result.replace("~~", "");

    // Remove inline code backticks and links
    let mut cleaned = String::new();
    let mut chars = result.chars().peekable();
    let mut in_code = false;

    while let Some(ch) = chars.next() {
        match ch {
            '`' => {
                // Skip backticks (inline code markers)
                in_code = !in_code;
            }
            '*' | '_' if !in_code => {
                // Skip single * or _ used for emphasis
                continue;
            }
            '!' if !in_code && chars.peek() == Some(&'[') => {
                // Handle images ![alt](url) - skip entirely
                chars.next(); // skip [
                for c in chars.by_ref() {
                    if c == ']' {
                        break;
                    }
                }
                // Skip (url) part
                if chars.peek() == Some(&'(') {
                    chars.next(); // skip (
                    for c in chars.by_ref() {
                        if c == ')' {
                            break;
                        }
                    }
                }
            }
            '[' if !in_code => {
                // Handle links [text](url) - extract text only
                let mut link_text = String::new();
                let mut found_closing = false;

                for c in chars.by_ref() {
                    if c == ']' {
                        found_closing = true;
                        break;
                    }
                    link_text.push(c);
                }

                if found_closing {
                    // Skip the (url) or [ref] part
                    if chars.peek() == Some(&'(') {
                        chars.next(); // skip (
                        for c in chars.by_ref() {
                            if c == ')' {
                                break;
                            }
                        }
                    } else if chars.peek() == Some(&'[') {
                        chars.next(); // skip [
                        for c in chars.by_ref() {
                            if c == ']' {
                                break;
                            }
                        }
                    }
                    cleaned.push_str(&link_text);
                } else {
                    cleaned.push('[');
                    cleaned.push_str(&link_text);
                }
            }
            '<' if !in_code => {
                // Remove HTML tags
                for c in chars.by_ref() {
                    if c == '>' {
                        break;
                    }
                }
            }
            _ => {
                if !in_code || ch != '`' {
                    cleaned.push(ch);
                }
            }
        }
    }

    // Normalize whitespace
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Render markdown for terminal display using pulldown-cmark parser
/// Returns both rendered lines and positions of note links for click detection
pub fn render_markdown_for_terminal(
    content: &str,
    syntax_highlighter: &crate::ui::syntax::SyntaxHighlighter,
    debug_log: &Option<Arc<Mutex<File>>>
) -> MarkdownRenderResult {
    use pulldown_cmark::{Parser, Event, Tag, TagEnd, CodeBlockKind, Options};
    use ratatui::style::{Style, Modifier, Color};
    use ratatui::text::{Line, Span};
    use crate::models::SyntaxLanguage;

    // Helper to write debug messages
    let log_debug = |msg: &str| {
        if let Some(log) = debug_log {
            if let Ok(mut file) = log.lock() {
                let _ = writeln!(file, "[{}] {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"), msg);
            }
        }
    };

    let mut result = MarkdownRenderResult::default();
    let mut current_line_spans: Vec<Span<'static>> = Vec::new();
    let mut current_style = Style::default();
    let mut in_code_block = false;
    let mut code_block_content = String::new();
    let mut code_block_lang = String::new();
    let mut in_table = false;
    let mut in_table_head = false;
    let mut in_list_item = false;
    let mut list_item_started = false;
    let mut table_cells: Vec<String> = Vec::new();
    let mut current_cell_text = String::new();
    let mut table_header_rows: Vec<Vec<String>> = Vec::new();
    let mut table_body_rows: Vec<Vec<String>> = Vec::new();

    // Track current note link being processed
    let mut current_link_url: Option<String> = None;
    let mut current_link_start_char: usize = 0;

    // Style stack for nested formatting
    let mut style_stack: Vec<Style> = vec![Style::default()];

    // Helper to calculate current character position in line
    let calc_current_char_pos = |spans: &[Span]| -> usize {
        spans.iter().map(|s| s.content.chars().count()).sum()
    };

    // Enable extensions: tables, strikethrough, tasklists, footnotes
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(content, options);

    for event in parser {
        match event {
            Event::Start(tag) => {
                match tag {
                    Tag::Heading { .. } => {
                        // Flush any current line before heading
                        if !current_line_spans.is_empty() {
                            result.lines.push(Line::from(current_line_spans.clone()));
                            current_line_spans.clear();
                        }
                        // Add blank line before heading (for spacing)
                        if !result.lines.is_empty() {
                            result.lines.push(Line::raw(""));
                        }

                        // Headers in cyan + bold
                        let header_style = Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD);
                        style_stack.push(header_style);
                        current_style = header_style;
                    }
                    Tag::Emphasis => {
                        // Italic -> cyan color (better terminal support than ITALIC modifier)
                        let italic_style = current_style.fg(Color::Cyan);
                        style_stack.push(italic_style);
                        current_style = italic_style;
                    }
                    Tag::Strong => {
                        // Bold -> white + BOLD
                        let bold_style = current_style
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD);
                        style_stack.push(bold_style);
                        current_style = bold_style;
                    }
                    Tag::Link { dest_url, .. } => {
                        // Check if this is a note link (link:UUID format)
                        let url = dest_url.to_string();
                        if url.starts_with("link:") {
                            // Note link -> yellow + underlined (distinct from regular links)
                            let link_style = current_style
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::UNDERLINED);
                            style_stack.push(link_style);
                            current_style = link_style;
                            // Track this link for position recording
                            current_link_url = Some(url);
                            current_link_start_char = calc_current_char_pos(&current_line_spans);
                        } else {
                            // Regular links -> blue + underlined
                            let link_style = current_style
                                .fg(Color::Blue)
                                .add_modifier(Modifier::UNDERLINED);
                            style_stack.push(link_style);
                            current_style = link_style;
                        }
                    }
                    Tag::CodeBlock(kind) => {
                        // Flush current line before code block
                        if !current_line_spans.is_empty() {
                            result.lines.push(Line::from(current_line_spans.clone()));
                            current_line_spans.clear();
                        }

                        in_code_block = true;
                        code_block_content.clear();
                        code_block_lang = match kind {
                            CodeBlockKind::Fenced(lang) => lang.to_string(),
                            CodeBlockKind::Indented => String::new(),
                        };
                    }
                    Tag::Table(_) => {
                        // Flush current line before table
                        if !current_line_spans.is_empty() {
                            result.lines.push(Line::from(current_line_spans.clone()));
                            current_line_spans.clear();
                        }
                        in_table = true;
                        table_header_rows.clear();
                        table_body_rows.clear();
                        log_debug(">>> Tag::Table start");
                    }
                    Tag::TableHead => {
                        in_table_head = true;
                        log_debug(">>> Tag::TableHead start (setting in_table_head=true)");
                    }
                    Tag::TableRow => {
                        table_cells.clear();
                        log_debug(&format!(">>> Tag::TableRow start (in_table_head={})", in_table_head));
                    }
                    Tag::TableCell => {
                        current_cell_text.clear();
                    }
                    Tag::Paragraph => {
                        // Paragraphs just group text, no special styling needed
                    }
                    Tag::List(_) => {
                        // List started - no special action needed
                    }
                    Tag::Item => {
                        // Flush current line before list item
                        if !current_line_spans.is_empty() {
                            result.lines.push(Line::from(current_line_spans.clone()));
                            current_line_spans.clear();
                        }
                        in_list_item = true;
                        list_item_started = false;
                    }
                    _ => {}
                }
            }
            Event::End(tag_end) => {
                match tag_end {
                    TagEnd::Heading(_) => {
                        // Pop style from stack
                        if style_stack.len() > 1 {
                            style_stack.pop();
                            current_style = *style_stack.last().unwrap();
                        }
                        // Headings end with a line break
                        if !current_line_spans.is_empty() {
                            result.lines.push(Line::from(current_line_spans.clone()));
                            current_line_spans.clear();
                        }
                    }
                    TagEnd::Emphasis | TagEnd::Strong => {
                        // Pop style from stack
                        if style_stack.len() > 1 {
                            style_stack.pop();
                            current_style = *style_stack.last().unwrap();
                        }
                    }
                    TagEnd::Link => {
                        // Record note link position if this was a note link
                        if let Some(url) = current_link_url.take() {
                            if url.starts_with("link:") {
                                let note_id = url.strip_prefix("link:").unwrap_or("").to_string();
                                if !note_id.is_empty() {
                                    let end_char = calc_current_char_pos(&current_line_spans);
                                    let line_index = result.lines.len();
                                    result.note_links.push((note_id, line_index, current_link_start_char, end_char));
                                }
                            }
                        }
                        // Pop style from stack
                        if style_stack.len() > 1 {
                            style_stack.pop();
                            current_style = *style_stack.last().unwrap();
                        }
                    }
                    TagEnd::CodeBlock => {
                        in_code_block = false;

                        // Determine syntax language
                        let lang = match code_block_lang.to_lowercase().as_str() {
                            "javascript" | "js" => SyntaxLanguage::Javascript,
                            "python" | "py" => SyntaxLanguage::Python,
                            "markdown" | "md" => SyntaxLanguage::Markdown,
                            "json" => SyntaxLanguage::Json,
                            "html" => SyntaxLanguage::Html,
                            "css" => SyntaxLanguage::Css,
                            "sql" => SyntaxLanguage::Sql,
                            "bash" | "sh" => SyntaxLanguage::Bash,
                            "perl" | "pl" => SyntaxLanguage::Perl,
                            _ => SyntaxLanguage::Plain,
                        };

                        // Apply syntax highlighting
                        let highlighted = syntax_highlighter.highlight(&code_block_content, lang);

                        // Convert borrowed lines to owned
                        for line in highlighted.lines {
                            let owned_line: Line<'static> = Line::from(
                                line.spans.into_iter()
                                    .map(|span| Span::styled(span.content.to_string(), span.style))
                                    .collect::<Vec<_>>()
                            );
                            result.lines.push(owned_line);
                        }

                        code_block_content.clear();
                        code_block_lang.clear();
                        // Add blank line after code block
                        result.lines.push(Line::raw(""));
                    }
                    TagEnd::TableCell => {
                        // Save the current cell text
                        table_cells.push(current_cell_text.clone());
                        current_cell_text.clear();
                    }
                    TagEnd::TableRow => {
                        // Save the current row to appropriate vector
                        if !table_cells.is_empty() {
                            log_debug(&format!("TableRow end - in_table_head={}, cells={:?}", in_table_head, table_cells));
                            if in_table_head {
                                table_header_rows.push(table_cells.clone());
                            } else {
                                table_body_rows.push(table_cells.clone());
                            }
                            table_cells.clear();
                        }
                    }
                    TagEnd::TableHead => {
                        // Save header cells (they come directly in TableHead, not in a TableRow)
                        if !table_cells.is_empty() {
                            log_debug(&format!(">>> TagEnd::TableHead - saving header cells: {:?}", table_cells));
                            table_header_rows.push(table_cells.clone());
                            table_cells.clear();
                        }
                        in_table_head = false;
                        log_debug(">>> TagEnd::TableHead (setting in_table_head=false)");
                    }
                    TagEnd::Table => {
                        in_table = false;
                        log_debug(&format!("Table end - header_rows={}, body_rows={}", table_header_rows.len(), table_body_rows.len()));

                        // Calculate column widths from both header and body
                        let mut col_widths: Vec<usize> = Vec::new();

                        // Check header rows
                        for row in &table_header_rows {
                            for (i, cell) in row.iter().enumerate() {
                                let width = cell.len();
                                if i >= col_widths.len() {
                                    col_widths.push(width);
                                } else if width > col_widths[i] {
                                    col_widths[i] = width;
                                }
                            }
                        }

                        // Check body rows
                        for row in &table_body_rows {
                            for (i, cell) in row.iter().enumerate() {
                                let width = cell.len();
                                if i >= col_widths.len() {
                                    col_widths.push(width);
                                } else if width > col_widths[i] {
                                    col_widths[i] = width;
                                }
                            }
                        }

                        // Render header rows (bold)
                        for row in &table_header_rows {
                            let mut row_text = String::from("| ");
                            for (i, cell) in row.iter().enumerate() {
                                let width = if i < col_widths.len() { col_widths[i] } else { 0 };
                                row_text.push_str(&format!("{:<width$} | ", cell, width = width));
                            }
                            result.lines.push(Line::styled(
                                row_text,
                                Style::default().add_modifier(Modifier::BOLD)
                            ));
                        }

                        // Add separator line after header
                        if !table_header_rows.is_empty() {
                            let mut separator = String::from("|-");
                            for &width in &col_widths {
                                separator.push_str(&"-".repeat(width));
                                separator.push_str("-|-");
                            }
                            // Remove trailing dash
                            separator.pop();
                            result.lines.push(Line::styled(separator, Style::default().fg(Color::DarkGray)));
                        }

                        // Render body rows (normal)
                        for row in &table_body_rows {
                            let mut row_text = String::from("| ");
                            for (i, cell) in row.iter().enumerate() {
                                let width = if i < col_widths.len() { col_widths[i] } else { 0 };
                                row_text.push_str(&format!("{:<width$} | ", cell, width = width));
                            }
                            result.lines.push(Line::raw(row_text));
                        }

                        table_header_rows.clear();
                        table_body_rows.clear();
                        // Add blank line after table
                        result.lines.push(Line::raw(""));
                    }
                    TagEnd::Paragraph => {
                        // End paragraph with a line break
                        if !current_line_spans.is_empty() {
                            result.lines.push(Line::from(current_line_spans.clone()));
                            current_line_spans.clear();
                        }
                        // Don't add extra blank lines - let block spacing handle it
                    }
                    TagEnd::Item => {
                        // End list item with line break
                        in_list_item = false;
                        if !current_line_spans.is_empty() {
                            result.lines.push(Line::from(current_line_spans.clone()));
                            current_line_spans.clear();
                        }
                    }
                    TagEnd::List(_) => {
                        // Add blank line after list
                        result.lines.push(Line::raw(""));
                    }
                    _ => {}
                }
            }
            Event::Text(text) => {
                if in_code_block {
                    code_block_content.push_str(&text);
                } else if in_table {
                    // Collect text for current table cell
                    current_cell_text.push_str(&text);
                } else if in_list_item && !list_item_started {
                    // Add bullet point at start of list item
                    list_item_started = true;
                    current_line_spans.push(Span::raw("• "));
                    current_line_spans.push(Span::styled(text.to_string(), current_style));
                } else {
                    current_line_spans.push(Span::styled(text.to_string(), current_style));
                }
            }
            Event::Code(code) => {
                if in_table {
                    // In tables, just add code text without styling
                    current_cell_text.push_str(&code);
                } else {
                    // Add bullet point at start of list item if not started
                    if in_list_item && !list_item_started {
                        list_item_started = true;
                        current_line_spans.push(Span::raw("• "));
                    }
                    // Inline code -> yellow
                    current_line_spans.push(Span::styled(
                        code.to_string(),
                        Style::default().fg(Color::Yellow)
                    ));
                }
            }
            Event::SoftBreak => {
                // Soft break in markdown (single newline) - keep on same line in most contexts
                if in_table {
                    // In tables, add space to cell text
                    current_cell_text.push(' ');
                } else {
                    // Add a space for soft breaks in regular text
                    current_line_spans.push(Span::raw(" "));
                }
            }
            Event::HardBreak => {
                // Hard break (two spaces + newline or <br>) - always break line
                if !current_line_spans.is_empty() {
                    result.lines.push(Line::from(current_line_spans.clone()));
                    current_line_spans.clear();
                }
            }
            Event::Rule => {
                // Flush current line before rule
                if !current_line_spans.is_empty() {
                    result.lines.push(Line::from(current_line_spans.clone()));
                    current_line_spans.clear();
                }
                // Horizontal rule
                result.lines.push(Line::styled(
                    "─".repeat(80),
                    Style::default().fg(Color::DarkGray)
                ));
                result.lines.push(Line::raw(""));
            }
            Event::TaskListMarker(checked) => {
                // Render task list checkbox
                let checkbox = if checked { "[x] " } else { "[ ] " };
                current_line_spans.push(Span::raw(checkbox));
            }
            _ => {}
        }
    }

    // Flush any remaining spans
    if !current_line_spans.is_empty() {
        result.lines.push(Line::from(current_line_spans));
    }

    // Return empty line if no content
    if result.lines.is_empty() {
        result.lines.push(Line::raw(""));
    }

    result
}
