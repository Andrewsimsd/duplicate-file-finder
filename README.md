[![Crates.io](https://img.shields.io/crates/v/duplicate_file_finder.svg)](https://crates.io/crates/duplicate_file_finder)
[![Documentation](https://docs.rs/duplicate_file_finder/badge.svg)](https://docs.rs/duplicate_file_finder)
[![CI](https://github.com/Andrewsimsd/duplicate_file_finder/actions/workflows/CI.yml/badge.svg)](https://github.com/Andrewsimsd/duplicate_file_finder/actions)
[![License](https://img.shields.io/crates/l/duplicate_file_finder)](LICENSE)
[![GitHub](https://img.shields.io/github/stars/Andrewsimsd/duplicate_file_finder?style=social)](https://github.com/Andrewsimsd/duplicate-file-finder)
# Duplicate File Finder

A simple yet efficient program written in Rust to detect and report duplicate files within a specified directory and its subdirectories. The program leverages hashing techniques to ensure accuracy while minimizing runtime overhead. It uses a terminal progress indicator and logs key events, making it both user-friendly and reliable.

## Features

- **Recursively checks directories** for duplicate files.
- **Efficient hashing** using quick hash (XXHash) and full hash (SHA-256) for accurate file comparison.
- **Progress bars** for scanning and hashing files (using `indicatif`).
- **Logs** events to a log file with `fern`, formatted in `YYYYMMDD` for the date.
- **Generates a report** of duplicate files, ordered by file size, in a human-readable format (KB, MB, GB, TB).

## Requirements

- Rust 1.60+ (or use `cargo` for managing dependencies)
- Dependencies: `sha2`, `twox-hash`, `walkdir`, `indicatif`, `fern`, `log`, `chrono`

## Installation

1. **Clone the repository**:
    ```bash
    git clone https://github.com/Andrewsimsd/duplicate-finder.git
    cd duplicate-finder
    ```

2. **Install dependencies**:
    ```bash
    cargo build
    ```

## Usage

To use the Duplicate Finder, run the following command:

```bash
cargo run -- <path_to_directory>
```

### Example:
```bash
cargo run -- /home/user/documents
```

This command will start scanning the specified directory (`/home/user/documents` in this case) for duplicate files. It will recursively scan all subdirectories, compare files based on their hashes, and output a report of duplicates into a file named `duplicates.txt`. The report will list duplicate files grouped by size and include full file paths.

Additionally, the program will log all actions to `duplicate_finder.log`, using the `YYYYMMDD` format for the timestamp.

## Log File

The log file, `duplicate_finder.log`, will contain detailed information about the program's execution. For example:
```
[20250125 14:32:12] [INFO] Starting duplicate file detection in /home/user/Documents
[20250125 14:32:12] [INFO] Scanning 1500 files in /home/user/Documents
[20250125 14:35:18] [INFO] Duplicate detection completed.
[20250125 14:35:19] [INFO] Duplicate files saved to duplicates.txt
```

## Output

The output file `duplicates.txt` will contain a list of duplicate files, ordered by size (from largest to smallest). Each entry will include the file size and the full file paths of the duplicates.

Example:
```
Size: 1.45 GB
/home/user/documents/file1.txt
/home/user/documents/subdir/file1.txt

Size: 500.23 MB
/home/user/documents/file2.txt
/home/user/documents/subdir/file2.txt
```

## Logging

The logger is set up using the `fern` crate and logs important events such as:

- When scanning starts and finishes.
- When duplicate detection is completed.
- When duplicate files are saved to the output file.

Logs are written to `duplicate_finder.log` and are formatted with the `YYYYMMDD` date format.

## Performance Considerations

This program is optimized for speed by:

- First performing a **quick hash** (XXHash) on the beginning of each file, reducing unnecessary full hashing of identical files.
- Using **SHA-256** for final verification only when necessary, ensuring accurate comparisons.

## Contributing

Feel free to fork the repository, submit issues, or create pull requests. We welcome contributions to improve performance or add new features!

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
