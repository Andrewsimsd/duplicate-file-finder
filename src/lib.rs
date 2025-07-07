use std::collections::HashMap;
use std::fs::{self, File};
use std::hash::Hasher;
use std::io::{BufReader, Read, Write, BufWriter};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use twox_hash::XxHash64;
use sha2::{Digest, Sha256};
use indicatif::{ProgressBar};
use fern::Dispatch;
use log::{info};
use chrono::{Local};
use std::error::Error;

/// Initializes and configures logging for the application using the `fern` backend.
///
/// Logs are written to a file named `duplicate_finder.log` with timestamps and log levels.
/// This function should be called before any logging takes place.
///
/// # Errors
/// Returns a `fern::InitError` if the logger fails to initialize.
///
/// # Example
/// ```
/// use duplicate_file_finder::{setup_logger};
/// setup_logger().expect("Failed to initialize logger");
/// ```rust
#[must_use]
pub fn setup_logger() -> Result<(), fern::InitError> {
    Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}] [{}] {}",
                Local::now().format("%Y%m%d %H:%M:%S"),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .chain(fern::log_file("duplicate_finder.log")?)
        .apply()?;
    Ok(())
}


/// Recursively scans the provided directory for duplicate files.
///
/// The process groups files by size, then by a quick hash, and finally by a full SHA-256 hash
/// to identify true duplicates. Only files that match in size, quick hash, and full hash
/// are considered duplicates.
///
/// # Arguments
/// * `dir` - The root path to scan for duplicate files.
///
/// # Returns
/// A `HashMap` where the key is the file size (in bytes), and the value is a `Vec<PathBuf>`
/// containing paths to duplicate files of that size.
///
/// # Example
/// ```rust, nocompile
/// use duplicate_file_finder::{find_duplicates};
/// let duplicates = find_duplicates(Path::new("/some/directory"));
/// ```
pub fn find_duplicates(dir: &Path) -> HashMap<u64, Vec<PathBuf>> {
    let mut size_map: HashMap<u64, Vec<PathBuf>> = HashMap::new();  // Maps file sizes to paths
    let files: Vec<PathBuf> = WalkDir::new(dir)  // Recursively walk through the directory
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())  // Only consider files
        .map(|entry| entry.path().to_path_buf())  // Collect file paths
        .collect();

    info!("{} files identified in {}", files.len(), dir.display());

    let progress = ProgressBar::new(files.len() as u64);

    // Loop over all files to collect them by size to quickly filter out non-duplicates.
    for file in &files {
        if let Ok(metadata) = file.metadata() {
            size_map.entry(metadata.len()).or_default().push(file.clone());  // Group files by size
        }
        progress.inc(1);  // Update progress bar
    }
    progress.finish();  // Finish progress bar
    info!("{} file sizes identified.", size_map.len());

    let mut potential_dupes: HashMap<u64, Vec<PathBuf>> = HashMap::new();

    // Further filter files by quick hash
    for (_size, files) in size_map.into_iter().filter(|(_, f)| f.len() > 1) {
        let mut quick_hash_map: HashMap<u64, Vec<PathBuf>> = HashMap::new();
        
        for file in files {
            if let Some(qh) = quick_hash(&file) {
                quick_hash_map.entry(qh).or_default().push(file);  // Group by quick hash
            }
        }

        // Filter out groups with only one file, indicating no duplicates
        for (_qh, group) in quick_hash_map.into_iter().filter(|(_, g)| g.len() > 1) {
            potential_dupes.insert(_qh, group);  // Add groups with duplicates to potential duplicates
        }
    }
    info!("{} unique quick hashes identified.", potential_dupes.len());

    let mut duplicates: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    let total_files = potential_dupes.values().map(Vec::len).sum::<usize>() as u64;
    let progress = ProgressBar::new(total_files);

    // Final step: Perform full hashing (SHA-256) and group duplicates
    for (_qh, files) in potential_dupes {
        let mut hash_map: HashMap<String, Vec<PathBuf>> = HashMap::new();
        
        for file in files {
            if let Some(fh) = full_hash(&file, &progress) {
                hash_map.entry(fh).or_default().push(file);  // Group by full hash
            }
        }

        // Only keep groups of files with the same hash (duplicates)
        for (_fh, group) in hash_map.into_iter().filter(|(_, g)| g.len() > 1) {
            let size = fs::metadata(&group[0]).ok().map(|m| m.len()).unwrap_or(0);
            duplicates.insert(size, group);  // Store duplicates by size
        }
    }
    progress.finish();  // Finish progress bar

    info!("{} duplicate files identified.", duplicates.len());
    duplicates  // Return the found duplicates
}


