//! Temporary files that hold decrypted notes and attachments.
//!
//! An editor, a pager and a viewer can only work on a file, so the text has to leave the
//! vault for a while. These files live in a private folder, in memory where this machine
//! has that, and don't rely on this process ending well: they're removed from a signal
//! handler, and a later run clears up after one that was killed outright.
//!
//! An editor's side files land in the same folder if it keeps them beside the file, so
//! they go with it. One that keeps them elsewhere isn't covered: see the README.

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use tempfile::NamedTempFile;

/// Files in use. A signal handler has to know what's out there.
static LIVE: Mutex<Option<HashSet<PathBuf>>> = Mutex::new(None);

/// This run's folder, once made
static SESSION: OnceLock<Session> = OnceLock::new();

/// Another program has the terminal, so Ctrl+C is meant for it
static CHILD_HAS_TERMINAL: AtomicBool = AtomicBool::new(false);

/// The user has been told that files here reach the disk
static DISK_WARNING_GIVEN: AtomicBool = AtomicBool::new(false);

/// This run's private folder.
struct Session {
    path: PathBuf,
    in_memory: bool,
}

/// Places for temporary files, best first, and whether each is held in memory.
fn candidate_bases() -> Vec<(PathBuf, bool)> {
    let mut bases = Vec::new();

    if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR").filter(|dir| !dir.is_empty()) {
        bases.push((PathBuf::from(runtime), true));
    }
    if cfg!(target_os = "linux") {
        bases.push((PathBuf::from("/dev/shm"), true));
    }
    bases.push((std::env::temp_dir(), false));

    bases
}

/// The folder under a base that all of this user's runs share.
fn user_root(base: &Path) -> PathBuf {
    #[cfg(unix)]
    let name = format!("jottery-{}", unsafe { libc::getuid() });
    #[cfg(not(unix))]
    let name = "jottery".to_string();

    base.join(name)
}

/// Make a folder only this user can enter, or refuse one that's already there and isn't
/// that. A shared temp folder lets anyone create the name first.
#[cfg(unix)]
fn make_private(path: &Path) -> Result<()> {
    use std::fs::DirBuilder;
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};

    match DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => return Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e).with_context(|| format!("Failed to create {}", path.display())),
    }

    let metadata = fs::symlink_metadata(path).context("Failed to inspect the temporary folder")?;
    let is_ours = metadata.is_dir() && metadata.uid() == unsafe { libc::getuid() };
    if !is_ours {
        anyhow::bail!("{} isn't a folder of ours, so it won't be used", path.display());
    }

    if metadata.permissions().mode() & 0o077 != 0 {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .context("Failed to restrict the temporary folder")?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn make_private(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("Failed to create {}", path.display()))
}

/// This run's folder under a base.
fn session_under(base: &Path) -> Result<PathBuf> {
    let root = user_root(base);
    make_private(&root)?;

    let session = root.join(std::process::id().to_string());
    make_private(&session)?;
    Ok(session)
}

fn session() -> Result<&'static Session> {
    if let Some(session) = SESSION.get() {
        return Ok(session);
    }

    let mut last_error = None;
    for (base, in_memory) in candidate_bases() {
        match session_under(&base) {
            Ok(path) => return Ok(SESSION.get_or_init(|| Session { path, in_memory })),
            Err(e) => last_error = Some(e),
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("No temporary folder is available")))
}

/// A temporary file that's accounted for. Dropping it removes it, and whatever an editor
/// left beside it once nothing else is open.
pub struct SecureTempFile {
    file: NamedTempFile,
}

impl Deref for SecureTempFile {
    type Target = NamedTempFile;

    fn deref(&self) -> &NamedTempFile {
        &self.file
    }
}

impl std::ops::DerefMut for SecureTempFile {
    fn deref_mut(&mut self) -> &mut NamedTempFile {
        &mut self.file
    }
}

impl Drop for SecureTempFile {
    fn drop(&mut self) {
        let Ok(mut live) = LIVE.lock() else {
            return;
        };

        let nothing_left = live.as_mut().is_none_or(|paths| {
            paths.remove(self.file.path());
            paths.is_empty()
        });

        if nothing_left {
            if let Some(session) = SESSION.get() {
                clear_folder(&session.path, Some(self.file.path()));
            }
        }
    }
}

/// Remove everything in a folder, apart from `except`.
fn clear_folder(path: &Path, except: Option<&Path>) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if Some(path.as_path()) == except {
            continue;
        }
        let _ = if path.is_dir() { fs::remove_dir_all(&path) } else { fs::remove_file(&path) };
    }
}

/// Create a temporary file for decrypted content. `suffix` includes any dot.
pub fn create(suffix: &str) -> Result<SecureTempFile> {
    let session = session()?;

    // Held across both steps, or another file closing in between would clear this one
    let mut live = LIVE.lock().map_err(|_| anyhow::anyhow!("Temporary file records are unavailable"))?;

    let file = tempfile::Builder::new()
        .suffix(suffix)
        .tempfile_in(&session.path)
        .context("Failed to create temporary file")?;

    live.get_or_insert_with(HashSet::new).insert(file.path().to_path_buf());

    Ok(SecureTempFile { file })
}

/// Whether the user still needs telling that temporary files here reach the disk. It
/// answers yes once, so that the warning doesn't repeat on every edit.
pub fn take_disk_warning() -> bool {
    let on_disk = SESSION.get().is_some_and(|session| !session.in_memory);
    on_disk && !DISK_WARNING_GIVEN.swap(true, Ordering::Relaxed)
}

/// Say whether another program has the terminal.
pub fn set_child_has_terminal(has_terminal: bool) {
    CHILD_HAS_TERMINAL.store(has_terminal, Ordering::Relaxed);
}

