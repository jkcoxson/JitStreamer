// jkcoxson

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::Config;

#[derive(Serialize, Deserialize, Debug)]
pub struct Backend {
    clients: Vec<Client>, // This is a Vec because we will need to iterate over it regardless of hashing method.
    pub allowed_ip: String,
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
                    clients: vec![],
                    allowed_ip: config.allowed_ip.clone(),
                    database_path: config.database_path.clone(),
                    plist_storage: config.plist_storage.clone(),
                    dmg_path: config.dmg_path.clone(),
                };
            }
        };
        let mut contents = String::new();
        std::io::Read::read_to_string(&mut file, &mut contents).unwrap();
        let clients: Vec<Client> = serde_json::from_str(&contents).unwrap();
        Backend {
            clients,
            allowed_ip: config.allowed_ip.clone(),
            database_path: config.database_path.clone(),
            plist_storage: config.plist_storage.clone(),
            dmg_path: config.dmg_path.clone(),
        }
    }

    /// Saves the database to disk.
    fn save(&self) {
        let contents = serde_json::to_string_pretty(&self.clients).unwrap();
        let mut file = std::fs::File::create(&self.database_path).unwrap();
        std::io::Write::write_all(&mut file, contents.as_bytes()).unwrap();
    }

    pub fn get_by_ip(&mut self, ip: &str) -> Option<&mut Client> {
        for client in &mut self.clients {
            if client.ip == ip {
                return Some(client);
            }
        }
        None
    }

    pub fn get_by_udid(&self, udid: &str) -> Option<&Client> {
        self.clients.iter().find(|c| c.udid == udid)
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
        self.clients.push(Client {
            ip,
            udid,
            apps: vec![],
            last_seen: since_the_epoch.as_secs(),
        });
        self.save();
        Ok(())
    }

    pub fn write_pairing_file(&self, plist: String, udid: &String) -> Result<(), ()> {
        let path = format!("{}/{}.plist", &self.plist_storage, &udid);
        let mut file = std::fs::File::create(&path).unwrap();
        match std::io::Write::write_all(&mut file, plist.as_bytes()) {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }
    }

    pub fn remove_pairing_file(&self, udid: &String) -> Result<(), ()> {
        let path = format!("{}/{}.plist", &self.plist_storage, &udid);
        match std::fs::remove_file(&path) {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }
    }

    pub async fn get_ios_dmg(base_path: &str, version: &str) -> Result<String, String> {
        println!("Finding iOS {}", version);
        // Check if directory exists
        let path = format!("{}/{}.dmg", &base_path, version);
        if std::path::Path::new(&path).exists() {
            return Ok(path);
        }
        // Download versions.json from GitHub
        println!("Downloading iOS dictionary...");
        let url = "https://raw.githubusercontent.com/jkcoxson/jit_shipper/master/versions.json";
        let response = match reqwest::get(url).await {
            Ok(response) => response,
            Err(_) => {
                return Err("Error downloading versions.json".to_string());
            }
        };
        let contents = match response.text().await {
            Ok(contents) => contents,
            Err(_) => {
                return Err("Error reading versions.json".to_string());
            }
        };
        // Parse versions.json
        let versions: serde_json::Value = serde_json::from_str(&contents).unwrap();
        // Get DMG url
        let ios_dmg_url = match versions.get(version.clone()) {
            Some(x) => x.as_str().unwrap().to_string(),
            None => return Err("DMG library does not contain your iOS version".to_string()),
        };
        // Download DMG zip
        println!("Downloading iOS {} DMG...", version.clone());
        let resp = match reqwest::get(ios_dmg_url).await {
            Ok(resp) => resp,
            Err(_) => {
                return Err("Error downloading DMG".to_string());
            }
        };
        let mut out = match std::fs::File::create("dmg.zip") {
            Ok(out) => out,
            Err(_) => {
                return Err("Error creating temp DMG.zip".to_string());
            }
        };
        let mut content = std::io::Cursor::new(match resp.bytes().await {
            Ok(content) => content,
            Err(_) => {
                return Err("Error reading DMG".to_string());
            }
        });
        match std::io::copy(&mut content, &mut out) {
            Ok(_) => (),
            Err(_) => {
                return Err("Error downloading DMG".to_string());
            }
        };
        // Create tmp path
        let tmp_path = format!("{}/tmp", &base_path);
        std::fs::create_dir_all(&tmp_path).unwrap();
        // Unzip zip
        let mut dmg_zip = match zip::ZipArchive::new(std::fs::File::open("dmg.zip").unwrap()) {
            Ok(dmg_zip) => dmg_zip,
            Err(_) => {
                return Err("Error opening DMG.zip".to_string());
            }
        };
        match dmg_zip.extract(&tmp_path) {
            Ok(_) => {}
            Err(e) => return Err(format!("Failed to unzip DMG: {:?}", e)),
        }
        // Remove zip
        match std::fs::remove_file("dmg.zip") {
            Ok(_) => (),
            Err(_) => return Err("Failed to remove DMG.zip".to_string()),
        }
        // Get folder name in tmp
        let mut dmg_path = std::path::PathBuf::new();
        for entry in std::fs::read_dir(&tmp_path).unwrap() {
            let entry = entry.unwrap();
            if entry.path().is_dir() {
                dmg_path = entry.path();
            }
        }
        // Move DMG to JIT Shipper directory
        let ios_dmg = dmg_path.join("DeveloperDiskImage.dmg");
        std::fs::rename(ios_dmg, format!("{}/{}.dmg", &base_path, version)).unwrap();
        let ios_sig = dmg_path.join("DeveloperDiskImage.dmg.signature");
        std::fs::rename(ios_sig, format!("{}/{}.dmg.signature", &base_path, version)).unwrap();

        // Remove tmp path
        std::fs::remove_dir_all(tmp_path).unwrap();
        println!(
            "Successfully downloaded and extracted iOS {} developer disk image",
            version
        );

        // Return DMG path
        Ok(format!("{}/{}.dmg", &base_path, version))
    }
}

#[derive(Serialize, Deserialize, Debug)]
/// Representation of an iDevice's information.
pub struct DeserialiedClient {
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
