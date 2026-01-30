//! Syntax highlighting utilities
//!
//! This module provides syntax highlighting functionality for code blocks and markdown
//! rendering using syntect and pulldown-cmark.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use syntect::easy::HighlightLines;
use syntect::highlighting::{ThemeSet, Theme};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

use crate::models::SyntaxLanguage;
use super::calc;

/// Syntax highlighter for note content
pub struct SyntaxHighlighter {
    syntax_set: SyntaxSet,
    theme: Theme,
}

impl SyntaxHighlighter {
    /// Create a new syntax highlighter with default theme
    pub fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();

        // Use InspiredGitHub theme (light) or base16-ocean.dark (dark)
        // For now, we'll use a theme that works well in terminals
        let theme = theme_set.themes.get("base16-ocean.dark")
            .or_else(|| theme_set.themes.get("Solarized (dark)"))
            .unwrap_or_else(|| theme_set.themes.values().next().unwrap())
            .clone();

        Self {
            syntax_set,
            theme,
        }
    }

    /// Highlight text with syntax highlighting or render markdown/calc/outliner
    pub fn highlight<'a>(&self, text: &'a str, language: SyntaxLanguage) -> Text<'a> {
        // Special handling for markdown - render it instead of highlighting
        if language == SyntaxLanguage::Markdown {
            return self.render_markdown(text);
        }

        // Special handling for calc - evaluate and render with results
        if language == SyntaxLanguage::Calc {
            return calc::render_calc(text);
        }

        // Special handling for outliner - render as tree structure
        if language == SyntaxLanguage::Outliner {
            return self.render_outliner(text);
        }

        // Map our SyntaxLanguage enum to syntect syntax names
        let syntax_name = match language {
            SyntaxLanguage::Plain => return Text::raw(text),
            SyntaxLanguage::Javascript => "JavaScript",
            SyntaxLanguage::Python => "Python",
            SyntaxLanguage::Markdown => "Markdown", // Won't reach here due to above check
            SyntaxLanguage::Json => "JSON",
            SyntaxLanguage::Html => "HTML",
            SyntaxLanguage::Css => "CSS",
            SyntaxLanguage::Sql => "SQL",
            SyntaxLanguage::Bash => "Bash",
            SyntaxLanguage::Perl => "Perl",
            SyntaxLanguage::Calc => "Plain Text", // Won't reach here due to above check
            SyntaxLanguage::Outliner => "Plain Text", // Won't reach here due to above check
        };

        // Try to find the syntax definition
        let syntax = match self.syntax_set.find_syntax_by_name(syntax_name) {
            Some(s) => s,
            None => {
                // Fallback to plain text if syntax not found
                return Text::raw(text);
            }
        };

        let mut highlighter = HighlightLines::new(syntax, &self.theme);
        let mut lines = Vec::new();

        for line in LinesWithEndings::from(text) {
            let highlighted = match highlighter.highlight_line(line, &self.syntax_set) {
                Ok(h) => h,
                Err(_) => {
                    // On error, just return the line as plain text
                    lines.push(Line::raw(line));
                    continue;
                }
            };

            let spans: Vec<Span> = highlighted
                .iter()
                .map(|(style, text)| {
                    let fg = syntect_to_ratatui_color(style.foreground);
                    let mut ratatui_style = Style::default().fg(fg);

                    // Apply text styles (bold, italic, underline)
                    if style.font_style.contains(syntect::highlighting::FontStyle::BOLD) {
                        ratatui_style = ratatui_style.add_modifier(Modifier::BOLD);
                    }
                    if style.font_style.contains(syntect::highlighting::FontStyle::ITALIC) {
                        ratatui_style = ratatui_style.add_modifier(Modifier::ITALIC);
                    }
                    if style.font_style.contains(syntect::highlighting::FontStyle::UNDERLINE) {
                        ratatui_style = ratatui_style.add_modifier(Modifier::UNDERLINED);
                    }

                    Span::styled(*text, ratatui_style)
                })
                .collect();

            lines.push(Line::from(spans));
        }

        Text::from(lines)
    }

    /// Render markdown with appropriate terminal styling
    fn render_markdown<'a>(&self, text: &'a str) -> Text<'a> {
        let mut options = Options::empty();
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_TASKLISTS);

        let parser = Parser::new_ext(text, options);
        let mut lines: Vec<Line> = Vec::new();
        let mut current_line: Vec<Span> = Vec::new();
        let mut style_stack: Vec<Style> = vec![Style::default()];
        let mut list_level: usize = 0;
        let mut in_code_block = false;

        for event in parser {
            match event {
                Event::Start(tag) => {
                    match tag {
                        Tag::Heading { level, .. } => {
                            // Flush current line before heading
                            if !current_line.is_empty() {
                                lines.push(Line::from(std::mem::take(&mut current_line)));
                            }

                            // Add styling for heading based on level
                            let heading_style = match level {
                                HeadingLevel::H1 => Style::default()
                                    .fg(Color::Cyan)
                                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                                HeadingLevel::H2 => Style::default()
                                    .fg(Color::Cyan)
                                    .add_modifier(Modifier::BOLD),
                                HeadingLevel::H3 => Style::default()
                                    .fg(Color::Blue)
                                    .add_modifier(Modifier::BOLD),
                                _ => Style::default()
                                    .fg(Color::Blue),
                            };
                            style_stack.push(heading_style);
                        }
                        Tag::Paragraph => {
                            // Paragraphs start on a new line
                            if !current_line.is_empty() || !lines.is_empty() {
                                lines.push(Line::from(std::mem::take(&mut current_line)));
                            }
                        }
                        Tag::List(_) => {
                            list_level += 1;
                        }
                        Tag::Item => {
                            // Flush current line
                            if !current_line.is_empty() {
                                lines.push(Line::from(std::mem::take(&mut current_line)));
                            }
                            // Add list bullet with indentation
                            let indent = "  ".repeat(list_level.saturating_sub(1));
                            current_line.push(Span::styled(
                                format!("{}• ", indent),
                                Style::default().fg(Color::Yellow),
                            ));
                        }
                        Tag::Emphasis => {
                            style_stack.push(
                                style_stack.last().unwrap_or(&Style::default())
                                    .add_modifier(Modifier::ITALIC),
                            );
                        }
                        Tag::Strong => {
                            style_stack.push(
                                style_stack.last().unwrap_or(&Style::default())
                                    .add_modifier(Modifier::BOLD),
                            );
                        }
                        Tag::Strikethrough => {
                            style_stack.push(
                                style_stack.last().unwrap_or(&Style::default())
                                    .add_modifier(Modifier::CROSSED_OUT),
                            );
                        }
                        Tag::Link { .. } => {
                            style_stack.push(
                                Style::default()
                                    .fg(Color::Blue)
                                    .add_modifier(Modifier::UNDERLINED),
                            );
                        }
                        Tag::CodeBlock(_) => {
                            in_code_block = true;
                            // Flush current line
                            if !current_line.is_empty() {
                                lines.push(Line::from(std::mem::take(&mut current_line)));
                            }
                            style_stack.push(Style::default().fg(Color::Green));
                        }
                        Tag::BlockQuote(_) => {
                            style_stack.push(Style::default().fg(Color::Magenta));
                        }
                        _ => {}
                    }
                }
                Event::End(tag_end) => {
                    match tag_end {
                        TagEnd::Heading(_) => {
                            style_stack.pop();
                            // End heading line
                            if !current_line.is_empty() {
                                lines.push(Line::from(std::mem::take(&mut current_line)));
                                lines.push(Line::raw("")); // Blank line after heading
                            }
                        }
                        TagEnd::Paragraph => {
                            // End paragraph
                            if !current_line.is_empty() {
                                lines.push(Line::from(std::mem::take(&mut current_line)));
                            }
                        }
                        TagEnd::List(_) => {
                            list_level = list_level.saturating_sub(1);
                        }
                        TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough
                        | TagEnd::Link | TagEnd::BlockQuote(_) => {
                            style_stack.pop();
                        }
                        TagEnd::CodeBlock => {
                            in_code_block = false;
                            style_stack.pop();
                        }
                        _ => {}
                    }
                }
                Event::Text(t) => {
                    let style = *style_stack.last().unwrap_or(&Style::default());

                    if in_code_block {
                        // In code blocks, preserve each line
                        for line in t.split('\n') {
                            if !current_line.is_empty() || !line.is_empty() {
                                current_line.push(Span::styled(line.to_string(), style));
                                lines.push(Line::from(std::mem::take(&mut current_line)));
                            }
                        }
                    } else {
                        current_line.push(Span::styled(t.to_string(), style));
                    }
                }
                Event::Code(t) => {
                    let code_style = Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD);
                    current_line.push(Span::styled(format!("`{}`", t), code_style));
                }
                Event::SoftBreak | Event::HardBreak => {
                    if in_code_block {
                        lines.push(Line::from(std::mem::take(&mut current_line)));
                    } else {
                        current_line.push(Span::raw(" "));
                    }
                }
                Event::Rule => {
                    if !current_line.is_empty() {
                        lines.push(Line::from(std::mem::take(&mut current_line)));
                    }
                    lines.push(Line::styled(
                        "─".repeat(80),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
                _ => {}
            }
        }

        // Flush any remaining line
        if !current_line.is_empty() {
            lines.push(Line::from(current_line));
        }

        Text::from(lines)
    }

    /// Render outliner content as a tree structure
    fn render_outliner<'a>(&self, text: &'a str) -> Text<'a> {
        let mut lines: Vec<Line> = Vec::new();

        for line in text.lines() {
            // Skip empty lines
            if line.trim().is_empty() {
                lines.push(Line::raw(""));
                continue;
            }

            // Count leading spaces to determine indent level
            let spaces = line.len() - line.trim_start().len();
            let level = spaces / 2; // 2 spaces per indent level
            let content = line.trim_start();

            // Determine bullet style and content based on prefix
            let (bullet_char, is_collapsed, text_content) = if content.starts_with("+ ") {
                ("▸", true, &content[2..]) // Collapsed: right-pointing triangle
            } else if content.starts_with("- ") {
                ("•", false, &content[2..]) // Expanded: bullet
            } else if content.starts_with('>') {
                // Backwards compat: old collapsed format
                ("▸", true, &content[1..])
            } else {
                // No bullet prefix, just show content
                ("•", false, content)
            };

            // Build the line with proper indentation and styling
            let mut spans: Vec<Span> = Vec::new();

            // Add indentation (tree lines could be added here for fancier display)
            if level > 0 {
                let indent = "  ".repeat(level);
                spans.push(Span::styled(indent, Style::default().fg(Color::DarkGray)));
            }

            // Add bullet with colour based on collapsed state
            let bullet_style = if is_collapsed {
                Style::default().fg(Color::Yellow) // Collapsed items in yellow
            } else {
                Style::default().fg(Color::Cyan) // Expanded items in cyan
            };
            spans.push(Span::styled(format!("{} ", bullet_char), bullet_style));

            // Add content text
            let content_style = if is_collapsed {
                Style::default().fg(Color::Gray) // Collapsed content slightly dimmed
            } else {
                Style::default()
            };
            spans.push(Span::styled(text_content.to_string(), content_style));

            lines.push(Line::from(spans));
        }

        // If empty, show a hint
        if lines.is_empty() {
            lines.push(Line::styled(
                "(empty outliner)",
                Style::default().fg(Color::DarkGray),
            ));
        }

        Text::from(lines)
    }
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert syntect color to ratatui color
fn syntect_to_ratatui_color(color: syntect::highlighting::Color) -> Color {
    Color::Rgb(color.r, color.g, color.b)
}
