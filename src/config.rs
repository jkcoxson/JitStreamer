// jkcoxson

use std::{fs::File, io::BufReader};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub port: u16,
    pub host: String,
    pub static_path: String,
    pub database_path: String,
    pub plist_storage: String,
    pub dmg_path: String,
    pub allowed_ip: String,
}

impl Config {
    pub fn load() -> Config {
        let json_path = "config.json";
        match File::open(json_path) {
            Ok(file) => {
                let reader = BufReader::new(file);
                let config: Config = serde_json::from_reader(reader).unwrap();
                config
            }
            Err(_) => {
                println!("Failed to load config.json, using default config");
                Config::default()
            }
        }
    }
    fn default() -> Config {
        Config {
            port: 443,
            host: "0.0.0.0".to_string(),
            static_path: "./dist".to_string(),
            database_path: "./database.json".to_string(),
            plist_storage: "/var/lib/lockdown/".to_string(),
            dmg_path: "/DeveloperDiskImages/".to_string(),
            allowed_ip: "127.0".to_string(),
        }
    }
}
