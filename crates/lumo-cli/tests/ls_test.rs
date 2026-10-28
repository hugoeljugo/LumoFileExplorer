use std::fs::{File, create_dir};
use std::path::MAIN_SEPARATOR;
use std::process::{Command, Output};

#[allow(clippy::expect_used)] // test helper, not a #[test] fn, so clippy.toml doesn't cover it
fn lumo(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lumo"))
        .args(args)
        .output()
        .expect("failed to run lumo binary")
}

#[test]
fn version_prints_crate_version() {
    let out = lumo(&["--version".as_ref()]);

    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.trim(), format!("lumo {}", env!("CARGO_PKG_VERSION")));
}

#[test]
fn ls_lists_directory_contents() {
    let dir = tempfile::tempdir().unwrap();
    File::create(dir.path().join("notes.txt")).unwrap();
    create_dir(dir.path().join("src")).unwrap();

    let out = lumo(&["ls".as_ref(), dir.path().as_os_str()]);

    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        [format!("src{MAIN_SEPARATOR}"), "notes.txt".to_owned()]
    );
}

#[test]
fn ls_missing_path_fails_with_message() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("nope");

    let out = lumo(&["ls".as_ref(), missing.as_os_str()]);

    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.starts_with("lumo: path not found:"),
        "stderr: {stderr}"
    );
}

#[test]
fn no_arguments_prints_help() {
    let out = lumo(&[]);

    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("Usage: lumo"), "stderr: {stderr}");
}
