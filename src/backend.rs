// jkcoxson

const SERVICE_NAME: &str = "12:34:56:78:90:AB@fe80::de52:85ff:fece:c422._apple-mobdev2._tcp";

use rusty_libimobiledevice::debug;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::client::Client;
use crate::config::Config;

#[derive(Serialize, Deserialize, Debug)]
pub struct Backend {
    deserialized_clients: Vec<DeserializedClient>,
    pub allowed_ips: Vec<String>,
    database_path: String,
    plist_storage: String,
    pub dmg_path: String,
}

impl Backend {
    /// Loads the database JSON file into memory.
    pub fn load(config: &Config) -> Backend {
        let mut file = match std::fs::File::open(config.database_path.clone()) {
            Ok(file) => file,
            Err(_) => {
                println!("Failed to open database file, using an empty database");
                return Backend {
                    deserialized_clients: vec![],
                    allowed_ips: config.allowed_ips.clone(),
                    database_path: config.database_path.clone(),
                    plist_storage: config.plist_storage.clone(),
                    dmg_path: config.dmg_path.clone(),
                };
            }
        };
        let mut contents = String::new();
        std::io::Read::read_to_string(&mut file, &mut contents).unwrap();
        let clients: Vec<DeserializedClient> = serde_json::from_str(&contents).unwrap();
        Backend {
            deserialized_clients: clients,
            allowed_ips: config.allowed_ips.clone(),
            database_path: config.database_path.clone(),
            plist_storage: config.plist_storage.clone(),
            dmg_path: config.dmg_path.clone(),
        }
    }

    /// Saves the database to disk.
    fn save(&self) {
        let contents = serde_json::to_string_pretty(&self.deserialized_clients).unwrap();
        let mut file = std::fs::File::create(&self.database_path).unwrap();
        std::io::Write::write_all(&mut file, contents.as_bytes()).unwrap();
    }

    pub fn check_ip(&self, ip: &str) -> bool {
        for allowed_ip in &self.allowed_ips {
            if ip.starts_with(allowed_ip) {
                return true;
            }
        }
        false
    }

    pub fn register_client(&mut self, ip: String, udid: String) -> Result<(), ()> {
        // Check if the client is already registered.
        if self.get_by_ip(&ip).is_some() {
            return Err(());
        }
        if self.get_by_udid(&udid).is_some() {
            return Err(());
        }
        let start = SystemTime::now();
        let since_the_epoch = start
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
        // Add the client to the database.
        self.deserialized_clients.push(DeserializedClient {
            ip,
            udid,
            apps: vec![],
            last_seen: since_the_epoch.as_secs(),
        });
        self.save();
        Ok(())
    }

    pub fn get_by_ip(&mut self, ip: &str) -> Option<Client> {
        let res = self
            .deserialized_clients
            .iter()
            .find(|client| client.ip == ip);
        match res {
            Some(c) => Some(c.into()),
            None => None,
        }
    }

    pub fn get_by_udid(&self, udid: &str) -> Option<Client> {
        let res = self.deserialized_clients.iter().find(|c| c.udid == udid);
        match res {
            Some(c) => Some(c.into()),
            None => None,
        }
    }

    pub fn write_pairing_file(&self, plist: String, udid: &String) -> Result<(), ()> {
        let path = format!("{}/{}.plist", &self.plist_storage, &udid);
        let mut file = std::fs::File::create(&path).unwrap();
        match std::io::Write::write_all(&mut file, plist.as_bytes()) {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }
    }

    pub fn _remove_pairing_file(&self, udid: &String) -> Result<(), ()> {
        let path = format!("{}/{}.plist", &self.plist_storage, &udid);
        match std::fs::remove_file(&path) {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }
    }

    pub async fn test_new_client(ip: &String, udid: &String) -> Result<(), ()> {
        let udids = match rusty_libimobiledevice::libimobiledevice::get_udid_list() {
            Ok(udids) => udids,
            Err(_) => {
                debug!("Error getting udid list");
                return Err(());
            }
        };
        if udids.contains(udid) {
            return Ok(());
        }
        // Register with usbmuxd
        let mut stream = match tokio::net::TcpStream::connect("127.0.0.1:32498").await {
            Ok(stream) => stream,
            Err(_) => {
                return Err(());
            }
        };
        // Send the register packet
        match tokio::io::AsyncWriteExt::write_all(
            &mut stream,
            format!("1\n{}\n{}\n{}\n", udid, SERVICE_NAME, ip).as_bytes(),
        )
        .await
        {
            _ => (),
        };
        for _ in 1..20 {
            // Wait for a few seconds for it to register
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            let udids = match rusty_libimobiledevice::libimobiledevice::get_udid_list() {
                Ok(udids) => udids,
                Err(_) => return Err(()),
            };
            if udids.contains(udid) {
                return Ok(());
            }
        }
        Err(())
    }
}

#[derive(Serialize, Deserialize, Debug)]
/// Representation of an iDevice's information.
pub struct DeserializedClient {
    /// The iDevice's IP on the VLAN.
    pub ip: String,
    /// The iDevice's UDID used to identify it.
    pub udid: String,
    /// Will be used to automatically resign apps maybe someday.
    pub apps: Vec<App>,
    /// If the device hasn't been seen in 28 days, it will be removed.
    pub last_seen: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct App {
    pub name: String,
    pub bundle_id: String,
}
