// jkcoxson

const DEFAULT: &str = r#"
# JitStreamer Config File
# Revision: A

[paths]
# The path to host static content when a route is not matched
# Useful for hosting sites along with JitStreamer
static_path = "static"

# The path to the database JSON file
database_path = "database.json"

# The path to the plist storage directory. This is different depending on the OS.
# On Linux, this is /var/lib/lockdown. On macOS, this is /var/db/lockdown. 
# On Windows, this is <username>\AppData\roaming\Apple Computer\Lockdown.
plist_storage = "plist_storage"

# The path on where to store downloaded DMG files
# These files are used for mounting the iOS device, and are different depending on the version of iOS.
dmg_path = "dmg_files"

[web_server]
# The port to run JitStreamer on
port = 8080

# The port to run JitStreamer on with SSL (uncomment to use)
# ssl_port = 443

# The host to bind JitStreamer to
host = "0.0.0.0"

# The path to the SSL certificate to use. Must be in place if using SSL port.
# ssl_cert = "cert.pem"

# The path to the SSL key to use. Must be in place if using SSL port.
# ssl_key = "key.pem"

[extra]
# The IPs that are allowed to use JitStreamer.
# This restricts the IPs that are allowed to use the JIT functionality
# while allowing access to the site
allowed_subnet = "0.0.0.0/0"

# The address that can be used to access netmuxd (uncomment to use)
# This is a temporary option for use in SideStore
# netmuxd_address = "127.0.0.1:27015"
                        
"#;

use std::{fs::File, io::Write};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub paths: Paths,
    pub web_server: WebServer,
    pub extra: Extra,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Paths {
    pub static_path: String,
    pub database_path: String,
    pub plist_storage: String,
    pub dmg_path: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WebServer {
    pub port: u16,
    pub ssl_port: Option<u16>,
    pub host: String,
    pub ssl_cert: Option<String>,
    pub ssl_key: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Extra {
    pub allowed_subnet: String,
    pub netmuxd_address: Option<String>,
}

impl Config {
    pub fn load() -> Config {
        let config_path = "config.toml";
        match std::fs::read_to_string(config_path) {
            Ok(contents) => {
                let config: Config = match toml::from_str(&contents) {
                    Ok(c) => c,
                    Err(e) => panic!("Error parsing config: {}", e),
                };
                config
            }
            Err(e) => {
                println!("Could not read config file: {}", e);
                match e.kind() {
                    std::io::ErrorKind::NotFound => {
                        println!("Creating default config file");
                        let default = DEFAULT.to_string();
                        let mut file = File::create(config_path).unwrap();
                        file.write_all(default.as_bytes()).unwrap();
                        Config::load()
                    }
                    _ => panic!("This is a fatal error."),
                }
            }
        }
    }
}
