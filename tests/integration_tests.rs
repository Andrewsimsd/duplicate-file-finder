use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;
use walkdir::WalkDir;

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    for entry in WalkDir::new(src) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src).expect("strip_prefix failed");
        let full_destination = dst.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&full_destination)?;
        } else {
            if let Some(parent) = full_destination.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), &full_destination)?;
        }
    }
    Ok(())
}

fn run_with_args(dir: &Path, args: &[&str]) -> std::process::Output {
    let exe = env!("CARGO_BIN_EXE_duplicate_file_finder");
    Command::new(exe)
        .current_dir(dir)
        .args(args)
        .output()
        .expect("failed to run binary")
}

#[test]
fn default_output_file_generated() {
    let tmp = tempdir().expect("create temp dir");
    let input_dir = tmp.path().join("data");
    copy_dir_recursive(Path::new("resources"), &input_dir).expect("copy resources");

    let output = run_with_args(tmp.path(), &[input_dir.to_str().expect("valid UTF-8")]);
    assert!(output.status.success());

    let report = tmp.path().join("duplicate_file_report.txt");
    assert!(report.exists());
    let content = fs::read_to_string(report).expect("read report");
    assert!(content.contains("Duplicate File Finder Report"));
}

#[test]
fn no_arguments_scans_current_directory() {
    let tmp = tempdir().expect("create temp dir");
    copy_dir_recursive(Path::new("resources"), tmp.path()).expect("copy resources");

    let output = run_with_args(tmp.path(), &[]);
    assert!(output.status.success());

    let report = tmp.path().join("duplicate_file_report.txt");
    assert!(report.exists());
}

#[test]
fn output_file_argument_creates_file() {
    let tmp = tempdir().expect("create temp dir");
    let input_dir = tmp.path().join("data");
    copy_dir_recursive(Path::new("resources"), &input_dir).expect("copy resources");

    let report = tmp.path().join("my_report.txt");
    let output = run_with_args(
        tmp.path(),
        &[
            input_dir.to_str().expect("valid UTF-8"),
            "--output",
            report.to_str().expect("valid UTF-8"),
        ],
    );
    assert!(output.status.success());
    assert!(report.exists());
}

#[test]
fn output_directory_argument_creates_report_in_directory() {
    let tmp = tempdir().expect("create temp dir");
    let input_dir = tmp.path().join("data");
    copy_dir_recursive(Path::new("resources"), &input_dir).expect("copy resources");

    let out_dir = tmp.path().join("reports");
    fs::create_dir(&out_dir).expect("create output dir");
    let output = run_with_args(
        tmp.path(),
        &[
            input_dir.to_str().expect("valid UTF-8"),
            "--output",
            out_dir.to_str().expect("valid UTF-8"),
        ],
    );
    assert!(output.status.success());

    let report = out_dir.join("duplicate_file_report.txt");
    assert!(report.exists());
}

#[test]
fn invalid_directory_returns_error() {
    let tmp = tempdir().expect("create temp dir");
    let bad_dir = tmp.path().join("does_not_exist");
    let output = run_with_args(tmp.path(), &[bad_dir.to_str().expect("valid UTF-8")]);
    assert!(!output.status.success());
}

#[test]
fn report_contains_expected_duplicates() {
    let tmp = tempdir().expect("create temp dir");
    let input_dir = tmp.path().join("data");
    copy_dir_recursive(Path::new("resources"), &input_dir).expect("copy resources");

    let output = run_with_args(tmp.path(), &[input_dir.to_str().expect("valid UTF-8")]);
    assert!(output.status.success());
    let report = tmp.path().join("duplicate_file_report.txt");
    let content = fs::read_to_string(report).expect("read report");
    assert!(content.contains("text_file.txt"));
    assert!(content.contains("text_file (Copy).txt"));
    assert!(content.contains("1_GI-td9gs8D5OKZd19mAOqA.png"));
}

#[test]
fn multiple_directories_scan() {
    let tmp = tempdir().expect("create temp dir");
    let input_dir1 = tmp.path().join("data1");
    let input_dir2 = tmp.path().join("data2");
    copy_dir_recursive(Path::new("resources"), &input_dir1).expect("copy resources");
    copy_dir_recursive(Path::new("resources"), &input_dir2).expect("copy resources");

    let output = run_with_args(
        tmp.path(),
        &[
            "--directories",
            input_dir1.to_str().expect("valid UTF-8"),
            input_dir2.to_str().expect("valid UTF-8"),
        ],
    );
    assert!(output.status.success());
    let report = tmp.path().join("duplicate_file_report.txt");
    assert!(report.exists());
    let content = fs::read_to_string(&report).expect("read report");
    assert!(content.contains(input_dir1.to_str().expect("valid UTF-8")));
    assert!(content.contains(input_dir2.to_str().expect("valid UTF-8")));
}
