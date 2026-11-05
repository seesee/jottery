//! Pager operations for viewing content in external tools
//!
//! Provides functionality for viewing note content in external pagers
//! with syntax highlighting support.

use anyhow::{Context, Result};

use crate::models::SyntaxLanguage;
use super::external;
use super::super::app::App;
use super::super::terminal::with_terminal_suspended;

/// View content in read-only mode using best available pager
///
/// Pager preference order:
/// 1. $PAGER environment variable (if set)
/// 2. bat (syntax highlighting pager)
/// 3. less
/// 4. more (fallback)
pub fn view_with_pager(app: &mut App, content: &str, syntax: SyntaxLanguage) -> Result<()> {
    use std::io::Write;

    // Map syntax language to file extension for syntax highlighting
    let extension = syntax_to_extension(syntax);

    // Create temp file with appropriate extension
    let mut temp_file = crate::secure_temp::create(&format!(".{}", extension))?;

    // Pagers pass some or all escape sequences straight to the terminal, and this
    // is a read-only view, so what they're given is cleaned first.
    let content = crate::terminal_text::sanitise_for_terminal(content);

    // Write content to temp file
    temp_file.write_all(content.as_bytes())
        .context("Failed to write to temporary file")?;
    temp_file.flush()?;

    let temp_path = temp_file.path().to_path_buf();

    with_terminal_suspended(app, || {
        if let Some(pager) = std::env::var("PAGER").ok().filter(|p| !p.trim().is_empty()) {
            // User-specified pager, which can carry its own arguments
            let (program, args) = external::parse_command(&pager)?;
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            run_pager(&program, &temp_path, Some(&args))
        } else if external::find_on_path("bat").is_some() {
            // bat with syntax highlighting (--paging=always forces pager mode)
            run_pager("bat", &temp_path, Some(&["--paging=always", "--style=plain"]))
        } else if external::find_on_path("less").is_some() {
            // less with raw control chars for any escape sequences
            run_pager("less", &temp_path, Some(&["-R"]))
        } else {
            // Fallback to more
            run_pager("more", &temp_path, None)
        }
    })
}

/// Map syntax language to file extension for syntax highlighting
fn syntax_to_extension(syntax: SyntaxLanguage) -> &'static str {
    match syntax {
        SyntaxLanguage::Plain => "txt",
        SyntaxLanguage::Markdown => "md",
        SyntaxLanguage::Javascript => "js",
        SyntaxLanguage::Typescript => "ts",
        SyntaxLanguage::Python => "py",
        SyntaxLanguage::Html => "html",
        SyntaxLanguage::Xml => "xml",
        SyntaxLanguage::Css => "css",
        SyntaxLanguage::Json => "json",
        SyntaxLanguage::Sql => "sql",
        SyntaxLanguage::Bash => "sh",
        SyntaxLanguage::Perl => "pl",
        SyntaxLanguage::Calc => "txt",
        SyntaxLanguage::Outliner => "md", // Markdown-compatible format
    }
}

/// Run a pager with optional arguments
fn run_pager(pager: &str, path: &std::path::Path, args: Option<&[&str]>) -> Result<()> {
    let mut cmd = std::process::Command::new(pager);
    if let Some(args) = args {
        cmd.args(args);
    }
    cmd.arg(path);

    let status = cmd.status();

    match status {
        Ok(exit_status) if exit_status.success() => Ok(()),
        Ok(_) => {
            // Non-zero exit is usually OK for pagers (user might quit early)
            Ok(())
        }
        Err(e) => {
            anyhow::bail!("Failed to run pager '{}': {}", pager, e)
        }
    }
}
