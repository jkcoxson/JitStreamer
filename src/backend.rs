// jkcoxson

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug)]
pub struct Backend {
    clients: Vec<Client>, // This is a Vec because we will need to iterate over it regardless of hashing method.
}

impl Backend {
    /// Loads the database JSON file into memory.
    pub fn load(path: PathBuf) -> Backend {
        let mut file = match std::fs::File::open(&path) {
            Ok(file) => file,
            Err(_) => {
                println!("Failed to open database file, using an empty database");
                return Backend { clients: vec![] };
            }
        };
        let mut contents = String::new();
        std::io::Read::read_to_string(&mut file, &mut contents).unwrap();
        let clients: Vec<Client> = serde_json::from_str(&contents).unwrap();
        Backend { clients }
    }
}

#[derive(Serialize, Deserialize, Debug)]
/// Representation of an iDevice's information.
pub struct Client {
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
