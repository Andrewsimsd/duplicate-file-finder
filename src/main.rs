use std::env;
use std::path::Path;
use log::{error, info};
use chrono::Local;
use duplicate_file_finder::{setup_logger, find_duplicates, write_output};

const VERSION: &str = "0.1.2";
const DEFAULT_REPORT_FILENAME: &str = "duplicate_file_report.txt";

fn print_help(program_name: &str) {
    println!("Duplicate File Finder v{}", VERSION);
    println!("Usage: {} <directory> [--output <file>]", program_name);
    println!();
    println!("Scans the specified directory recursively for duplicate files.");
    println!("Outputs a report to the specified file, or 'duplicates.txt' by default.");
    println!();
    println!("Options:");
    println!("  -h, --help           Show this help message and exit");
    println!("  --output <file>      Specify the output file path for the report. If a directory is given, the report will be saved as {DEFAULT_REPORT_FILENAME}");
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

    let mut dir_arg = None;
    let mut output_file = "duplicate_file_report.txt".to_string();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help(program_name);
                return;
            }
            "--output" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --output requires a file or directory path");
                    std::process::exit(1);
                }
                let output_arg = Path::new(&args[i + 1]);

                // If it's a directory, append "output.txt"
                if output_arg.exists() && output_arg.is_dir() {
                    output_file = output_arg.join(DEFAULT_REPORT_FILENAME).to_string_lossy().into_owned();
                } else {
                    // Otherwise, treat as direct file path
                    output_file = output_arg.to_string_lossy().into_owned();
                }
                i += 1;
            }
            val if dir_arg.is_none() => {
                dir_arg = Some(val.to_string());
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
                print_help(program_name);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let dir_str = dir_arg.expect("Missing directory argument");
    let dir = Path::new(&dir_str);
    if !dir.exists() || !dir.is_dir() {
        eprintln!("Error: '{}' is not a valid directory", dir.display());
        error!("Invalid directory: {}", dir.display());
        std::process::exit(1);
    }

    let start_time = Local::now().format("%Y%m%d %H:%M:%S").to_string();

    println!("Scanning directory: {}", dir.display());
    println!("Output will be saved to: {}", output_file);
    info!("Starting duplicate file detection in {}", dir.display());

    let duplicates = find_duplicates(dir);

    if duplicates.is_empty() {
        println!("No duplicate files found.");
        info!("No duplicate files found.");
    } else {
        match write_output(duplicates, &output_file, &start_time, dir) {
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
