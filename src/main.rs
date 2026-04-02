use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
 use std::sync::{Mutex, MutexGuard, OnceLock};

use clap::Parser;
use serde::Deserialize;

/// Solar system simulator
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to data directory
    #[arg(short, long, default_value = "data")]
    data: String
}

#[derive(Debug, Deserialize)]
enum CelestialType {
    STAR,
    PLANET,
    MOON
}

#[derive(Debug, Deserialize)]
struct CelestialSet {
    #[serde(flatten)]
    objects: BTreeMap<String, CelestialBody>
}

#[derive(Debug, Deserialize)]
struct CelestialBody {
    name: String,
    kind: CelestialType
}

fn main() -> ExitCode {
    let args = Args::parse();

    println!("Solar system simulator");
    println!("Copyright (c) 2026, Dmitry Sednev <dmitry@sednev.ru>");
    println!();

    let data_dir = get_data_dir(&args.data);
    println!("Reading data from {}...", data_dir.display());

    for entry in std::fs::read_dir(&data_dir).unwrap() {
        match entry {
            Ok(path) => {
                read_data(&path.path());
            }
            Err(msg) => {
                panic!("Error: {}", msg);
            }
        }
    }

    return ExitCode::SUCCESS;
}

fn get_data_dir(path: &String) -> PathBuf {
    let data_dir = match Path::new(path).canonicalize() {
        Ok(dir) => {
            if !dir.is_dir() {
                panic!("Not a directory: {:?}", dir);
            }

            dir
        }
        Err(msg) => {
            panic!("Invalid path: {}: {}", path, msg);
        }
    };

    data_dir
}

fn get_object_map() -> MutexGuard<'static, BTreeMap<String, CelestialBody>> {
    static MAP: OnceLock<Mutex<BTreeMap<String, CelestialBody>>> = OnceLock::new();

    MAP.get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .expect("Let's hope the lock isn't poisoned")
}

fn read_data(path: &Path) {
    println!("# {}", path.display());

    let mut file = File::open(path).unwrap();
    let mut contents = String::new();

    file.read_to_string(&mut contents).unwrap();

    let mut map = get_object_map();
    let data: CelestialSet = toml::from_str(&contents).unwrap();

    for (key, entry) in data.objects.into_iter() {
        println!("... {:?}", entry.name);
        map.insert(key, entry);
    }
}
