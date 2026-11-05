//! CLI End-to-End Tests for Jottery TUI
//!
//! These tests verify the CLI commands work correctly by running the actual binary.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tempfile::TempDir;

/// Get the path to the jottery binary, wherever the build put it
fn get_binary_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_jottery"))
}

/// Create a test environment with temporary database
fn create_test_env() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test.db");
    (temp_dir, db_path)
}

#[test]
fn test_version_flag() {
    let binary = get_binary_path();
    if !binary.exists() {
        eprintln!("Binary not found at {:?}, skipping CLI test", binary);
        return;
    }

    let output = Command::new(&binary)
        .arg("--version")
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("jottery"), "Version output should contain 'jottery'");
    assert!(output.status.success(), "Version command should succeed");
}

#[test]
fn test_help_flag() {
    let binary = get_binary_path();
    if !binary.exists() {
        eprintln!("Binary not found at {:?}, skipping CLI test", binary);
        return;
    }

    let output = Command::new(&binary)
        .arg("--help")
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage"), "Help output should contain usage info");
    assert!(stdout.contains("note"), "Help should list note command");
    assert!(stdout.contains("list"), "Help should list list command");
    assert!(stdout.contains("search"), "Help should list search command");
    assert!(stdout.contains("sync"), "Help should list sync command");
    assert!(output.status.success(), "Help command should succeed");
}

#[test]
fn test_list_subcommand_help() {
    let binary = get_binary_path();
    if !binary.exists() {
        eprintln!("Binary not found at {:?}, skipping CLI test", binary);
        return;
    }

    let output = Command::new(&binary)
        .args(["list", "--help"])
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("password"), "List help should mention password option");
    assert!(stdout.contains("tag"), "List help should mention tag filter");
    assert!(output.status.success());
}

#[test]
fn test_search_subcommand_help() {
    let binary = get_binary_path();
    if !binary.exists() {
        eprintln!("Binary not found at {:?}, skipping CLI test", binary);
        return;
    }

    let output = Command::new(&binary)
        .args(["search", "--help"])
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("query"), "Search help should mention query");
    assert!(stdout.contains("password"), "Search help should mention password");
    assert!(output.status.success());
}

#[test]
fn test_export_subcommand_help() {
    let binary = get_binary_path();
    if !binary.exists() {
        eprintln!("Binary not found at {:?}, skipping CLI test", binary);
        return;
    }

    let output = Command::new(&binary)
        .args(["export", "--help"])
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("output"), "Export help should mention output file");
    assert!(stdout.contains("password"), "Export help should mention password");
    assert!(output.status.success());
}

#[test]
fn test_register_user_subcommand_help() {
    let binary = get_binary_path();
    if !binary.exists() {
        eprintln!("Binary not found at {:?}, skipping CLI test", binary);
        return;
    }

    let output = Command::new(&binary)
        .args(["register-user", "--help"])
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("server"), "Register user help should mention server");
    assert!(stdout.contains("email"), "Register user help should mention email");
    assert!(output.status.success());
}

#[test]
fn test_register_device_subcommand_help() {
    let binary = get_binary_path();
    if !binary.exists() {
        eprintln!("Binary not found at {:?}, skipping CLI test", binary);
        return;
    }

    let output = Command::new(&binary)
        .args(["register-device", "--help"])
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("server"), "Register device help should mention server");
    assert!(stdout.contains("email"), "Register device help should mention email");
    assert!(stdout.contains("device-name"), "Register device help should mention device name");
    assert!(output.status.success());
}

#[test]
fn test_database_flag() {
    let binary = get_binary_path();
    if !binary.exists() {
        eprintln!("Binary not found at {:?}, skipping CLI test", binary);
        return;
    }

    let (_temp_dir, db_path) = create_test_env();

    // Just test that the flag is recognized (we won't actually create a database without password)
    let output = Command::new(&binary)
        .args(["--database", db_path.to_str().unwrap(), "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Database flag should be recognized");
}

#[test]
fn test_invalid_subcommand() {
    let binary = get_binary_path();
    if !binary.exists() {
        eprintln!("Binary not found at {:?}, skipping CLI test", binary);
        return;
    }

    let output = Command::new(&binary)
        .arg("invalid-command-that-does-not-exist")
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success(), "Invalid subcommand should fail");
}

#[test]
fn test_export_without_required_args() {
    let binary = get_binary_path();
    if !binary.exists() {
        eprintln!("Binary not found at {:?}, skipping CLI test", binary);
        return;
    }

    let output = Command::new(&binary)
        .arg("export")
        .output()
        .expect("Failed to execute command");

    // Export requires --output and --password
    assert!(!output.status.success(), "Export without args should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("required") || stderr.contains("error"),
        "Should report missing required arguments"
    );
}

#[test]
fn test_sync_subcommand_help() {
    let binary = get_binary_path();
    if !binary.exists() {
        eprintln!("Binary not found at {:?}, skipping CLI test", binary);
        return;
    }

    let output = Command::new(&binary)
        .args(["sync", "--help"])
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("password"), "Sync help should mention password");
    assert!(output.status.success());
}

#[test]
fn test_show_subcommand_help() {
    let binary = get_binary_path();
    if !binary.exists() {
        eprintln!("Binary not found at {:?}, skipping CLI test", binary);
        return;
    }

    let output = Command::new(&binary)
        .args(["show", "--help"])
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("id"), "Show help should mention note ID");
    assert!(stdout.contains("password"), "Show help should mention password");
    assert!(output.status.success());
}

