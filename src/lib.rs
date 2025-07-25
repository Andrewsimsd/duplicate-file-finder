#![warn(clippy::pedantic)]

use chrono::Local;
use fern::Dispatch;
use indicatif::{ProgressBar, ProgressStyle};
use log::info;
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::error::Error;
use std::fs::{self, File};
use std::hash::Hasher;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use twox_hash::XxHash64;
use walkdir::WalkDir;

/// Initializes logging for the library and command line tool.
///
/// The logger records messages to a file called `duplicate_finder.log` and
/// formats each entry with a timestamp and log level. Call this once near the
/// start of your program before emitting any log messages.
///
/// # Errors
/// Returns a [`fern::InitError`] if the logger fails to initialize.
///
/// # Example
/// ```
/// use duplicate_file_finder::setup_logger;
/// use log::info;
///
/// fn init() -> Result<(), fern::InitError> {
///     setup_logger()?;
///     info!("logging ready");
///     Ok(())
/// }
/// ```
pub fn setup_logger() -> Result<(), fern::InitError> {
    Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}] [{}] {}",
                Local::now().format("%Y%m%d %H:%M:%S"),
                record.level(),
                message
            ));
        })
        .level(log::LevelFilter::Info)
        .chain(fern::log_file("duplicate_finder.log")?)
        .apply()?;
    Ok(())
}

/// Recursively scans the provided directory for duplicate files.
///
/// Files are grouped by size, then by a quick non-cryptographic hash and
/// finally by a full SHA-256 hash. Only files matching at every stage are
/// returned as duplicates.
///
/// # Arguments
/// * `dir` - The root path to scan for duplicate files.
///
/// # Returns
/// A map from SHA‑256 hash to a list of files sharing that hash.
///
/// # Example
/// ```
/// use duplicate_file_finder::find_duplicates;
/// use tempfile::tempdir;
///
/// fn check() -> std::io::Result<()> {
///     let dir = tempdir()?;
///     std::fs::write(dir.path().join("a.txt"), b"same")?;
///     std::fs::write(dir.path().join("b.txt"), b"same")?;
///     let dupes = find_duplicates(dir.path());
///     assert_eq!(dupes.values().next().unwrap().len(), 2);
///     Ok(())
/// }
/// ```
#[must_use]
pub fn find_duplicates(dir: &Path) -> HashMap<String, Vec<PathBuf>> {
    find_duplicates_in_dirs(&[dir.to_path_buf()])
}

/// Recursively scans the given directories for duplicate files.
///
/// Files are grouped by size and a quick hash of their first 8 KiB before
/// verifying equality with a full SHA‑256 hash. Only paths that match at every
/// stage are returned.
///
/// # Arguments
/// * `dirs` - The directories to search for duplicates.
///
/// # Returns
/// A map from SHA‑256 hash to a list of files sharing that hash.
///
/// # Example
/// ```
/// use duplicate_file_finder::find_duplicates_in_dirs;
/// use tempfile::tempdir;
/// use std::fs;
///
/// fn demo() -> std::io::Result<()> {
///     let d1 = tempdir()?;
///     let d2 = tempdir()?;
///     fs::write(d1.path().join("a.txt"), b"same")?;
///     fs::write(d2.path().join("b.txt"), b"same")?;
///     let dupes = find_duplicates_in_dirs(&[
///         d1.path().to_path_buf(),
///         d2.path().to_path_buf(),
///     ]);
///     assert_eq!(dupes.values().next().unwrap().len(), 2);
///     Ok(())
/// }
/// ```
#[allow(clippy::module_name_repetitions)]
#[must_use]
pub fn find_duplicates_in_dirs(dirs: &[PathBuf]) -> HashMap<String, Vec<PathBuf>> {
    let style =
        ProgressStyle::with_template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("█>-");

    let files = collect_files(dirs);
    info!(
        "{} files identified across {} directories",
        files.len(),
        dirs.len()
    );
    println!(
        "{} files identified across {} directories",
        files.len(),
        dirs.len()
    );

    let by_size = group_by_size(&files, &style);
    let by_quick_hash = group_by_quick_hash(by_size, &style);
    let duplicates = group_by_full_hash(by_quick_hash, &style);

    info!("{} duplicate files identified.", duplicates.len());
    duplicates
}

