use std::env;
use std::path::Path;
use log::{info, error};
use chrono::{Local};
use duplicate_file_finder::{setup_logger, find_duplicates, write_output};



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
        match write_output(duplicates, output_file, &start_time, dir){
            Ok(())=>{
                println!("Duplicate files saved to {}", output_file);
                info!("Duplicate files saved to {}", output_file);
            },
            Err(e)=> eprintln!("Error: {e}"),
        }
        println!("Duplicate files saved to {}", output_file);
    }
}




