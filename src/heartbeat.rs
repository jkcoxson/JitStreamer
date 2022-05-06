// jkcoxson

use log::{info, warn};
use rusty_libimobiledevice::{idevice::Device, services::heartbeat::HeartbeatClient};
use std::collections::HashMap;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub struct Heart {
    devices: HashMap<String, UnboundedSender<()>>,
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
        info!("Heart lock received");
        let client = client.clone();
        // Check to see if the device already has a heartbeat channel
        if self.devices.contains_key(&client.get_udid()) {
            info!(
                "Device {} already has a heartbeat channel",
                client.get_udid()
            );
            return;
        }

        // Create a new heartbeat channel
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.devices.insert(client.get_udid(), tx);

        let heartbeat_client = match client.new_heartbeat_client("JitStreamer".to_string()) {
            Ok(heartbeat) => {
                info!("Heartbeat client created successfully");
                heartbeat
            }
            Err(e) => {
                warn!("Error creating heartbeat: {:?}", e);
                return;
            }
        };

        // Start the heartbeat
        tokio::spawn(async move {
            heartbeat_loop(heartbeat_client, rx).await;
        });
    }
    pub fn kill(&mut self, udid: impl Into<String>) {
        let udid = udid.into();
        info!("Attempting to kill heartbeat for {}", udid);
        if self.devices.contains_key(&udid) {
            let sender = self.devices.remove(&udid).unwrap();
            sender.send(()).unwrap();
        }
    }
}

async fn heartbeat_loop(heartbeat_client: HeartbeatClient, mut rx: UnboundedReceiver<()>) {
    loop {
        tokio::select! {
            _ = rx.recv() => {
                info!("Heartbeat instructed to die");
                return;
            }
            res = heartbeat_client.receive_async(10000) => {
                match res {
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
            }
        }
    }
}