/// Walks all provided directories and returns a flat list of file paths.
///
/// # Arguments
/// * `dirs` - Directories to traverse recursively.
///
/// # Returns
/// A vector containing the full paths of every file found.
#[must_use]
fn collect_files(dirs: &[PathBuf]) -> Vec<PathBuf> {
    dirs.iter()
        .flat_map(|dir| {
            WalkDir::new(dir)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|entry| entry.path().is_file())
                .map(|entry| entry.path().to_path_buf())
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Groups files by size and returns a map keyed by file length.
///
/// # Arguments
/// * `files` - List of file paths to examine.
/// * `style` - Progress bar style shared across stages.
///
/// # Returns
/// A map from file size to the files with that length.
fn group_by_size(files: &[PathBuf], style: &ProgressStyle) -> HashMap<u64, Vec<PathBuf>> {
    let progress = ProgressBar::new(files.len() as u64);
    progress.set_style(style.clone());
    progress.set_message("Indexing files by size...");

    let size_entries: Vec<(u64, PathBuf)> = files
        .par_iter()
        .filter_map(|file| {
            let size = file.metadata().ok()?.len();
            progress.inc(1);
            Some((size, file.clone()))
        })
        .collect();

    let mut size_map: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    for (size, path) in size_entries {
        size_map.entry(size).or_default().push(path);
    }

    progress.finish_with_message("File sizes indexed.");
    info!("{} file sizes identified.", size_map.len());
    println!("{} file sizes identified.", size_map.len());
    size_map
}

/// Filters size groups using a quick hash of the first 8 KiB of each file.
///
/// # Arguments
/// * `size_map` - Files grouped by size from [`group_by_size`].
/// * `style` - Progress bar style shared across stages.
///
/// # Returns
/// A map from quick hash to files sharing that hash.
fn group_by_quick_hash(
    size_map: HashMap<u64, Vec<PathBuf>>,
    style: &ProgressStyle,
) -> HashMap<u64, Vec<PathBuf>> {
    let progress = ProgressBar::new(size_map.len() as u64);
    progress.set_style(style.clone());
    progress.set_message("Computing quick hashes...");

    let potential_dupes: HashMap<u64, Vec<PathBuf>> = size_map
        .into_par_iter()
        .filter(|(_, files)| files.len() > 1)
        .flat_map_iter(|(_, files)| {
            let mut quick_hash_map: HashMap<u64, Vec<PathBuf>> = HashMap::new();
            for file in files {
                if let Some(qh) = quick_hash(&file) {
                    quick_hash_map.entry(qh).or_default().push(file);
                }
            }
            progress.inc(1);
            quick_hash_map
                .into_iter()
                .filter(|(_, group)| group.len() > 1)
                .collect::<Vec<_>>()
        })
        .collect();

    progress.finish_with_message("Quick hashes complete.");
    info!("{} unique quick hashes identified.", potential_dupes.len());
    println!("{} unique quick hashes identified.", potential_dupes.len());
    potential_dupes
}

/// Performs full SHA‑256 hashing on candidates and groups confirmed duplicates.
///
/// # Arguments
/// * `potential_dupes` - Files that matched in [`group_by_quick_hash`].
/// * `style` - Progress bar style shared across stages.
///
/// # Returns
/// A map from SHA-256 hash to all files with identical content.
fn group_by_full_hash(
    potential_dupes: HashMap<u64, Vec<PathBuf>>,
    style: &ProgressStyle,
) -> HashMap<String, Vec<PathBuf>> {
    let total_files = potential_dupes.values().map(Vec::len).sum::<usize>() as u64;
    let progress = ProgressBar::new(total_files);
    progress.set_style(style.clone());
    progress.set_message("Computing full hashes...");

    let duplicates: HashMap<String, Vec<PathBuf>> = potential_dupes
        .into_par_iter()
        .flat_map_iter(|(_qh, files)| {
            let mut hash_map: HashMap<String, Vec<PathBuf>> = HashMap::new();
            for file in files {
                if let Some(fh) = full_hash(&file) {
                    hash_map.entry(fh).or_default().push(file);
                }
                progress.inc(1);
            }
            hash_map
                .into_iter()
                .filter(|(_, g)| g.len() > 1)
                .collect::<Vec<_>>()
        })
        .collect();

    progress.finish_with_message("Full hashes computed.");
    duplicates
}

/// Writes a report of duplicate files to a specified output file, including metadata such as
/// the user who generated the report, the start and end time, and the base directory scanned.
///
/// The duplicate file entries are sorted in descending order by file size. Each group of duplicates
/// is listed with its size followed by the full paths to the duplicate files.
///
/// # Arguments
///
/// * `duplicates` - A map where each key is a SHA-256 hash and the value is a list of file paths
///   that share that hash (i.e., files with the same content).
/// * `output_file` - The path to the output file where the report should be written.
/// * `start_time` - A string representing the start time of the operation (usually formatted as `YYYYMMDD HH:MM:SS`).
/// * `base_dirs` - The directory or directories searched for duplicates. Each will be
///   listed in the report header.
///
/// # Returns
///
/// Returns `Ok(())` if the report is written successfully. Returns an error if the output file cannot be created
/// or written to.
///
/// # Errors
///
/// This function will return an error if the output file cannot be created, or if any I/O operation
/// (e.g., writing to the file) fails.
///
/// # Example
///
/// ```
/// use std::collections::HashMap;
/// use std::path::PathBuf;
/// use duplicate_file_finder::{write_output};
/// fn example_usage() -> Result<(), Box<dyn std::error::Error>> {
///     let mut duplicates = HashMap::new();
///     duplicates.insert(
///         String::from("somehash"),
///         vec![PathBuf::from("/tmp/file1.txt"), PathBuf::from("/tmp/file2.txt")],
///     );
///
///     let start_time = "20250707 15:00:00";
///     let output_file = "duplicates.txt";
///     let base_dirs = &[PathBuf::from("/tmp")];
///
///     write_output(duplicates, output_file, start_time, base_dirs)?;
///     Ok(())
/// }
/// ```
///
pub fn write_output<S: ::std::hash::BuildHasher>(
    duplicates: HashMap<String, Vec<PathBuf>, S>,
    output_file: &str,
    start_time: &str,
    base_dirs: &[PathBuf],
) -> Result<(), Box<dyn Error>> {
    let mut entries: Vec<(u64, Vec<PathBuf>)> = duplicates
        .into_values()
        .map(|paths| {
            let size = fs::metadata(&paths[0]).map(|m| m.len()).unwrap_or(0);
            (size, paths)
        })
        .collect();
    entries.sort_by(|a, b| b.0.cmp(&a.0)); // Sort by file size descending

    let username = whoami::username();
    let end_time = Local::now().format("%Y%m%d %H:%M:%S").to_string();

    let file = File::create(output_file)?;
    let mut writer = BufWriter::new(file);

    // Write header
    writeln!(writer, "Duplicate File Finder Report")?;
    writeln!(writer, "Generated by: {username}")?;
    writeln!(writer, "Start Time: {start_time}")?;
    writeln!(writer, "End Time: {end_time}")?;
    if base_dirs.len() == 1 {
        writeln!(writer, "Base Directory: {}", base_dirs[0].display())?;
    } else {
        writeln!(writer, "Base Directories:")?;
        for dir in base_dirs {
            writeln!(writer, " - {}", dir.display())?;
        }
    }
    writeln!(writer)?;

    // Calculate potential space savings
    let total_savings: u64 = entries
        .iter()
        .map(|(size, paths)| size * (paths.len().saturating_sub(1) as u64))
        .sum();

    writeln!(
        writer,
        "Total Potential Space Savings: {}",
        format_size(total_savings)
    )?;
    writeln!(writer)?;

    // Write duplicate files
    for (size, paths) in entries {
        writeln!(writer, "Size: {}", format_size(size))?;
        for path in paths {
            writeln!(writer, "{}", path.display())?;
        }
        writeln!(writer)?;
    }

    info!("Duplicate files saved to {output_file}");
    Ok(())
}

/// Converts a file size in bytes to a human-readable string (e.g., "1.43 MB").
///
/// # Arguments
/// * `size` - File size in bytes.
///
/// # Returns
/// A `String` representing the size in the most appropriate unit (bytes, KB, MB, GB, or TB).
///
#[must_use]
#[allow(clippy::cast_precision_loss)]
fn format_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if size >= TB {
        format!("{:.2} TB", size as f64 / TB as f64)
    } else if size >= GB {
        format!("{:.2} GB", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.2} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.2} KB", size as f64 / KB as f64)
    } else {
        format!("{size} bytes")
    }
}