/// Writes a report of duplicate files to a specified output file, including metadata such as
/// the user who generated the report, the start and end time, and the base directory scanned.
///
/// The duplicate file entries are sorted in descending order by file size. Each group of duplicates
/// is listed with its size followed by the full paths to the duplicate files.
///
/// # Arguments
///
/// * `duplicates` - A map where each key is a file size in bytes and the value is a list of file paths
///                  that are duplicates (i.e., files with the same content and size).
/// * `output_file` - The path to the output file where the report should be written.
/// * `start_time` - A string representing the start time of the operation (usually formatted as `YYYYMMDD HH:MM:SS`).
/// * `base_dir` - The base directory where the duplicate search was initiated. This is included in the report header.
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
/// ```rust
/// use std::collections::HashMap;
/// use std::path::PathBuf;
/// use duplicate_file_finder::{write_output};
/// fn example_usage() -> Result<(), Box<dyn std::error::Error>> {
///     let mut duplicates = HashMap::new();
///     duplicates.insert(
///         1024,
///         vec![PathBuf::from("/tmp/file1.txt"), PathBuf::from("/tmp/file2.txt")],
///     );
///
///     let start_time = "20250707 15:00:00";
///     let output_file = "duplicates.txt";
///     let base_dir = std::path::Path::new("/tmp");
///
///     write_output(duplicates, output_file, start_time, base_dir)?;
///     Ok(())
/// }
/// ```
/// 
pub fn write_output(
    duplicates: HashMap<u64, Vec<PathBuf>>,
    output_file: &str,
    start_time: &str,
    base_dir: &Path,
) -> Result<(), Box<dyn Error>> {
    let mut entries: Vec<(u64, Vec<PathBuf>)> = duplicates.into_iter().collect();
    entries.sort_by(|a, b| b.0.cmp(&a.0)); // Sort by file size descending

    let username = whoami::username();
    let end_time = Local::now().format("%Y%m%d %H:%M:%S").to_string();

    let file = File::create(output_file)?;
    let mut writer = BufWriter::new(file);

    // Write header
    writeln!(writer, "Duplicate File Finder Report")?;
    writeln!(writer, "Generated by: {}", username)?;
    writeln!(writer, "Start Time: {}", start_time)?;
    writeln!(writer, "End Time: {}", end_time)?;
    writeln!(writer, "Base Directory: {}", base_dir.display())?;
    writeln!(writer)?;

    // Write duplicate files
    for (size, paths) in entries {
        writeln!(writer, "Size: {}", format_size(size))?;
        for path in paths {
            writeln!(writer, "{}", path.display())?;
        }
        writeln!(writer)?;
    }

    info!("Duplicate files saved to {}", output_file);
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
        format!("{} bytes", size)
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
fn quick_hash(file_path: &Path) -> Option<u64> {
    let mut hasher = XxHash64::with_seed(0);
    let file = File::open(file_path).ok()?;
    let mut reader = BufReader::new(file);
    let mut buffer = [0; 8192];
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
/// * `progress` - Progress bar to update as file is read.
///
/// # Returns
/// An `Option<String>` with the lowercase hex representation of the SHA-256 hash,
/// or `None` if the file could not be read.
///
fn full_hash(file_path: &Path, progress: &ProgressBar) -> Option<String> {
    let file = File::open(file_path).ok()?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0; 65536];

    while let Ok(bytes_read) = reader.read(&mut buffer) {
        if bytes_read == 0 { break; }
        hasher.update(&buffer[..bytes_read]);
        progress.inc(1); // Update progress bar
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
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_file.txt");
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "Hello, world!").unwrap();

        let hash = quick_hash(&file_path);
        assert!(hash.is_some());
    }

    #[test]
    fn test_full_hash() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_file.txt");
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "Hello, world!").unwrap();

        let progress = ProgressBar::hidden(); // Use a hidden progress bar for tests
        let hash = full_hash(&file_path, &progress);
        assert!(hash.is_some());
        assert_eq!(
            hash.unwrap(),
            "d9014c4624844aa5bac314773d6b689ad467fa4e1d1a50a1b8a99d5a95f72ff5"
        ); // Precomputed SHA-256 of "Hello, world!\n"
    }

    #[test]
    fn test_find_duplicates() {
        let dir = tempdir().unwrap();

        // Create some duplicate files
        let file1 = dir.path().join("file1.txt");
        let file2 = dir.path().join("file2.txt");
        let unique_file = dir.path().join("unique.txt");

        fs::write(&file1, "Duplicate content").unwrap();
        fs::write(&file2, "Duplicate content").unwrap();
        fs::write(&unique_file, "Unique content").unwrap();

        let duplicates = find_duplicates(dir.path());
        assert_eq!(duplicates.len(), 1); // Only one group of duplicates
        let duplicate_group = duplicates.values().next().unwrap();
        assert_eq!(duplicate_group.len(), 2);
        assert!(duplicate_group.contains(&file1));
        assert!(duplicate_group.contains(&file2));
    }

    #[test]
    fn test_write_output() {
        let dir = tempdir().unwrap();

        // Create some files and simulate duplicates
        let file1 = dir.path().join("file1.txt");
        let file2 = dir.path().join("file2.txt");
        fs::write(&file1, "Duplicate content").unwrap();
        fs::write(&file2, "Duplicate content").unwrap();

        let mut duplicates = HashMap::new();
        duplicates.insert(
            file1.metadata().unwrap().len(),
            vec![file1.clone(), file2.clone()],
        );

        let output_file = dir.path().join("output.txt");
        let res = write_output(
            duplicates,
            output_file.to_str().unwrap(),
            "20250101 12:00:00",
            dir.path(),
        );

        let output = fs::read_to_string(&output_file).unwrap();
        assert!(output.contains("Duplicate File Finder Report"));
        assert!(output.contains(file1.to_str().unwrap()));
        assert!(output.contains(file2.to_str().unwrap()));
    }
}