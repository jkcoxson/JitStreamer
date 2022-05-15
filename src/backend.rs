// jkcoxson

use log::warn;
use rand::Rng;
use rusty_libimobiledevice::idevice::Device;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::client::Client;
use crate::config::Config;
use crate::heartbeat::Heart;

#[derive(Serialize, Deserialize)]
pub struct Backend {
    pub deserialized_clients: Vec<DeserializedClient>,
    pub allowed_ips: Vec<String>,
    database_path: String,
    plist_storage: String,
    pub dmg_path: String,

    #[serde(skip)]
    pub pair_potential: Vec<PairPotential>,

    #[serde(skip)]
    pub heart: Arc<Mutex<Heart>>,

    #[serde(skip)]
    pub counter: Counter,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Counter {
    pub launched: usize,
    pub fetched: usize,
    pub attached: usize,
    pub uptime: Duration,
}

#[derive(Debug)]
pub struct PairPotential {
    pub ip: String,
    pub code: u16,
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
                    pair_potential: vec![],
                    heart: Arc::new(Mutex::new(Heart::new())),
                    counter: Counter {
                        launched: 0,
                        fetched: 0,
                        attached: 0,
                        uptime: SystemTime::now().duration_since(UNIX_EPOCH).unwrap(),
                    },
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
            pair_potential: vec![],
            heart: Arc::new(Mutex::new(Heart::new())),
            counter: Counter {
                launched: 0,
                fetched: 0,
                attached: 0,
                uptime: SystemTime::now().duration_since(UNIX_EPOCH).unwrap(),
            },
        }
    }

    /// Saves the database to disk.
    fn save(&self) {
        let contents = serde_json::to_string_pretty(&self.deserialized_clients).unwrap();
        let mut file = std::fs::File::create(&self.database_path).unwrap();
        std::io::Write::write_all(&mut file, contents.as_bytes()).unwrap();
    }

    pub fn check_ip(&self, ip: &str) -> bool {
        if self.allowed_ips.len() == 0 {
            return true;
        }
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

    pub fn unregister_client(&mut self, ip: String) -> Result<(), ()> {
        if let Some(client) = self.get_by_ip(&ip) {
            // Delete pairing file
            match std::fs::remove_file(format!("/var/lib/lockdown/{}.plist", client.udid)) {
                _ => {}
            }

            // Remove from database
            let mut i = 0;
            while i < self.deserialized_clients.len() {
                if &self.deserialized_clients[i].ip == &ip {
                    self.deserialized_clients.remove(i);
                }
                i = i + 1;
            }
            self.save();
            return Ok(());
        } else {
            return Err(());
        }
    }

    pub fn get_by_ip(&mut self, ip: &str) -> Option<Client> {
        let res = self
            .deserialized_clients
            .iter()
            .find(|client| client.ip == ip);
        match res {
            Some(c) => Some(c.to_client(
                &format!("{}/{}.plist", self.plist_storage, c.udid),
                &self.dmg_path,
                self.heart.clone(),
            )),
            None => None,
        }
    }

    pub fn _get_by_udid(&self, udid: &str) -> Option<Client> {
        let res = self.deserialized_clients.iter().find(|c| c.udid == udid);
        match res {
            Some(c) => Some(c.to_client(
                &format!("{}/{}.plist", self.plist_storage, c.udid),
                &self.dmg_path,
                self.heart.clone(),
            )),
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
        // Determine if device is in the muxer
        let ip = match IpAddr::from_str(ip) {
            Ok(ip) => ip,
            Err(e) => {
                warn!("Error parsing ip {}: {}", ip, e);
                return Err(());
            }
        };
        let to_test = Device::new(udid.to_string(), true, Some(ip), 0).unwrap();
        // Start lockdownd
        let _ = match to_test.new_lockdownd_client("test".to_string()) {
            Ok(_) => return Ok(()),
            Err(e) => {
                warn!("Error creating lockdownd client: {:?}", e);
                return Err(());
            }
        };
    }

    pub fn prefered_app(name: &str) -> bool {
        let app_list = include_str!("known_apps.txt").to_string();
        let apps: Vec<&str> = app_list.split("\n").collect();
        for app in apps {
            if name.contains(app) {
                return true;
            }
        }
        false
    }

    pub fn potential_pair(&mut self, ip: String) -> u16 {
        let mut rng = rand::thread_rng();
        let code: u16 = rng.gen_range(10000..65535);

        let p = PairPotential { ip, code };
        self.pair_potential.push(p);
        code
    }

    pub fn check_code(&mut self, code: u16) -> Option<String> {
        let mut i = 0;
        while i < self.pair_potential.len() {
            if self.pair_potential[i].code == code {
                let ip = self.pair_potential[i].ip.clone();
                return Some(ip);
            }
            i = i + 1;
        }
        None
    }

    pub fn remove_code(&mut self, code: u16) {
        let mut i = 0;
        while i < self.pair_potential.len() {
            if self.pair_potential[i].code == code {
                self.pair_potential.remove(i);
            }
            i = i + 1;
        }
    }
}

impl Default for Counter {
    fn default() -> Self {
        Counter {
            fetched: 0,
            launched: 0,
            attached: 0,
            uptime: SystemTime::now().duration_since(UNIX_EPOCH).unwrap(),
        }
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

impl DeserializedClient {
    pub fn to_client(
        &self,
        plist_path: &String,
        dmg_path: &String,
        heart: Arc<Mutex<Heart>>,
    ) -> Client {
        Client {
            ip: self.ip.clone(),
            udid: self.udid.clone(),
            pairing_file: plist_path.to_string(),
            dmg_path: dmg_path.to_string(),
            heart,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct App {
    pub name: String,
    pub bundle_id: String,
}
