//! Pager operations for viewing content in external tools
//!
//! Provides functionality for viewing note content in external pagers
//! with syntax highlighting support.

use anyhow::{Context, Result};

use crate::models::SyntaxLanguage;
use super::super::app::App;

/// View content in read-only mode using best available pager
///
/// Pager preference order:
/// 1. $PAGER environment variable (if set)
/// 2. bat (syntax highlighting pager)
/// 3. less
/// 4. more (fallback)
pub fn view_with_pager(app: &mut App, content: &str, syntax: SyntaxLanguage) -> Result<()> {
    use std::io::Write;
    use crossterm::{
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
                   Clear as TerminalClear, ClearType},
        cursor::MoveTo,
    };

    // Map syntax language to file extension for syntax highlighting
    let extension = syntax_to_extension(syntax);

    // Create temp file with appropriate extension
    let mut temp_file = tempfile::Builder::new()
        .suffix(&format!(".{}", extension))
        .tempfile()
        .context("Failed to create temporary file")?;

    // Write content to temp file
    temp_file.write_all(content.as_bytes())
        .context("Failed to write to temporary file")?;
    temp_file.flush()?;

    let temp_path = temp_file.path().to_path_buf();

    // Suspend TUI
    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(std::io::stdout(), LeaveAlternateScreen)
        .context("Failed to leave alternate screen")?;

    // Determine pager to use
    let pager_result = if let Ok(pager) = std::env::var("PAGER") {
        // User-specified pager
        run_pager(&pager, &temp_path, None)
    } else if is_command_available("bat") {
        // bat with syntax highlighting (--paging=always forces pager mode)
        run_pager("bat", &temp_path, Some(&["--paging=always", "--style=plain"]))
    } else if is_command_available("less") {
        // less with raw control chars for any escape sequences
        run_pager("less", &temp_path, Some(&["-R"]))
    } else {
        // Fallback to more
        run_pager("more", &temp_path, None)
    };

    // Resume TUI
    execute!(std::io::stdout(), EnterAlternateScreen)
        .context("Failed to enter alternate screen")?;
    enable_raw_mode().context("Failed to enable raw mode")?;

    // Clear screen and force redraw
    execute!(
        std::io::stdout(),
        TerminalClear(ClearType::All),
        TerminalClear(ClearType::Purge),
        MoveTo(0, 0)
    )
    .context("Failed to clear screen")?;
    std::io::stdout().flush().context("Failed to flush stdout")?;
    app.need_redraw = true;

    pager_result
}

/// Map syntax language to file extension for syntax highlighting
fn syntax_to_extension(syntax: SyntaxLanguage) -> &'static str {
    match syntax {
        SyntaxLanguage::Plain => "txt",
        SyntaxLanguage::Markdown => "md",
        SyntaxLanguage::Javascript => "js",
        SyntaxLanguage::Python => "py",
        SyntaxLanguage::Html => "html",
        SyntaxLanguage::Css => "css",
        SyntaxLanguage::Json => "json",
        SyntaxLanguage::Sql => "sql",
        SyntaxLanguage::Bash => "sh",
        SyntaxLanguage::Perl => "pl",
        SyntaxLanguage::Calc => "txt",
        SyntaxLanguage::Outliner => "md", // Markdown-compatible format
    }
}

/// Check if a command is available in PATH
fn is_command_available(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
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
