use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::hash::Hasher;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use twox_hash::XxHash64;
use sha2::{Digest, Sha256};
use indicatif::{ProgressBar, ProgressStyle};
use fern::Dispatch;
use log::{info, error};
use chrono::{Local};


/// Sets up logging to a file
fn setup_logger() -> Result<(), fern::InitError> {
    Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}] [{}] {}",
                Local::now().format("%Y%m%d %H:%M:%S"), // Changed format to YYYYMMDD
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .chain(fern::log_file("duplicate_finder.log")?)
        .apply()?;
    Ok(())
}


/// Converts file size in bytes to a human-readable format (GB, MB, KB).
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

/// Calculates a fast hash for the first few KB of a file.
fn quick_hash(file_path: &Path) -> Option<u64> {
    let mut hasher = XxHash64::with_seed(0);
    let file = File::open(file_path).ok()?;
    let mut reader = BufReader::new(file);
    let mut buffer = [0; 8192];
    let bytes_read = reader.read(&mut buffer).ok()?;
    
    hasher.write(&buffer[..bytes_read]);
    Some(hasher.finish())
}

/// Computes a full cryptographic hash (SHA-256) for file comparison.
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

/// Finds duplicate files in the given directory recursively.
fn find_duplicates(dir: &Path) -> HashMap<u64, Vec<PathBuf>> {
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

/// Writes duplicate files to an output file with header information.
fn write_output(duplicates: HashMap<u64, Vec<PathBuf>>, output_file: &str, start_time: &str, base_dir: &Path) {
    let mut entries: Vec<(u64, Vec<PathBuf>)> = duplicates.into_iter().collect();
    entries.sort_by(|a, b| b.0.cmp(&a.0)); // Sort by file size descending

    let username = whoami::username();
    let end_time = Local::now().format("%Y%m%d %H:%M:%S").to_string();

    let mut file = File::create(output_file).expect("Unable to create output file");

    // Write header
    writeln!(file, "Duplicate File Finder Report").unwrap();
    writeln!(file, "Generated by: {}", username).unwrap();
    writeln!(file, "Start Time: {}", start_time).unwrap();
    writeln!(file, "End Time: {}", end_time).unwrap();
    writeln!(file, "Base Directory: {}", base_dir.display()).unwrap();
    writeln!(file, "").unwrap();

    // Write duplicate files
    for (size, paths) in entries {
        writeln!(file, "Size: {}", format_size(size)).unwrap();
        for path in paths {
            writeln!(file, "{}", path.display()).unwrap();
        }
        writeln!(file, "").unwrap();
    }

    info!("Duplicate files saved to {}", output_file);
}

fn main() {
    setup_logger().expect("Failed to initialize logger");

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        error!("Usage: duplicate_finder <directory>");
        eprintln!("Usage: duplicate_finder <directory>");
        return;
    }

    let dir = Path::new(&args[1]);
    if !dir.exists() || !dir.is_dir() {
        error!("Error: '{}' is not a valid directory", dir.display());
        eprintln!("Error: '{}' is not a valid directory", dir.display());
        return;
    }

    let start_time = Local::now().format("%Y%m%d %H:%M:%S").to_string();
    let output_file = "duplicates.txt";

    info!("Starting duplicate file detection in {}", dir.display());
    println!("Scanning directory: {}", dir.display());
    let duplicates = find_duplicates(dir);

    if duplicates.is_empty() {
        println!("No duplicate files found.");
        info!("No duplicate files found.");
    } else {
        write_output(duplicates, output_file, &start_time, dir);
        println!("Duplicate files saved to {}", output_file);
    }
}

// Existing imports and code ...

/// Unit tests module
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
        write_output(
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

