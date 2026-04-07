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
use tracing::{error, info};
use tracing_subscriber;
use tracing_subscriber::fmt;

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

    init_logging();
    load_data(get_data_dir(&args.data));

    let context  = sdl3::init().unwrap();
    let window   = create_window(&context);
    let instance = create_vulkan_instance(&window);

    let device = pick_physical_device(&instance);

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

fn init_logging() {
    fmt()
        .with_max_level(tracing::Level::DEBUG)
        .without_time()
        .with_target(false)
        .compact()
        .with_ansi(true)
        .init();
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

fn load_data(data_dir: PathBuf) {
    info!("Loading {}...", data_dir.display());

    for entry in std::fs::read_dir(&data_dir).unwrap() {
        match entry {
            Ok(path) => {
                load_data_file(&path.path());
            }
            Err(msg) => {
                panic!("Error: {}", msg);
            }
        }
    }
}

fn get_object_map() -> MutexGuard<'static, BTreeMap<String, CelestialBody>> {
    static MAP: OnceLock<Mutex<BTreeMap<String, CelestialBody>>> = OnceLock::new();

    MAP.get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .expect("Let's hope the lock isn't poisoned")
}

fn load_data_file(path: &Path) {
    info!("- {}...", path.display());

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

fn create_vulkan_instance(window: &Window) -> ash::Instance {
    info!("Initializing Vulkan...");

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

    return instance;
}

fn pick_physical_device(instance: &ash::Instance) -> Option<PhysicalDevice> {
    info!("Scanning for physical devices...");

    let devices = unsafe {
        instance
            .enumerate_physical_devices()
            .expect("Unable to enumerate Vulkan physical devices")
    };

    if devices.is_empty() {
        error!("Failed to find GPUs with Vulkan support!");
        return None;
    }

    let mut candidates: Vec<(PhysicalDevice, u32)> = Vec::new();

    for device in devices {
        let properties = unsafe { instance.get_physical_device_properties(device) };

        let device_name = properties.device_name_as_c_str().unwrap().to_string_lossy();
        let api_major = vk::api_version_major(properties.api_version);
        let api_minor = vk::api_version_minor(properties.api_version);
        let api_patch = vk::api_version_patch(properties.api_version);

        info!("- {}", device_name);
        info!("-- API: {}.{}.{}", api_major, api_minor, api_patch);
        info!("-- Type: {:?}", properties.device_type);

        if !is_device_suitable(instance, &device) {
            info!("-- Device is not suitable");
            continue;
        }

        let score: u32 = match properties.device_type {
            PhysicalDeviceType::DISCRETE_GPU => 2000,
            PhysicalDeviceType::INTEGRATED_GPU => 1000,
            _ => 0
        };
        info!("-- Score: {}", score);

        candidates.push((device, score));
    }

    if candidates.is_empty() {
        error!("No suitable GPUs found");
        return None;
    }

    let device_index = candidates.iter().enumerate().fold(
        (0, 0),
        |max, (index, &val)| if val.1 > max.1 { (index, val.1) } else { max }
    ).0;

    let device = candidates[device_index].0;
    let properties = unsafe { instance.get_physical_device_properties(device) };

    let device_name = properties.device_name_as_c_str().unwrap().to_string_lossy();
    info!("Using GPU: {}", device_name);

    return Some(device);
}

fn is_device_suitable(instance: &ash::Instance, device: &PhysicalDevice) -> bool {
    // Check Vulkan API version
    let properties = unsafe { instance.get_physical_device_properties(*device) };
    if properties.api_version < vk::API_VERSION_1_3 {
        return false;
    }

    // Check if any of the queue families support graphics operations
    let queue_families = unsafe { instance.get_physical_device_queue_family_properties(*device) };
    let supports_graphics = queue_families.iter().any(
        |&family| family.queue_flags.contains(QueueFlags::GRAPHICS)
    );
    if !supports_graphics {
        return false;
    }

    return true;
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

    info!("- Vulkan {}.{}.{}", major, minor, patch);
}

fn dump_vulkan_extensions(entry: &Entry) {
    let extensions = unsafe {
        entry
            .enumerate_instance_extension_properties(None)
            .expect("Unable to enumerate Vulkan extensions")
    };

    for extension in extensions {
        let name = extension.extension_name_as_c_str().unwrap();
        info!("- {}", name.to_string_lossy());
    }
}
