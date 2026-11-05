//! Terminal initialization and management
//!
//! This module provides the TUI terminal wrapper for setting up and tearing down
//! the terminal interface.

use anyhow::{Context, Result};
use crossterm::{
    cursor::{MoveTo, Show},
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout, Write};

use super::app::App;

/// Put the terminal back as the shell expects it. Safe to call in any state, and
/// from a panic, so it reports nothing and keeps going past each failure.
pub fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture, Show);
}

/// Restore the terminal before a panic is reported.
///
/// Without this the message lands on the alternate screen, which vanishes, and the
/// shell is left in raw mode. Panics on other threads don't end the app, and they
/// report through their own channels, so the screen is left alone for those.
pub fn install_panic_hook() {
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if std::thread::current().name() == Some("main") {
            restore_terminal();
            report(info);
        }
    }));
}

/// The TUI stepping aside so another program can have the terminal.
///
/// Dropping this brings the TUI back, so every way out of the suspended section
/// restores it, including an early return and a failed launch.
struct SuspendedTerminal;

impl SuspendedTerminal {
    fn new() -> Result<Self> {
        // Exists before the first step, so a failure part-way through is undone
        let suspended = Self;
        disable_raw_mode().context("Failed to disable raw mode")?;
        execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)
            .context("Failed to leave alternate screen")?;
        Ok(suspended)
    }
}

impl Drop for SuspendedTerminal {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = execute!(stdout, EnterAlternateScreen, EnableMouseCapture);
        let _ = enable_raw_mode();
        let _ = execute!(stdout, Clear(ClearType::All), Clear(ClearType::Purge), MoveTo(0, 0));
        let _ = stdout.flush();
    }
}

/// Run `task` with the TUI out of the way, for an editor, pager or viewer.
pub fn with_terminal_suspended<T>(app: &mut App, task: impl FnOnce() -> Result<T>) -> Result<T> {
    crate::secure_temp::set_child_has_terminal(true);
    let result = SuspendedTerminal::new().and_then(|_suspended| task());
    crate::secure_temp::set_child_has_terminal(false);
    app.need_redraw = true;
    // Time in another program counts as use, or finishing a long edit would lock
    app.register_activity();
    result
}

/// TUI terminal wrapper
pub struct Tui {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl Tui {
    /// Create a new TUI
    pub fn new() -> Result<Self> {
        let backend = CrosstermBackend::new(io::stdout());
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    /// Enter the terminal UI
    pub fn enter(&mut self) -> Result<()> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        // Don't hide cursor - let ratatui manage it via set_cursor()
        self.terminal.clear()?;
        Ok(())
    }

    /// Exit the terminal UI
    pub fn exit(&mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
        self.terminal.show_cursor()?;
        Ok(())
    }

    /// Draw a frame
    pub fn draw<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut ratatui::Frame),
    {
        self.terminal.draw(f)?;
        Ok(())
    }

    /// Clear the terminal buffer (force full redraw)
    pub fn clear(&mut self) -> Result<()> {
        self.terminal.clear()?;
        Ok(())
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        let _ = self.exit();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        std::mem::forget(dir);
        App::new(path, None).unwrap()
    }

    #[test]
    fn a_failure_while_suspended_still_comes_back_for_a_redraw() {
        let mut app = app();
        let result: Result<()> = with_terminal_suspended(&mut app, || anyhow::bail!("no such editor"));
        assert!(result.is_err());
        assert!(app.need_redraw);
    }

    #[test]
    fn a_task_runs_and_its_result_is_returned() {
        let mut app = app();
        let result = with_terminal_suspended(&mut app, || Ok(42));
        assert_eq!(result.unwrap(), 42);
    }
}
