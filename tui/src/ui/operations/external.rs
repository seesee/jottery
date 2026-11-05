//! Running other programs: the editor, the pager and the system's file opener.

use anyhow::{Context, Result};
use rust_i18n::t;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Split a command from `$EDITOR` or `$PAGER` into a program and its arguments.
///
/// These variables hold shell-style command lines such as `code -w`, not just a
/// program name.
pub fn parse_command(command: &str) -> Result<(String, Vec<String>)> {
    let mut words = shell_words::split(command)
        .map_err(|e| anyhow::anyhow!("{}: {}", command, e))?
        .into_iter();

    match words.next() {
        Some(program) => Ok((program, words.collect())),
        None => anyhow::bail!("{}", t!("editor.external_failed", error = command)),
    }
}

/// The editor to use. An empty `$EDITOR` counts as unset.
pub fn editor_command() -> String {
    env::var("EDITOR")
        .ok()
        .filter(|editor| !editor.trim().is_empty())
        .unwrap_or_else(|| t!("editor.default_editor").to_string())
}

/// Options that stop an editor keeping copies of the note outside the vault, for the
/// editors known to take them. A swap file, a backup, undo history and viminfo all hold
/// the text, and all of them outlive the temporary file.
fn private_editing_args(program: &str) -> &'static [&'static str] {
    const VIM: &[&str] = &["-n", "-i", "NONE", "--cmd", "set nobackup nowritebackup noundofile"];

    let name = Path::new(program).file_stem().and_then(|name| name.to_str()).unwrap_or("");
    match name {
        "vim" | "nvim" | "gvim" | "mvim" => VIM,
        // vi is often vim under another name, but a smaller vi would refuse these
        "vi" if is_really_vim(program) => VIM,
        _ => &[],
    }
}

/// Whether the `vi` that would run is vim.
fn is_really_vim(program: &str) -> bool {
    let path = if program.contains(std::path::MAIN_SEPARATOR) {
        Some(PathBuf::from(program))
    } else {
        find_on_path(program)
    };

    path.and_then(|path| std::fs::canonicalize(path).ok())
        .and_then(|path| path.file_name().map(|name| name.to_string_lossy().contains("vim")))
        .unwrap_or(false)
}

/// Open `file` in `editor` and wait for it to finish.
pub fn run_editor(editor: &str, file: &Path) -> Result<()> {
    let (program, args) = parse_command(editor)?;
    let private_args = private_editing_args(&program);

    let status = Command::new(program)
        .args(args)
        .args(private_args)
        .arg(file)
        .status()
        .with_context(|| t!("editor.external_failed", error = editor).to_string())?;

    if !status.success() {
        anyhow::bail!("{}", t!("editor.external_exit_error"));
    }
    Ok(())
}

/// Find a program on `$PATH` without running anything.
pub fn find_on_path(program: &str) -> Option<PathBuf> {
    let extensions: &[&str] = if cfg!(windows) { &["exe", "cmd", "bat"] } else { &[""] };

    env::split_paths(&env::var_os("PATH")?).find_map(|dir| {
        extensions.iter().find_map(|extension| {
            let candidate = dir.join(program).with_extension(extension);
            candidate.is_file().then_some(candidate)
        })
    })
}

/// The command that opens `path` with the system's default handler, if this
/// machine has one. Headless Linux often has no xdg-open.
pub fn system_opener(path: &Path) -> Option<Command> {
    if cfg!(windows) {
        // `start` is built into cmd.exe rather than being a program. Its first
        // quoted argument is a window title, hence the empty one.
        let mut command = Command::new("cmd");
        command.args(["/C", "start", ""]).arg(path);
        return Some(command);
    }

    let program = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    find_on_path(program)?;
    let mut command = Command::new(program);
    command.arg(path);
    Some(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_editor_can_have_arguments() {
        let (program, args) = parse_command("code -w").unwrap();
        assert_eq!(program, "code");
        assert_eq!(args, vec!["-w"]);
    }

    #[test]
    fn a_quoted_path_stays_in_one_piece() {
        let (program, args) = parse_command("\"/opt/my editor/ed\" --wait 'a b'").unwrap();
        assert_eq!(program, "/opt/my editor/ed");
        assert_eq!(args, vec!["--wait", "a b"]);
    }

    #[test]
    fn a_bare_program_has_no_arguments() {
        let (program, args) = parse_command("vi").unwrap();
        assert_eq!(program, "vi");
        assert!(args.is_empty());
    }

    #[test]
    fn unbalanced_quotes_are_an_error() {
        assert!(parse_command("vi 'oops").is_err());
    }

    /// Vim's swap, backup, undo and viminfo files all hold the note's text.
    #[test]
    fn vim_is_told_to_keep_no_copies() {
        for program in ["vim", "nvim", "/usr/local/bin/vim", "gvim"] {
            let args = private_editing_args(program);
            assert!(args.contains(&"-n"), "{program}");
            assert!(args.contains(&"NONE"), "{program}");
            assert!(args.iter().any(|arg| arg.contains("noundofile")), "{program}");
        }
    }

    #[test]
    fn an_editor_that_might_refuse_those_options_gets_none() {
        for program in ["nano", "emacs", "code", "ed", "/opt/not-vim/editor"] {
            assert!(private_editing_args(program).is_empty(), "{program}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_vi_that_is_not_vim_gets_none() {
        let folder = tempfile::tempdir().unwrap();
        let vi = folder.path().join("vi");
        std::fs::write(&vi, "#!/bin/sh\n").unwrap();

        assert!(private_editing_args(vi.to_str().unwrap()).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_vi_that_is_vim_under_another_name_is_recognised() {
        let folder = tempfile::tempdir().unwrap();
        let vim = folder.path().join("vim");
        std::fs::write(&vim, "#!/bin/sh\n").unwrap();
        let vi = folder.path().join("vi");
        std::os::unix::fs::symlink(&vim, &vi).unwrap();

        assert!(!private_editing_args(vi.to_str().unwrap()).is_empty());
    }

    #[test]
    fn an_empty_command_is_an_error() {
        assert!(parse_command("   ").is_err());
    }

    #[test]
    fn a_missing_editor_is_reported() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let name = format!("no-such-editor-{}", uuid::Uuid::new_v4());
        let error = run_editor(&name, file.path()).unwrap_err();
        assert!(format!("{error:#}").contains(&name));
    }

    #[cfg(unix)]
    #[test]
    fn an_editor_gets_its_arguments_before_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("note.txt");
        std::fs::write(&file, "before").unwrap();

        // sh -c 'script' name FILE: the file arrives as $1
        run_editor("sh -c 'echo after > \"$1\"' editor", &file).unwrap();

        assert_eq!(std::fs::read_to_string(&file).unwrap().trim(), "after");
    }

    #[cfg(unix)]
    #[test]
    fn an_editor_that_fails_is_reported() {
        let file = tempfile::NamedTempFile::new().unwrap();
        assert!(run_editor("false", file.path()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn programs_are_found_on_the_path() {
        assert!(find_on_path("sh").is_some());
        assert!(find_on_path(&format!("no-such-program-{}", uuid::Uuid::new_v4())).is_none());
    }
}