/// Run jottery against one database, feeding it `input`. Returns its exit status,
/// stdout and stderr.
fn run_with_input(db: &Path, args: &[&str], input: &str) -> (bool, String, String) {
    let mut child = Command::new(get_binary_path())
        .env("JOTTERY_DB_PATH", db)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("jottery should start");

    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let output = child.wait_with_output().unwrap();

    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn run_on(db: &Path, args: &[&str]) -> (bool, String, String) {
    run_with_input(db, args, "")
}

/// A vault holding one note. Returns the note's short id.
fn vault_with_note(db: &Path, password: &str, content: &str) -> String {
    let (ok, stdout, stderr) = run_with_input(db, &["note", "-p", password], content);
    assert!(ok, "{stderr}");
    stdout.split_whitespace().last().unwrap().to_string()
}

#[test]
fn register_device_refuses_one_password_for_both_jobs() {
    let (dir, _) = create_test_env();
    let password = uuid::Uuid::new_v4().to_string();

    // Nothing listens on this port: the refusal has to come before any request
    let (ok, _, stderr) = run_on(&dir.path().join("test.db"), &[
        "register-device", "-s", "http://127.0.0.1:1", "-e", "someone@example.com",
        "-p", &password, "-k", &password,
    ]);

    assert!(!ok);
    assert!(stderr.contains("different"), "{stderr}");
}

/// Refusing here would stop someone whose passwords already match adding a device.
/// Nothing listens on this port, so the command gets as far as trying to connect.
#[test]
fn connecting_an_existing_vault_with_one_password_for_both_jobs_only_warns() {
    let (_temp_dir, db) = create_test_env();
    let password = uuid::Uuid::new_v4().to_string();
    vault_with_note(&db, &password, "Already here");

    let (succeeded, _, stderr) = run_on(&db, &[
        "register-device",
        "--server", "http://127.0.0.1:1",
        "--email", "someone@example.com",
        "--password", &password,
        "--key-password", &password,
    ]);

    assert!(!succeeded);
    assert!(stderr.contains("⚠"), "should warn: {stderr}");
    assert!(!stderr.contains("Use different passwords"), "should not refuse: {stderr}");
}

#[test]
fn a_wrong_password_is_refused_rather_than_showing_an_empty_vault() {
    let (dir, _) = create_test_env();
    let db = dir.path().join("test.db");
    let password = uuid::Uuid::new_v4().to_string();
    vault_with_note(&db, &password, "Secret");

    let wrong = uuid::Uuid::new_v4().to_string();
    let (ok, stdout, _) = run_on(&db, &["list", "-p", &wrong]);

    assert!(!ok);
    assert!(!stdout.contains("Secret"));
}

#[test]
fn a_new_vault_needs_a_password_of_eight_characters() {
    let (dir, _) = create_test_env();

    let (ok, _, stderr) = run_with_input(&dir.path().join("test.db"), &["note", "-p", "short"], "A note");

    assert!(!ok);
    assert!(stderr.contains('8'), "{stderr}");
}

#[test]
fn output_to_a_pipe_keeps_the_note_exact() {
    let (dir, _) = create_test_env();
    let db = dir.path().join("test.db");
    let password = uuid::Uuid::new_v4().to_string();
    let content = "Coloured \u{1b}[31mtext\u{1b}[0m";
    let id = vault_with_note(&db, &password, content);

    let (ok, stdout, _) = run_on(&db, &["show", &id, "-p", &password]);

    assert!(ok);
    assert!(stdout.contains(content));
}

#[test]
fn archiving_from_the_command_line() {
    let (dir, _) = create_test_env();
    let db = dir.path().join("test.db");
    let password = uuid::Uuid::new_v4().to_string();
    let id = vault_with_note(&db, &password, "Finished with this");

    assert!(run_on(&db, &["archive", &id, "-p", &password]).0);

    assert!(!run_on(&db, &["list", "-p", &password]).1.contains("Finished with this"));
    assert!(run_on(&db, &["list", "--archived", "-p", &password]).1.contains("Finished with this"));

    assert!(run_on(&db, &["unarchive", &id, "-p", &password]).0);

    assert!(run_on(&db, &["list", "-p", &password]).1.contains("Finished with this"));
}

#[test]
fn attaching_and_saving_from_the_command_line() {
    let (dir, _) = create_test_env();
    let db = dir.path().join("test.db");
    let password = uuid::Uuid::new_v4().to_string();
    let id = vault_with_note(&db, &password, "With a file");

    let source = dir.path().join("source.txt");
    fs::write(&source, "attached").unwrap();
    let (ok, _, stderr) = run_on(&db, &["attach", &id, source.to_str().unwrap(), "-p", &password]);
    assert!(ok, "{stderr}");

    assert!(run_on(&db, &["show", &id, "-p", &password]).1.contains("source.txt"));

    let saved = dir.path().join("saved.txt");
    let (ok, _, stderr) = run_on(&db, &["save-attachment", &id, "-o", saved.to_str().unwrap(), "-p", &password]);
    assert!(ok, "{stderr}");
    assert_eq!(fs::read_to_string(saved).unwrap(), "attached");
}

