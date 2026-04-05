use std::collections::BTreeMap;
use std::ffi::CString;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::thread::sleep;
use std::time::Duration;

use clap::Parser;
use serde::Deserialize;

extern crate sdl3;
use sdl3::Sdl;
use sdl3::event::Event;
use sdl3::keyboard::Keycode;
use sdl3::video::Window;
use sdl3_sys::vulkan::*;

use ash::{self, vk, Entry};
use ash::vk::*;

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
    println!();

    let context  = sdl3::init().unwrap();
    let window   = create_window(&context);
    let instance = create_vulkan_instance(&window);

    let mut event_pump = context.event_pump().unwrap();
    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit {..} |
                Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    break 'running
                },
                _ => {}
            }
        }

        sleep(Duration::new(0, 1_000_000_000u32 / 60));
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
        map.insert(key, entry);
    }
}

fn create_window(sdl_context: &Sdl) -> Window {
    let video_subsystem = sdl_context.video().unwrap();

    return video_subsystem
        .window("Solar/RS", 800, 600)
        .position_centered()
        .resizable()
        .vulkan()
        .build()
        .unwrap();
}

fn create_vulkan_instance(window: &Window) -> Entry {
    println!("Initializing Vulkan...");

    let entry = unsafe {
        match Entry::load() {
            Ok(result) => result,
            Err(msg) => {
                panic!("Unable to initialize Vulkan: {}", msg);
            }
        }
    };

    dump_vulkan_version(&entry);
    dump_vulkan_extensions(&entry);

    let app_name = CString::new("Solar/RS").unwrap();
    let app_info = vk::ApplicationInfo::default()
        .application_name(&app_name)
        .application_version(vk::make_api_version(0, 1, 0, 0))
        .api_version(vk::API_VERSION_1_3);

    let mut extensions = Vec::new();
    extensions.push(vk::KHR_PORTABILITY_ENUMERATION_NAME.as_ptr());

    let create_flags = vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR;
    let create_info = vk::InstanceCreateInfo::default()
        .application_info(&app_info)
        .enabled_extension_names(&extensions)
        .flags(create_flags);

    let instance = unsafe {
        entry
            .create_instance(&create_info, None)
            .expect("Unable to create Vulkan instance")
    };

    let mut surface = vk::SurfaceKHR::null();
    unsafe {
        SDL_Vulkan_CreateSurface(
            window.raw(),
            instance.handle(),
            std::ptr::null(),
            &mut surface as *mut _ as *mut _
        );
    }

    return entry;
}

fn dump_vulkan_version(entry: &Entry) {
    let version = unsafe {
        entry
            .try_enumerate_instance_version()
            .expect("Unable to enumerate Vulkan instance version")
    };

    let api_version = match version {
        Some(version) => version,
        None => API_VERSION_1_0
    };

    let major = vk::api_version_major(api_version);
    let minor = vk::api_version_minor(api_version);
    let patch = vk::api_version_patch(api_version);

    println!("# Vulkan {}.{}.{}", major, minor, patch);
}

fn dump_vulkan_extensions(entry: &Entry) {
    let extensions = unsafe {
        entry
            .enumerate_instance_extension_properties(None)
            .expect("Unable to enumerate Vulkan extensions")
    };

    for extension in extensions {
        let name = extension.extension_name_as_c_str().unwrap();
        println!("# {}", name.to_string_lossy());
    }
}
