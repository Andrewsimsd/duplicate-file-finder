use std::path::PathBuf;
use clap::Parser;
use dialoguer::{MultiSelect, Input};
use log::{error, info};
use chrono::Local;
use duplicate_file_finder::{setup_logger, find_duplicates, find_duplicates_in_dirs, write_output};

const VERSION: &str = "0.1.2";
const DEFAULT_REPORT_FILENAME: &str = "duplicate_file_report.txt";

#[derive(Parser)]
#[command(
    author,
    version = VERSION,
    about = "Scans the specified directory recursively for duplicate files."
)]
struct Cli {
    /// Directory to scan for duplicates
    directory: Option<PathBuf>,

    /// One or more directories to scan for duplicates
    #[arg(short = 'd', long = "directories", value_name = "DIR", num_args = 1..)]
    directories: Option<Vec<PathBuf>>,

    /// Launch interactive TUI
    #[arg(long)]
    gui: bool,

    /// Output file or directory for the report
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,
}

fn run_gui() {
    let mut dirs: Vec<PathBuf> = Vec::new();
    let entries: Vec<PathBuf> = std::fs::read_dir(".")
        .unwrap()
        .filter_map(|e| {
            let p = e.ok()?.path();
            if p.is_dir() { Some(p) } else { None }
        })
        .collect();

    if entries.is_empty() {
        println!("No directories found in current location");
        return;
    }

    let options: Vec<String> = entries.iter().map(|p| p.display().to_string()).collect();
    let selections = MultiSelect::new()
        .with_prompt("Select directories to scan")
        .items(&options)
        .interact()
        .unwrap();

    if selections.is_empty() {
        println!("No directories selected. Exiting.");
        return;
    }

    for i in selections {
        dirs.push(entries[i].clone());
    }

    let output: String = Input::new()
        .with_prompt("Output file")
        .default(String::from(DEFAULT_REPORT_FILENAME))
        .interact_text()
        .unwrap();

    let mut output_file = PathBuf::from(output);
    if output_file.is_dir() {
        output_file = output_file.join(DEFAULT_REPORT_FILENAME);
    }

    let start_time = Local::now().format("%Y%m%d %H:%M:%S").to_string();

    let duplicates = if dirs.len() == 1 {
        find_duplicates(&dirs[0])
    } else {
        find_duplicates_in_dirs(&dirs)
    };

    if duplicates.is_empty() {
        println!("No duplicate files found.");
    } else if let Err(e) = write_output(duplicates, output_file.to_str().unwrap(), &start_time, &dirs) {
        eprintln!("Error writing output: {}", e);
    } else {
        println!("Duplicate file report saved to {}", output_file.display());
    }
}

fn main() {
    setup_logger().expect("Failed to initialize logger");

    let cli = Cli::parse();

    if cli.gui {
        run_gui();
        return;
    }

    if cli.directory.is_none() && cli.directories.is_none() {
        eprintln!("Error: no directory specified. Use <directory> or --directories, or --gui for interactive mode.");
        std::process::exit(1);
    }

    let dirs: Vec<PathBuf> = if let Some(multi) = cli.directories.clone() {
        multi
    } else {
        vec![cli.directory.clone().unwrap()]
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
        match write_output(duplicates, output_file.to_str().unwrap(), &start_time, &dirs) {
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
