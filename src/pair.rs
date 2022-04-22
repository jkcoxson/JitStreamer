// jkcoxson

use rusty_libimobiledevice::{idevice::get_devices, services::userpref};

const VERSION: &str = "0.1.2";

fn main() {
    // Collect arguments
    let mut target = "http://jitstreamer.com".to_string();
    let mut args: Vec<String> = std::env::args().collect();

    // Correct for Windows
    for i in 0..args.len() {
        if args[i].contains("—") {
            args[i] = args[i].replace("—", "-"); // yes these are different
        }
    }

    let mut i = 0;

    while i < args.len() {
        if args[i] == "--target" || args[i] == "-t" {
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

    loop {
        // Attempt to use already generated pair file to avoid nullifying old pairing files
        let pair_record = match userpref::read_pair_record(device.get_udid()) {
            Ok(pair_record) => Some(pair_record),
            Err(e) => {
                println!("Error reading pair record: {:?}", e);
                None
            }
        };

        if let Some(mut pair_record) = pair_record {
            // Add UDID to the pair record
            pair_record
                .dict_set_item("UDID", device.get_udid().to_string().into())
                .unwrap();
            let pair_record = pair_record.to_string();
            let pair_record: Vec<u8> = pair_record.into_bytes();

            // Ask the user for the launch code
            println!("Please enter the code you got from the shortcut");
            let mut launch_code = String::new();
            std::io::stdin().read_line(&mut launch_code).unwrap();
            launch_code = launch_code.trim().to_string();

            // Yeet this bad boi off to JitStreamer
            let client = reqwest::blocking::Client::new();
            let res = client
                .post(format!("{}/potential_follow_up/{}/", target, launch_code,).as_str())
                .body(pair_record)
                .send();

            match res {
                Ok(res) => {
                    let res = res.text().unwrap();
                    let res: serde_json::Value = match serde_json::from_str(res.as_str()) {
                        Ok(res) => res,
                        Err(_) => {
                            println!("Error parsing response, pair failed");
                            continue;
                        }
                    };
                    if res["success"].as_bool().unwrap() {
                        println!("Successfully paired!");
                        wait_for_enter();
                        return;
                    } else {
                        println!("Failed to pair, attempting to regenerate the pair record");
                        println!("Error: {}", res["message"].as_str().unwrap());
                    }
                }
                Err(e) => {
                    println!("Error sending pair record: {:?}", e);
                }
            }
        }

        let lockdown_client = match device.new_lockdownd_client("jit_streamer_pair".to_string()) {
            Ok(lockdown_client) => lockdown_client,
            Err(e) => {
                println!("Error getting lockdown client: {:?}", e);
                continue;
            }
        };

        loop {
            match lockdown_client.pair(None, None) {
                Ok(()) => break,
                Err(e) => {
                    println!("Error pairing: {:?}", e);
                    println!("Make sure your device is unlocked and has a passcode");
                    wait_for_enter();
                    continue;
                }
            }
        }
    }
}

fn wait_for_enter() {
    let mut input = String::new();
    println!("Press enter to continue");
    std::io::stdin().read_line(&mut input).unwrap();
}
