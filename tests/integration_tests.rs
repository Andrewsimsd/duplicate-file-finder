use std::fs;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn test_duplicate_finder_cli() {
    let dir = tempdir().unwrap();

    // Create some files with duplicate content
    let file1 = dir.path().join("file1.txt");
    let file2 = dir.path().join("file2.txt");
    let unique_file = dir.path().join("unique.txt");

    fs::write(&file1, "Duplicate content").unwrap();
    fs::write(&file2, "Duplicate content").unwrap();
    fs::write(&unique_file, "Unique content").unwrap();

    // Run the binary as a subprocess
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg(dir.path()) // Pass temp directory as argument
        .output()
        .expect("Failed to execute process");

    // Ensure the program ran successfully
    assert!(output.status.success());

    // Read the output file
    let output_file = "duplicates.txt";
    let output_contents = fs::read_to_string(output_file)
        .expect("Failed to read output file");

    // Verify that duplicates are correctly reported
    assert!(output_contents.contains("Duplicate File Finder Report"));
    assert!(output_contents.contains(file1.to_str().unwrap()));
    assert!(output_contents.contains(file2.to_str().unwrap()));

    // Verify that unique files are NOT reported as duplicates
    assert!(!output_contents.contains(unique_file.to_str().unwrap()));

    // Cleanup: Remove the generated `duplicates.txt`
    fs::remove_file(output_file).unwrap();
}
