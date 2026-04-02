use std::path::Path;
use std::process::ExitCode;

use clap::Parser;

/// Solar system simulator
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to data directory
    #[arg(short, long, default_value = "data")]
    data: String
}

fn main() -> ExitCode {
    let args = Args::parse();

    println!("Solar system simulator");
    println!("Copyright (c) 2026, Dmitry Sednev <dmitry@sednev.ru>");
    println!();

    let data_dir = match Path::new(&args.data).canonicalize() {
        Ok(dir) => {
            if !dir.is_dir() {
                println!("Not a directory: {:?}", dir);
                return ExitCode::FAILURE;
            }

            dir
        }
        Err(msg) => {
            println!("Invalid path: {}: {}", args.data, msg);
            return ExitCode::FAILURE;
        }
    };

    println!("Reading data from {:?}...", data_dir);
    for entry in std::fs::read_dir(&data_dir).unwrap() {
        match entry {
            Ok(path) => {
                println!("# {}", path.path().display());
            }
            Err(msg) => {
                println!("Error: {}", msg);
            }
        }
    }

    return ExitCode::SUCCESS;
}
