use std::env;
use std::path::Path;
use log::{error, info};
use chrono::Local;
use duplicate_file_finder::{setup_logger, find_duplicates, write_output};

const VERSION: &str = "0.1.2";

fn print_help(program_name: &str) {
    println!("Duplicate File Finder v{}", VERSION);
    println!("Usage: {} <directory>", program_name);
    println!();
    println!("Scans the specified directory recursively for duplicate files.");
    println!("Outputs a report to 'duplicates.txt' in the current working directory.");
    println!();
    println!("Options:");
    println!("  -h, --help       Show this help message and exit");
}

fn main() {
    setup_logger().expect("Failed to initialize logger");

    let args: Vec<String> = env::args().collect();
    let program_name = &args[0];

    if args.len() < 2 {
        eprintln!("Error: missing required argument <directory>");
        print_help(program_name);
        std::process::exit(1);
    }

    if args[1] == "--help" || args[1] == "-h" {
        print_help(program_name);
        return;
    }

    let dir = Path::new(&args[1]);
    if !dir.exists() || !dir.is_dir() {
        eprintln!("Error: '{}' is not a valid directory", dir.display());
        error!("Invalid directory: {}", dir.display());
        std::process::exit(1);
    }

    let start_time = Local::now().format("%Y%m%d %H:%M:%S").to_string();
    let output_file = "duplicates.txt";

    println!("Scanning directory: {}", dir.display());
    info!("Starting duplicate file detection in {}", dir.display());

    let duplicates = find_duplicates(dir);

    if duplicates.is_empty() {
        println!("No duplicate files found.");
        info!("No duplicate files found.");
    } else {
        match write_output(duplicates, output_file, &start_time, dir) {
            Ok(()) => {
                println!("Duplicate file report saved to {}", output_file);
                info!("Duplicate file report saved to {}", output_file);
            }
            Err(e) => {
                eprintln!("Error writing output: {}", e);
                error!("Failed to write output: {}", e);
                std::process::exit(1);
            }
        }
    }
}