/// Computes a fast, non-cryptographic hash for a file based on its first 8 KB.
///
/// Used for quickly eliminating obviously different files.
///
/// # Arguments
/// * `file_path` - Path to the file to hash.
///
/// # Returns
/// An `Option<u64>` containing the hash value, or `None` if the file couldn't be read.
///
const QUICK_HASH_SIZE: usize = 8 * 1024;

#[must_use]
fn quick_hash(file_path: &Path) -> Option<u64> {
    let mut hasher = XxHash64::with_seed(0);
    let file = File::open(file_path).ok()?;
    let mut reader = BufReader::new(file);
    let mut buffer = [0; QUICK_HASH_SIZE];
    let bytes_read = reader.read(&mut buffer).ok()?;

    hasher.write(&buffer[..bytes_read]);
    Some(hasher.finish())
}

/// Computes a full SHA-256 hash of a file's contents.
///
/// Used in the final step of duplicate detection to confirm file identity.
///
/// # Arguments
/// * `file_path` - Path to the file to hash.
///
/// # Returns
/// An `Option<String>` with the lowercase hex representation of the SHA-256 hash,
/// or `None` if the file could not be read.
///
const FULL_HASH_BUFFER_SIZE: usize = 64 * 1024;

#[must_use]
#[allow(clippy::large_stack_arrays)]
fn full_hash(file_path: &Path) -> Option<String> {
    let file = File::open(file_path).ok()?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0; FULL_HASH_BUFFER_SIZE];

    while let Ok(bytes_read) = reader.read(&mut buffer) {
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Some(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 bytes");
        assert_eq!(format_size(1500), "1.46 KB");
        assert_eq!(format_size(1_500_000), "1.43 MB");
        assert_eq!(format_size(1_500_000_000), "1.40 GB");
        assert_eq!(format_size(1_500_000_000_000), "1.36 TB");
    }

    #[test]
    fn test_quick_hash() {
        let dir = tempdir().expect("create temp dir");
        let file_path = dir.path().join("test_file.txt");
        let mut file = File::create(&file_path).expect("create file");
        writeln!(file, "Hello, world!").expect("write file");

        let hash = quick_hash(&file_path);
        assert!(hash.is_some());
    }

    #[test]
    fn test_full_hash() {
        let dir = tempdir().expect("create temp dir");
        let file_path = dir.path().join("test_file.txt");
        let mut file = File::create(&file_path).expect("create file");
        writeln!(file, "Hello, world!").expect("write file");

        let hash = full_hash(&file_path);
        assert!(hash.is_some());
        assert_eq!(
            hash.expect("hash exists"),
            "d9014c4624844aa5bac314773d6b689ad467fa4e1d1a50a1b8a99d5a95f72ff5"
        ); // Precomputed SHA-256 of "Hello, world!\n"
    }

    #[test]
    fn test_find_duplicates() {
        let dir = tempdir().expect("create temp dir");

        // Create some duplicate files
        let file1 = dir.path().join("file1.txt");
        let file2 = dir.path().join("file2.txt");
        let unique_file = dir.path().join("unique.txt");

        fs::write(&file1, "Duplicate content").expect("write file");
        fs::write(&file2, "Duplicate content").expect("write file");
        fs::write(&unique_file, "Unique content").expect("write file");

        let duplicates = find_duplicates(dir.path());
        assert_eq!(duplicates.len(), 1); // Only one group of duplicates
        let duplicate_group = duplicates.values().next().expect("duplicates");
        assert_eq!(duplicate_group.len(), 2);
        assert!(duplicate_group.contains(&file1));
        assert!(duplicate_group.contains(&file2));
    }

    #[test]
    fn test_find_duplicates_in_dirs() {
        let dir1 = tempdir().expect("create temp dir");
        let dir2 = tempdir().expect("create temp dir");

        let file1 = dir1.path().join("file1.txt");
        let file2 = dir2.path().join("file2.txt");
        let unique = dir2.path().join("unique.txt");

        fs::write(&file1, "Duplicate content").expect("write file");
        fs::write(&file2, "Duplicate content").expect("write file");
        fs::write(&unique, "Unique content").expect("write file");

        let duplicates =
            find_duplicates_in_dirs(&[dir1.path().to_path_buf(), dir2.path().to_path_buf()]);
        assert_eq!(duplicates.len(), 1);
        let group = duplicates.values().next().expect("duplicates");
        assert_eq!(group.len(), 2);
        assert!(group.contains(&file1));
        assert!(group.contains(&file2));
    }

    #[test]
    fn test_write_output() {
        let dir = tempdir().expect("create temp dir");

        // Create some files and simulate duplicates
        let file1 = dir.path().join("file1.txt");
        let file2 = dir.path().join("file2.txt");
        fs::write(&file1, "Duplicate content").expect("write file");
        fs::write(&file2, "Duplicate content").expect("write file");

        let mut duplicates = HashMap::new();
        duplicates.insert("dummy_hash".to_string(), vec![file1.clone(), file2.clone()]);

        let output_file = dir.path().join("output.txt");
        let _res = write_output(
            duplicates,
            output_file.to_str().expect("valid UTF-8 path"),
            "20250101 12:00:00",
            &[dir.path().to_path_buf()],
        );

        let output = fs::read_to_string(&output_file).expect("read file");
        assert!(output.contains("Duplicate File Finder Report"));
        assert!(output.contains(file1.to_str().expect("valid UTF-8")));
        assert!(output.contains(file2.to_str().expect("valid UTF-8")));
    }
}