/// Remove every temporary file of this run. Safe to call at any time and from anywhere,
/// including while files are open.
pub fn remove_everything() {
    if let Ok(mut live) = LIVE.lock() {
        for path in live.take().unwrap_or_default() {
            let _ = fs::remove_file(path);
        }
    }

    if let Some(session) = SESSION.get() {
        let _ = fs::remove_dir_all(&session.path);
    }
}

/// Whether a process with this id is still running.
#[cfg(unix)]
fn is_running(pid: i32) -> bool {
    // Signal 0 checks without sending. EPERM means it's there, and belongs to someone else.
    let answered = unsafe { libc::kill(pid, 0) } == 0;
    answered || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(not(unix))]
fn is_running(_pid: i32) -> bool {
    true
}

/// Clear the folders of runs that are gone from under one user's root.
fn sweep_root(root: &Path) -> usize {
    let Ok(entries) = fs::read_dir(root) else {
        return 0;
    };

    let mut removed = 0;
    for entry in entries.flatten() {
        let pid = entry.file_name().to_str().and_then(|name| name.parse::<i32>().ok());
        if pid.is_some_and(|pid| pid != std::process::id() as i32 && !is_running(pid))
            && fs::remove_dir_all(entry.path()).is_ok()
        {
            removed += 1;
        }
    }
    removed
}

/// Clear up after runs that were killed or lost power, without touching any still going.
pub fn sweep_leftovers() {
    for (base, _) in candidate_bases() {
        sweep_root(&user_root(&base));
    }
}

/// Remove temporary files when told to stop.
///
/// SIGHUP is the dropped SSH connection, and SIGTERM a shutdown. Ctrl+C reaches this
/// process too while an editor or pager has the terminal, and it's meant for them.
#[cfg(unix)]
pub fn install_signal_cleanup(restore_terminal: bool) {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
    use signal_hook::iterator::Signals;

    let Ok(mut signals) = Signals::new([SIGHUP, SIGTERM, SIGINT]) else {
        return;
    };

    std::thread::spawn(move || {
        for signal in signals.forever() {
            if signal == SIGINT && CHILD_HAS_TERMINAL.load(Ordering::Relaxed) {
                continue;
            }

            remove_everything();
            if restore_terminal {
                crate::ui::terminal::restore_terminal();
            }
            std::process::exit(128 + signal);
        }
    });
}

#[cfg(not(unix))]
pub fn install_signal_cleanup(_restore_terminal: bool) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[cfg(unix)]
    #[test]
    fn a_file_is_only_open_to_its_owner() {
        use std::os::unix::fs::PermissionsExt;

        let file = create(".txt").unwrap();

        assert_eq!(fs::metadata(file.path()).unwrap().permissions().mode() & 0o777, 0o600);
        let folder = file.path().parent().unwrap();
        assert_eq!(fs::metadata(folder).unwrap().permissions().mode() & 0o777, 0o700);
        assert_eq!(fs::metadata(folder.parent().unwrap()).unwrap().permissions().mode() & 0o777, 0o700);
    }

    #[test]
    fn a_file_keeps_its_suffix_and_goes_when_dropped() {
        let mut file = create(".md").unwrap();
        file.write_all(b"secret").unwrap();
        let path = file.path().to_path_buf();
        assert_eq!(path.extension().unwrap(), "md");

        drop(file);

        assert!(!path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_someone_else_planted_is_refused() {
        let base = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(elsewhere.path(), user_root(base.path())).unwrap();

        assert!(session_under(base.path()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_loose_folder_of_ours_is_tightened() {
        use std::os::unix::fs::PermissionsExt;

        let base = tempfile::tempdir().unwrap();
        let root = user_root(base.path());
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();

        session_under(base.path()).unwrap();

        assert_eq!(fs::metadata(&root).unwrap().permissions().mode() & 0o777, 0o700);
    }

    #[cfg(unix)]
    #[test]
    fn a_sweep_clears_dead_runs_and_leaves_running_ones() {
        let root = tempfile::tempdir().unwrap();

        // A process that has finished, so its id is known to be free
        let mut finished = std::process::Command::new("true").spawn().unwrap();
        let dead_pid = finished.id();
        finished.wait().unwrap();

        let dead = root.path().join(dead_pid.to_string());
        let alive = root.path().join(std::process::id().to_string());
        let unrelated = root.path().join("not-a-run");
        for folder in [&dead, &alive, &unrelated] {
            fs::create_dir(folder).unwrap();
            fs::write(folder.join("note.txt"), "secret").unwrap();
        }

        assert_eq!(sweep_root(root.path()), 1);

        assert!(!dead.exists());
        assert!(alive.exists());
        assert!(unrelated.exists());
    }

    #[cfg(unix)]
    #[test]
    fn another_run_still_going_is_left_alone() {
        let root = tempfile::tempdir().unwrap();
        let mut running = std::process::Command::new("sleep").arg("30").spawn().unwrap();
        let folder = root.path().join(running.id().to_string());
        fs::create_dir(&folder).unwrap();

        let removed = sweep_root(root.path());

        running.kill().unwrap();
        running.wait().unwrap();
        assert_eq!(removed, 0);
        assert!(folder.exists());
    }

    #[test]
    fn clearing_a_folder_takes_side_files_and_spares_what_it_is_told_to() {
        let folder = tempfile::tempdir().unwrap();
        let kept = folder.path().join("note.txt");
        let backup = folder.path().join("note.txt~");
        let swap = folder.path().join(".note.txt.swp");
        for file in [&kept, &backup, &swap] {
            fs::write(file, "secret").unwrap();
        }

        clear_folder(folder.path(), Some(&kept));

        assert!(kept.exists());
        assert!(!backup.exists());
        assert!(!swap.exists());
    }
}
