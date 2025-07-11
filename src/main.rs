#![warn(clippy::pedantic)]

use chrono::Local;
use clap::{ArgGroup, Parser};
use duplicate_file_finder::{find_duplicates, find_duplicates_in_dirs, setup_logger, write_output};
use log::{error, info};
use std::path::PathBuf;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_REPORT_FILENAME: &str = "duplicate_file_report.txt";

#[derive(Parser)]
#[command(
    author,
    version = VERSION,
    about = "Scans the specified directory recursively for duplicate files.",
    group = ArgGroup::new("input").args(["directory", "directories"])
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
    } else if let Some(dir) = cli.directory.clone() {
        vec![dir]
    } else {
        vec![std::env::current_dir().expect("cannot determine current directory")]
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
        info!(
            "Starting duplicate file detection across {} directories",
            dirs.len()
        );
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
