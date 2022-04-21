// jkcoxson

use rusty_libimobiledevice::{idevice::get_devices, services::userpref};

const VERSION: &str = "0.1.2";

fn main() {
    // Collect arguments
    let mut target = "".to_string();
    let args: Vec<String> = std::env::args().collect();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--target" {
            target = args[i + 1].clone();
        }
        if args[i] == "-h" || args[i] == "--help" {
            println!("Usage: {} [--target <IP>]", args[0]);
            return;
        }
        if args[i] == "-a" || args[i] == "--about" {
            println!("Pair program for JitStreamer");
            println!("Written by Jackson Coxson");
        }
        if args[i] == "-v" || args[i] == "--version" {
            println!("Pair version {}", VERSION);
        }
        i = i + 1;
    }

    // Wait until a device is connected by USB
    let mut device = None;
    loop {
        let devices = match get_devices() {
            Ok(devices) => devices,
            Err(e) => {
                println!("Error getting device list: {:?}", e);
                println!("You need to install iTunes or start usbmuxd to get the device list");
                return;
            }
        };
        if devices.len() == 0 {
            println!("Please connect your device via USB and try again.");
            println!("If your device is connected, check the cable and make sure iTunes is running if on Windows");
            wait_for_enter();
            continue;
        }
        for dev in devices {
            if !dev.get_network() {
                device = Some(dev);
                break;
            }
        }
        if device.is_some() {
            break;
        }
        println!("Please connect your device via USB and try again.");
        println!("If your device is connected, check the cable and make sure iTunes is running if on Windows");
        wait_for_enter();
    }
    let device = device.unwrap();

    // Attempt to use already generated pair file to avoid nullifying old pairing files
    let pair_record = match userpref::read_pair_record(device.get_udid()) {
        Ok(pair_record) => Some(pair_record),
        Err(e) => {
            println!("Error reading pair record: {:?}", e);
            None
        }
    };

    if let Some(pair_record) = pair_record {
        println!("{:?}", pair_record);
    }
}

fn wait_for_enter() {
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
}
