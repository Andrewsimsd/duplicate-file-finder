#![warn(clippy::pedantic)]

use std::path::PathBuf;
use clap::{Parser, ArgGroup};
use log::{error, info};
use chrono::Local;
use duplicate_file_finder::{setup_logger, find_duplicates, find_duplicates_in_dirs, write_output};

const VERSION: &str = "0.1.2";
const DEFAULT_REPORT_FILENAME: &str = "duplicate_file_report.txt";

#[derive(Parser)]
#[command(
    author,
    version = VERSION,
    about = "Scans the specified directory recursively for duplicate files.",
    group = ArgGroup::new("input").required(true).args(["directory", "directories"])
)]
struct Cli {
    /// Directory to scan for duplicates
    #[arg(group = "input")]
    directory: Option<PathBuf>,

    /// One or more directories to scan for duplicates
    #[arg(short = 'd', long = "directories", value_name = "DIR", num_args = 1.., group = "input")]
    directories: Option<Vec<PathBuf>>,

    /// Output file or directory for the report
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,
}

fn main() {
    setup_logger().expect("Failed to initialize logger");

    let cli = Cli::parse();

    let dirs: Vec<PathBuf> = if let Some(multi) = cli.directories.clone() {
        multi
    } else {
        vec![cli.directory.clone().expect("a directory path is required")] 
    };

    let mut output_file = cli
        .output
        .unwrap_or_else(|| PathBuf::from(DEFAULT_REPORT_FILENAME));

    if output_file.is_dir() {
        output_file = output_file.join(DEFAULT_REPORT_FILENAME);
    }
    for d in &dirs {
        if !d.exists() || !d.is_dir() {
            eprintln!("Error: '{}' is not a valid directory", d.display());
            error!("Invalid directory: {}", d.display());
            std::process::exit(1);
        }
    }

    let start_time = Local::now().format("%Y%m%d %H:%M:%S").to_string();

    if dirs.len() == 1 {
        println!("Scanning directory: {}", dirs[0].display());
        info!("Starting duplicate file detection in {}", dirs[0].display());
    } else {
        println!("Scanning {} directories", dirs.len());
        info!("Starting duplicate file detection across {} directories", dirs.len());
    }
    println!("Output will be saved to: {}", output_file.display());

    let duplicates = if dirs.len() == 1 {
        find_duplicates(&dirs[0])
    } else {
        find_duplicates_in_dirs(&dirs)
    };

    if duplicates.is_empty() {
        println!("No duplicate files found.");
        info!("No duplicate files found.");
    } else {
        match write_output(
            duplicates,
            output_file.to_str().expect("valid UTF-8 path"),
            &start_time,
            &dirs,
        ) {
            Ok(()) => {
                println!("Duplicate file report saved to {}", output_file.display());
                info!("Duplicate file report saved to {}", output_file.display());
            }
            Err(e) => {
                eprintln!("Error writing output: {e}");
                error!("Failed to write output: {e}");
                std::process::exit(1);
            }
        }
    }
}
