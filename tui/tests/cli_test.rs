//! CLI End-to-End Tests for Jottery TUI
//!
//! These tests verify the CLI commands work correctly by running the actual binary.

use std::process::Command;
use std::path::PathBuf;
use tempfile::TempDir;

/// Get the path to the jottery binary
fn get_binary_path() -> PathBuf {
    // In test mode, the binary is in target/debug
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target");
    path.push("debug");
    path.push("jottery");
    path
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
