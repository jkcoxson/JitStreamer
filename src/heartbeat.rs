// jkcoxson

use log::{info, warn};
use rusty_libimobiledevice::{idevice::Device, services::heartbeat::HeartbeatClient};
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

        // Insert the mutex into the hashmap
        self.devices.insert(udid.clone(), mutex.clone());

        // Start the heartbeat
        for i in 0..1 {
            let mutex = mutex.clone();
            info!("Creating heartbeat from device");
            let heartbeat_client = match client.new_heartbeat_client(format!(
                "JitStreamerHeartbeat-{}-{}",
                client.get_udid(),
                i
            )) {
                Ok(heartbeat_client) => heartbeat_client,
                Err(e) => {
                    warn!("Error creating heartbeat client: {:?}", e);
                    return;
                }
            };
            tokio::task::spawn_blocking(|| {
                heartbeat_loop(heartbeat_client, mutex);
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

fn heartbeat_loop(heartbeat_client: HeartbeatClient, stopper: Arc<Mutex<bool>>) {
    info!("Heartbeat loop started");
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
            info!("We have been instructed to die");
            return;
        }
    }
}
