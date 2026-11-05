//! Printing untrusted text on a terminal.
//!
//! Note content, tags, filenames and anything a server says can hold escape
//! sequences. Printed as they are, those can write to the clipboard (OSC 52),
//! rewrite earlier output or plant links. The full-screen UI is safe because
//! ratatui drops control characters, so this only matters for what the CLI
//! prints itself.
//!
//! CLI code prints through [`outln!`] and [`errln!`], which clean everything
//! they're given rather than relying on each call site to remember. Output going
//! to a file or a pipe stays exact, so `jottery show ID > note.txt` is faithful.

use std::borrow::Cow;
use std::io::IsTerminal;

/// Replace what a terminal would act on with a visible stand-in.
///
/// Newlines and tabs survive, and a `\r\n` line ending becomes `\n`. Other C0
/// controls become their Unicode control pictures (`␛`), DEL becomes `␡`, and C1
/// controls, which some terminals treat as the same escape sequences, become `�`.
pub fn sanitise_for_terminal(text: &str) -> String {
    let mut clean = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\n' | '\t' => clean.push(c),
            '\r' if chars.peek() == Some(&'\n') => {}
            '\u{7f}' => clean.push('\u{2421}'),
            '\u{0}'..='\u{1f}' => {
                clean.push(char::from_u32(0x2400 + c as u32).unwrap_or('\u{fffd}'));
            }
            '\u{80}'..='\u{9f}' => clean.push('\u{fffd}'),
            _ => clean.push(c),
        }
    }

    clean
}

fn clean_if(is_terminal: bool, text: &str) -> Cow<'_, str> {
    if is_terminal {
        Cow::Owned(sanitise_for_terminal(text))
    } else {
        Cow::Borrowed(text)
    }
}

/// `text` as it should reach stdout: cleaned on a terminal, exact anywhere else.
pub fn for_stdout(text: &str) -> Cow<'_, str> {
    clean_if(std::io::stdout().is_terminal(), text)
}

/// `text` as it should reach stderr: cleaned on a terminal, exact anywhere else.
pub fn for_stderr(text: &str) -> Cow<'_, str> {
    clean_if(std::io::stderr().is_terminal(), text)
}

/// `println!` for anything that might hold note or server text.
macro_rules! outln {
    () => { println!() };
    ($($arg:tt)*) => {
        println!("{}", $crate::terminal_text::for_stdout(&format!($($arg)*)))
    };
}

/// `eprintln!` for anything that might hold note or server text.
macro_rules! errln {
    () => { eprintln!() };
    ($($arg:tt)*) => {
        eprintln!("{}", $crate::terminal_text::for_stderr(&format!($($arg)*)))
    };
}

pub(crate) use {errln, outln};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_sequences_lose_their_escape() {
        let clipboard_write = "\u{1b}]52;c;cm0gLXJmIH4=\u{7}";
        let clean = sanitise_for_terminal(clipboard_write);
        assert!(!clean.contains('\u{1b}'));
        assert!(!clean.contains('\u{7}'));
        assert_eq!(clean, "\u{241b}]52;c;cm0gLXJmIH4=\u{2407}");
    }

    #[test]
    fn c1_controls_are_replaced() {
        // 0x9B is CSI on terminals that honour 8-bit controls
        let clean = sanitise_for_terminal("a\u{9b}31mb\u{9d}0;title\u{9c}");
        assert_eq!(clean, "a\u{fffd}31mb\u{fffd}0;title\u{fffd}");
    }

    #[test]
    fn delete_is_replaced() {
        assert_eq!(sanitise_for_terminal("a\u{7f}b"), "a\u{2421}b");
    }

    #[test]
    fn newlines_and_tabs_are_kept() {
        let text = "line one\n\tline two\n";
        assert_eq!(sanitise_for_terminal(text), text);
    }

    #[test]
    fn a_windows_line_ending_stays_a_line_ending() {
        assert_eq!(sanitise_for_terminal("one\r\ntwo"), "one\ntwo");
    }

    #[test]
    fn a_lone_carriage_return_cannot_overwrite_the_line() {
        assert_eq!(sanitise_for_terminal("safe\rfake"), "safe\u{240d}fake");
    }

    #[test]
    fn ordinary_text_is_untouched() {
        let text = "日本語 — naïve café 📎";
        assert_eq!(sanitise_for_terminal(text), text);
    }

    #[test]
    fn text_bound_for_a_file_stays_exact() {
        let text = "\u{1b}[31mred\u{1b}[0m";
        assert_eq!(clean_if(false, text), text);
        assert_ne!(clean_if(true, text), text);
    }

    /// Everything the CLI prints goes through the macros above. A bare println!
    /// in main.rs is how an escape sequence reaches the terminal.
    #[test]
    fn the_cli_never_prints_around_the_sanitiser() {
        let source = include_str!("main.rs");
        for (number, line) in source.lines().enumerate() {
            let trimmed = line.trim_start();
            let printing = trimmed.starts_with("println!") || trimmed.starts_with("eprintln!");
            assert!(!printing, "main.rs:{}: use outln! or errln!: {}", number + 1, trimmed);
        }
    }
}
