use std::path::PathBuf;
use clap::Parser;
use log::{error, info};
use chrono::Local;
use duplicate_file_finder::{setup_logger, find_duplicates, write_output};

const VERSION: &str = "0.1.2";
const DEFAULT_REPORT_FILENAME: &str = "duplicate_file_report.txt";

#[derive(Parser)]
#[command(author, version = VERSION, about = "Scans the specified directory recursively for duplicate files.")]
struct Cli {
    /// Directory to scan for duplicates
    directory: PathBuf,

    /// Output file or directory for the report
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,
}

fn main() {
    setup_logger().expect("Failed to initialize logger");

    let cli = Cli::parse();

    let dir = &cli.directory;
    let mut output_file = cli
        .output
        .unwrap_or_else(|| PathBuf::from(DEFAULT_REPORT_FILENAME));

    if output_file.is_dir() {
        output_file = output_file.join(DEFAULT_REPORT_FILENAME);
    }
    if !dir.exists() || !dir.is_dir() {
        eprintln!("Error: '{}' is not a valid directory", dir.display());
        error!("Invalid directory: {}", dir.display());
        std::process::exit(1);
    }

    let start_time = Local::now().format("%Y%m%d %H:%M:%S").to_string();

    println!("Scanning directory: {}", dir.display());
    println!("Output will be saved to: {}", output_file.display());
    info!("Starting duplicate file detection in {}", dir.display());

    let duplicates = find_duplicates(dir);

    if duplicates.is_empty() {
        println!("No duplicate files found.");
        info!("No duplicate files found.");
    } else {
        match write_output(duplicates, output_file.to_str().unwrap(), &start_time, dir) {
            Ok(()) => {
                println!("Duplicate file report saved to {}", output_file.display());
                info!("Duplicate file report saved to {}", output_file.display());
            }
            Err(e) => {
                eprintln!("Error writing output: {}", e);
                error!("Failed to write output: {}", e);
                std::process::exit(1);
            }
        }
    }
}
