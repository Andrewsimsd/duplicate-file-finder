use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::hash::Hasher;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use twox_hash::XxHash64;
use sha2::{Digest, Sha256};

/// Converts a file size in bytes to a human-readable format (GB, MB, KB).
fn format_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if size >= GB {
        format!("{:.2} GB", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.2} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.2} KB", size as f64 / KB as f64)
    } else {
        format!("{} bytes", size)
    }
}

/// Calculates a fast hash for the first few KB of a file to quickly detect differences.
fn quick_hash(file_path: &Path) -> Option<u64> {
    let mut hasher = XxHash64::with_seed(0);
    let file = File::open(file_path).ok()?;
    let mut reader = BufReader::new(file);
    let mut buffer = [0; 8192]; // Read only the first 8KB for a quick hash
    let bytes_read = reader.read(&mut buffer).ok()?;
    
    hasher.write(&buffer[..bytes_read]);
    Some(hasher.finish())
}

/// Computes a full cryptographic hash (SHA-256) for file comparison.
fn full_hash(file_path: &Path) -> Option<String> {
    let file = File::open(file_path).ok()?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0; 65536]; // 64KB buffer size for efficient hashing
    
    while let Ok(bytes_read) = reader.read(&mut buffer) {
        if bytes_read == 0 { break; }
        hasher.update(&buffer[..bytes_read]);
    }

    Some(format!("{:x}", hasher.finalize()))
}

/// Finds duplicate files in the given directory.
fn find_duplicates(dir: &Path) -> HashMap<u64, Vec<PathBuf>> {
    let mut size_map: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    
    // Group files by size
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Ok(metadata) = path.metadata() {
                    size_map.entry(metadata.len()).or_default().push(path);
                }
            }
        }
    }

    let mut potential_dupes: HashMap<u64, Vec<PathBuf>> = HashMap::new();

    // Use quick hash for fast grouping
    for (_size, files) in size_map.into_iter().filter(|(_, f)| f.len() > 1) {
        let mut quick_hash_map: HashMap<u64, Vec<PathBuf>> = HashMap::new();
        
        for file in files {
            if let Some(qh) = quick_hash(&file) {
                quick_hash_map.entry(qh).or_default().push(file);
            }
        }

        for (_qh, group) in quick_hash_map.into_iter().filter(|(_, g)| g.len() > 1) {
            potential_dupes.insert(_qh, group);
        }
    }

    let mut duplicates: HashMap<u64, Vec<PathBuf>> = HashMap::new();

    // Use full hash for final comparison
    for (_qh, files) in potential_dupes {
        let mut hash_map: HashMap<String, Vec<PathBuf>> = HashMap::new();
        
        for file in files {
            if let Some(fh) = full_hash(&file) {
                hash_map.entry(fh).or_default().push(file);
            }
        }

        for (_fh, group) in hash_map.into_iter().filter(|(_, g)| g.len() > 1) {
            let size = fs::metadata(&group[0]).ok().map(|m| m.len()).unwrap_or(0);
            duplicates.insert(size, group);
        }
    }

    duplicates
}

/// Writes duplicate files to an output file sorted by file size.
fn write_output(duplicates: HashMap<u64, Vec<PathBuf>>, output_file: &str) {
    let mut entries: Vec<(u64, Vec<PathBuf>)> = duplicates.into_iter().collect();
    entries.sort_by(|a, b| b.0.cmp(&a.0)); // Sort by file size descending

    let mut file = File::create(output_file).expect("Unable to create output file");

    for (size, paths) in entries {
        writeln!(file, "Size: {}", format_size(size)).unwrap();
        for path in paths {
            writeln!(file, "{}", path.display()).unwrap();
        }
        writeln!(file, "").unwrap();
    }
}

fn main() {
    let dir = Path::new("/home/andrew/Documents/GitHub/duplicate-file-finder/resources");
    let output_file = "duplicates.txt";

    println!("Scanning directory: {}", dir.display());
    let duplicates = find_duplicates(dir);

    if duplicates.is_empty() {
        println!("No duplicate files found.");
    } else {
        write_output(duplicates, output_file);
        println!("Duplicate files saved to {}", output_file);
    }
}

