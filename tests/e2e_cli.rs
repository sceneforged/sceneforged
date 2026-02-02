//! CLI end-to-end tests
//!
//! Tests for sceneforged command-line interface.

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

/// Get a command for the sceneforged binary
#[allow(deprecated)]
fn sceneforged_cmd() -> Command {
    Command::cargo_bin("sceneforged").unwrap()
}

fn test_media_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/media")
}

#[test]
fn test_cli_no_args_shows_help() {
    let mut cmd = sceneforged_cmd();
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}

#[test]
fn test_cli_help_flag() {
    let mut cmd = sceneforged_cmd();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("sceneforged"))
        .stdout(predicate::str::contains("Usage"));
}

#[test]
fn test_cli_version_flag() {
    let mut cmd = sceneforged_cmd();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("sceneforged"));
}

#[test]
fn test_cli_check_tools_command() {
    let mut cmd = sceneforged_cmd();
    // The command is "check-tools" but Clap normalizes it
    cmd.arg("check-tools").assert().success().stdout(
        predicate::str::contains("ffmpeg")
            .or(predicate::str::contains("ffprobe"))
            .or(predicate::str::contains("tools")),
    );
}

#[test]
fn test_cli_start_help() {
    let mut cmd = sceneforged_cmd();
    cmd.args(["start", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Start the server"));
}

#[test]
fn test_cli_run_help() {
    let mut cmd = sceneforged_cmd();
    cmd.args(["run", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Process a single file"));
}

#[test]
fn test_cli_probe_help() {
    let mut cmd = sceneforged_cmd();
    cmd.args(["probe", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Probe a media file"));
}

#[test]
fn test_cli_run_nonexistent_file() {
    let mut cmd = sceneforged_cmd();
    cmd.args(["run", "/nonexistent/path/movie.mkv"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("exist")));
}

#[test]
fn test_cli_probe_nonexistent_file() {
    let mut cmd = sceneforged_cmd();
    cmd.args(["probe", "/nonexistent/path/movie.mkv"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("exist")));
}

#[test]
fn test_cli_run_with_config() {
    let test_file = test_media_dir().join("sample_640x360.mp4");
    if !test_file.exists() {
        eprintln!("Skipping: Test file not found. Run: ./scripts/download-test-media.sh");
        return;
    }

    let temp = tempdir().unwrap();
    let config_file = temp.path().join("config.toml");

    fs::write(
        &config_file,
        r#"
[server]
host = "127.0.0.1"
port = 8080

[[rules]]
name = "test-rule"
enabled = true
priority = 100

[rules.match]
codecs = ["h264"]

[[rules.actions]]
type = "remux"
container = "mkv"
"#,
    )
    .unwrap();

    let mut cmd = sceneforged_cmd();
    cmd.args([
        "run",
        "--config",
        config_file.to_str().unwrap(),
        "--dry-run",
        test_file.to_str().unwrap(),
    ])
    .assert()
    .success();
}

#[test]
fn test_cli_run_dry_run_shows_plan() {
    let test_file = test_media_dir().join("sample_640x360.mp4");
    if !test_file.exists() {
        eprintln!("Skipping: Test file not found. Run: ./scripts/download-test-media.sh");
        return;
    }

    let temp = tempdir().unwrap();
    let config_file = temp.path().join("config.toml");

    fs::write(
        &config_file,
        r#"
[[rules]]
name = "Always Match"
enabled = true
priority = 100

[rules.match]

[[rules.actions]]
type = "remux"
container = "mkv"
"#,
    )
    .unwrap();

    let mut cmd = sceneforged_cmd();
    cmd.args([
        "run",
        "--config",
        config_file.to_str().unwrap(),
        "--dry-run",
        test_file.to_str().unwrap(),
    ])
    .assert()
    .success()
    .stdout(
        predicate::str::contains("dry run")
            .or(predicate::str::contains("Would"))
            .or(predicate::str::contains("match")),
    );
}

#[test]
fn test_cli_start_invalid_port() {
    let mut cmd = sceneforged_cmd();
    cmd.args(["start", "--port", "99999"]).assert().failure();
}

#[test]
fn test_cli_start_with_help() {
    let mut cmd = sceneforged_cmd();
    cmd.args(["start", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Host").or(predicate::str::contains("Port")));
}

#[test]
fn test_cli_config_validation() {
    let temp = tempdir().unwrap();
    let config_file = temp.path().join("config.toml");

    fs::write(
        &config_file,
        r#"
[invalid_section]
foo = "bar"
"#,
    )
    .unwrap();

    let mut cmd = sceneforged_cmd();
    cmd.args(["serve", "--config", config_file.to_str().unwrap()])
        .assert()
        .failure();
}

#[test]
fn test_cli_probe_real_file() {
    let path = test_media_dir().join("sample_640x360.mp4");
    if !path.exists() {
        eprintln!("Skipping: Test file not found. Run: ./scripts/download-test-media.sh");
        return;
    }

    let mut cmd = sceneforged_cmd();
    cmd.args(["probe", path.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn test_cli_probe_json_output() {
    let path = test_media_dir().join("sample_640x360.mp4");
    if !path.exists() {
        eprintln!("Skipping: Test file not found. Run: ./scripts/download-test-media.sh");
        return;
    }

    let mut cmd = sceneforged_cmd();
    cmd.args(["probe", "--json", path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("video_tracks"));
}
