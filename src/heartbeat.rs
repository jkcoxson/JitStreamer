// jkcoxson

use log::{info, warn};
use rusty_libimobiledevice::idevice::Device;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

pub struct Heart {
    devices: HashMap<String, Arc<Mutex<bool>>>,
}

impl Default for Heart {
    fn default() -> Self {
        Heart {
            devices: HashMap::new(),
        }
    }
}

impl Heart {
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
        }
    }
    pub fn start(&mut self, client: &Device) {
        // Check to see if the device already has a heartbeat channel
        if self.devices.contains_key(&client.get_udid()) {
            info!(
                "Device {} already has a heartbeat channel",
                client.get_udid()
            );
            return;
        }
        // Create a new heartbeat mutex
        let mutex = Arc::new(Mutex::new(false));

        let udid = client.get_udid();
        let ip_addr = match client.get_ip_address() {
            Some(ip) => ip,
            None => {
                warn!("Device {} has no IP address", udid);
                return;
            }
        };

        // Insert the mutex into the hashmap
        self.devices.insert(udid.clone(), mutex.clone());

        // Start the heartbeat
        for _ in 0..4 {
            let mutex = mutex.clone();
            let udid = udid.clone();
            let ip_addr = ip_addr.clone();
            tokio::task::spawn_blocking(|| {
                heartbeat_loop(udid, ip_addr, mutex);
            });
        }
    }
    pub fn kill(&mut self, udid: impl Into<String>) {
        let udid = udid.into();
        info!("Attempting to kill heartbeat for {}", udid);
        if self.devices.contains_key(&udid) {
            let stopper = self.devices.remove(&udid).unwrap();
            // Set stopper to true so the heartbeat loop will exit
            let mut stopper = stopper.lock().unwrap();
            *stopper = true;
        }
    }
}

fn heartbeat_loop(udid: String, ip_addr: String, stopper: Arc<Mutex<bool>>) {
    loop {
        let device = match Device::new(
            (&udid).to_string(),
            true,
            Some(ip_addr.parse().unwrap()),
            69,
        ) {
            Ok(device) => device,
            Err(e) => {
                warn!("Error connecting to device {}: {:?}", udid, e);
                return;
            }
        };
        let heartbeat_client = match device.new_heartbeat_client("JitStreamer".to_string()) {
            Ok(heartbeat) => heartbeat,
            Err(e) => {
                warn!("Error creating heartbeat for {}: {:?}", udid, e);
                return;
            }
        };

        loop {
            match heartbeat_client.receive(15000) {
                Ok(plist) => {
                    info!("Received heartbeat");
                    match heartbeat_client.send(plist) {
                        Ok(_) => {}
                        Err(e) => {
                            warn!("Error sending response: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Error receiving heartbeat: {:?}", e);
                    break;
                }
            }
            if stopper.lock().unwrap().clone() {
                info!("We have been instructed to die: {}", udid);
                return;
            }
        }
    }
}
