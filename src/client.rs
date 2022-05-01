// jkcoxson

use std::{
    net::IpAddr,
    str::FromStr,
    sync::{Arc, Mutex},
};

use log::{info, warn};
use rusty_libimobiledevice::{idevice::Device, services::instproxy::InstProxyClient};

use plist_plus::Plist;

pub struct Client {
    pub ip: String,
    pub udid: String,
    pub pairing_file: String,
    pub dmg_path: String,
}

impl Client {
    #[allow(dead_code)]
    pub fn new(ip: String, udid: String, pairing_file: String, dmg_path: String) -> Client {
        Client {
            ip,
            udid,
            pairing_file,
            dmg_path,
        }
    }

    /// Connects to a given device and runs preflight operations.
    pub async fn connect(&self) -> Result<(Device, Arc<Mutex<bool>>), String> {
        // Determine if device is in the muxer
        let ip = match IpAddr::from_str(&self.ip) {
            Ok(ip) => ip,
            Err(e) => {
                warn!("Error parsing ip: {}", e);
                return Err("Unable to parse ip".to_string());
            }
        };
        let device = Device::new(self.udid.clone(), true, Some(ip), 0).unwrap();
        info!("Starting heartbeat {}", self.udid);

        let heartbeat = match device.new_heartbeat_client("JitStreamer".to_string()) {
            Ok(heartbeat) => heartbeat,
            Err(e) => {
                warn!("Error creating heartbeat: {:?}", e);
                return Err("Unable to create heartbeat".to_string());
            }
        };
        let stopper = Arc::new(Mutex::new(false));
        let stopper_clone = Arc::clone(&stopper);
        tokio::task::spawn_blocking(move || {
            info!("Starting heartbeat loop");
            let mut i = 0;
            loop {
                match heartbeat.receive(15000) {
                    Ok(plist) => {
                        info!("Received heartbeat: {:?}", plist);
                        match heartbeat.send(plist) {
                            Ok(_) => {}
                            Err(e) => {
                                warn!("Error sending response: {:?}", e);
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Error receiving heartbeat: {:?}", e);
                        break;
                    }
                }
                i = i + 1;
                if i > 30 {
                    info!("Heartbeat loop expired");
                    break;
                }
                if *stopper_clone.lock().unwrap() {
                    break;
                }
            }
        });

        Ok((device, stopper))
    }

    pub async fn get_apps(&self) -> Result<Plist, String> {
        let (device, stopper) = match self.connect().await {
            Ok(device) => device,
            Err(_) => {
                return Err("Unable to connect to device".to_string());
            }
        };

        let instproxy_client = match device.new_instproxy_client("jitstreamer".to_string()) {
            Ok(instproxy) => instproxy,
            Err(e) => {
                warn!("Error starting instproxy: {:?}", e);
                return Err("Unable to start instproxy".to_string());
            }
        };
        let client_opts = InstProxyClient::create_return_attributes(
            vec![("ApplicationType".to_string(), Plist::new_string("Any"))],
            vec![
                "CFBundleIdentifier".to_string(),
                "CFBundleDisplayName".to_string(),
            ],
        );
        let lookup_results = match instproxy_client.lookup(vec![], Some(client_opts)) {
            Ok(apps) => apps,
            Err(e) => {
                warn!("Error looking up apps: {:?}", e);
                return Err("Unable to lookup apps".to_string());
            }
        };

        *stopper.lock().unwrap() = true;

        Ok(lookup_results)
    }

    pub async fn debug_app(&self, app: String) -> Result<(), String> {
        let (device, stopper) = match self.connect().await {
            Ok(device) => device,
            Err(_) => {
                return Err("Unable to connect to device".to_string());
            }
        };

        let instproxy_client = match device.new_instproxy_client("idevicedebug".to_string()) {
            Ok(instproxy) => instproxy,
            Err(e) => {
                warn!("Error starting instproxy: {:?}", e);
                return Err("Unable to start instproxy".to_string());
            }
        };
        let client_opts = InstProxyClient::create_return_attributes(
            vec![("ApplicationType".to_string(), Plist::new_string("Any"))],
            vec![
                "CFBundleIdentifier".to_string(),
                "CFBundleExecutable".to_string(),
                "Container".to_string(),
            ],
        );
        let lookup_results = match instproxy_client.lookup(vec![app.clone()], Some(client_opts)) {
            Ok(apps) => apps,
            Err(e) => {
                warn!("Error looking up apps: {:?}", e);
                return Err("Unable to lookup apps".to_string());
            }
        };
        let lookup_results = lookup_results.dict_get_item(&app).unwrap();

        let working_directory = match lookup_results.dict_get_item("Container") {
            Ok(p) => p,
            Err(_) => {
                warn!("App not found");
                return Err("App not found".to_string());
            }
        };

        let working_directory = match working_directory.get_string_val() {
            Ok(p) => p,
            Err(_) => {
                warn!("App not found");
                return Err("App not found".to_string());
            }
        };
        info!("Working directory: {}", working_directory);

        let bundle_path = match instproxy_client.get_path_for_bundle_identifier(app) {
            Ok(p) => p,
            Err(e) => {
                warn!("Error getting path for bundle identifier: {:?}", e);
                return Err("Unable to get path for bundle identifier".to_string());
            }
        };

        // Attempt to create a debug server 3 times before giving up
        let mut debug_server = None;
        for _ in 1..4 {
            let ds = match device.new_debug_server("jitstreamer") {
                Ok(d) => Some(d),
                Err(_) => None,
            };
            if ds.is_some() {
                debug_server = ds;
                break;
            }
        }

        if debug_server.is_none() {
            let (device, _stopper) = match self.connect().await {
                Ok(device) => device,
                Err(_) => {
                    return Err("Unable to connect to device for disk mounting".to_string());
                }
            };
            let path = match self.get_dmg_path().await {
                Ok(p) => p,
                Err(_) => {
                    return Err(
                        "Unable to get dmg path, the server was set up incorrectly!".to_string()
                    );
                }
            };
            tokio::spawn(async move {
                match Client::upload_dev_dmg(device, path).await {
                    Ok(_) => {}
                    Err(e) => {
                        warn!("Error uploading dmg: {:?}", e);
                    }
                }
                // *stopper.lock().unwrap() = true;
            });
            return Err("JitStreamer is mounting the developer disk image, please keep your device on and connected. Check back back in a few minutes.".to_string());
        }
        let debug_server = debug_server.unwrap();

        match debug_server.send_command("QSetMaxPacketSize: 1024".into()) {
            Ok(res) => {
                info!("Successfully set max packet size: {:?}", res);
            }
            Err(e) => {
                warn!("Error setting max packet size: {:?}", e);
                return Err("Unable to set max packet size".to_string());
            }
        }

        match debug_server.send_command(format!("QSetWorkingDir: {}", working_directory).into()) {
            Ok(res) => {
                info!("Successfully set working directory: {:?}", res);
            }
            Err(e) => {
                warn!("Error setting working directory: {:?}", e);
                return Err("Unable to set working directory".to_string());
            }
        }

        match debug_server.set_argv(vec![bundle_path.clone(), bundle_path.clone()]) {
            Ok(res) => {
                info!("Successfully set argv: {:?}", res);
            }
            Err(e) => {
                warn!("Error setting argv: {:?}", e);
                return Err("Unable to set argv".to_string());
            }
        }

        match debug_server.send_command("qLaunchSuccess".into()) {
            Ok(res) => info!("Got launch response: {:?}", res),
            Err(e) => {
                warn!("Error checking if app launched: {:?}", e);
                return Err("Unable to check if app launched".to_string());
            }
        }

        match debug_server.send_command("D".into()) {
            Ok(res) => info!("Detaching: {:?}", res),
            Err(e) => {
                warn!("Error detaching: {:?}", e);
                return Err("Unable to detach".to_string());
            }
        }

        *stopper.lock().unwrap() = true;

        Ok(())
    }

    pub async fn get_ios_version(&self) -> Result<String, String> {
        let (device, _stopper) = match self.connect().await {
            Ok(device) => device,
            Err(_) => {
                return Err("Unable to connect to device".to_string());
            }
        };

        let lockdown_client = match device.new_lockdownd_client("ideviceimagemounter".to_string()) {
            Ok(lckd) => {
                info!("Successfully connected to lockdownd");
                lckd
            }
            Err(e) => {
                warn!("Error starting lockdown service: {:?}", e);
                return Err("Unable to start lockdown".to_string());
            }
        };

        let ios_version =
            match lockdown_client.get_value("ProductVersion".to_string(), "".to_string()) {
                Ok(ios_version) => ios_version.get_string_val().unwrap(),
                Err(e) => {
                    warn!("Error getting iOS version: {:?}", e);
                    return Err("Unable to get iOS version".to_string());
                }
            };

        info!("iOS version: {}", ios_version);

        //*stopper.lock().unwrap() = true;

        Ok(ios_version)
    }

    pub async fn get_dmg_path(&self) -> Result<String, String> {
        let ios_version = self.get_ios_version().await?;

        // Check if directory exists
        let path = std::path::Path::new(&self.dmg_path).join(format!("{}.dmg", &ios_version));
        info!("Checking if {} exists", path.display());
        if path.exists() {
            return Ok(String::from(path.to_string_lossy()));
        }

        let mut ios_dmg_url = None;
        let dmg_libraries = [
            "https://raw.githubusercontent.com/jkcoxson/JitStreamer/master/versions.json",
            "https://cdn.altstore.io/file/altstore/altserver/developerdisks.json",
        ];
        for lib in dmg_libraries {
            if ios_dmg_url != None {
                break;
            }
            // Download versions.json from GitHub
            info!("Downloading iOS dictionary...");
            let response = match reqwest::get(lib).await {
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
            ios_dmg_url = match versions.get(ios_version.clone()) {
                Some(x) => Some(x.as_str().unwrap().to_string()),
                None => None,
            };
        }

        if ios_dmg_url == None {
            return Err("Libraries did not contain iOS DMG".to_string());
        }

        // Download DMG zip
        info!("Downloading iOS {} DMG...", ios_version.clone());
        let resp = match reqwest::get(ios_dmg_url.unwrap()).await {
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
        let tmp_path = format!("{}/tmp", &self.dmg_path);
        info!("tmp path {}", tmp_path);
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
                if entry.path().to_str().unwrap().contains("__MACOSX") {
                    continue;
                }
                dmg_path = entry.path();
            }
        }
        // Move DMG to JIT Shipper directory
        let ios_dmg = dmg_path.join("DeveloperDiskImage.dmg");
        std::fs::rename(ios_dmg, format!("{}/{}.dmg", &self.dmg_path, ios_version)).unwrap();
        let ios_sig = dmg_path.join("DeveloperDiskImage.dmg.signature");
        std::fs::rename(
            ios_sig,
            format!("{}/{}.dmg.signature", &self.dmg_path, ios_version),
        )
        .unwrap();

        // Remove tmp path
        std::fs::remove_dir_all(tmp_path).unwrap();
        println!(
            "Successfully downloaded and extracted iOS {} developer disk image",
            ios_version
        );

        // Return DMG path
        Ok(format!("{}/{}.dmg", &self.dmg_path, ios_version))
    }

    pub async fn upload_dev_dmg(device: Device, dmg_path: String) -> Result<(), String> {
        let mut lockdown_client =
            match device.new_lockdownd_client("ideviceimagemounter".to_string()) {
                Ok(lckd) => {
                    info!("Successfully connected to lockdownd");
                    lckd
                }
                Err(e) => {
                    warn!("Error starting lockdown service: {:?}", e);
                    return Err("Unable to start lockdown".to_string());
                }
            };

        let service = match lockdown_client
            .start_service("com.apple.mobile.mobile_image_mounter".to_string(), false)
        {
            Ok(service) => {
                info!("Successfully started com.apple.mobile.mobile_image_mounter");
                service
            }
            Err(e) => {
                warn!(
                    "Error starting com.apple.mobile.mobile_image_mounter: {:?}",
                    e
                );
                return Err("Unable to start com.apple.mobile.mobile_image_mounter".to_string());
            }
        };

        let mim = match device.new_mobile_image_mounter(&service) {
            Ok(mim) => {
                info!("Successfully started mobile_image_mounter");
                mim
            }
            Err(e) => {
                warn!("Error starting mobile_image_mounter: {:?}", e);
                return Err("Unable to start mobile_image_mounter".to_string());
            }
        };

        info!("Uploading DMG from: {}", dmg_path);
        info!(
            "signature: {}",
            format!("{}.signature", dmg_path.clone()).to_string()
        );
        match mim.upload_image(
            dmg_path.clone(),
            "Developer".to_string(),
            format!("{}.signature", dmg_path.clone()).to_string(),
        ) {
            Ok(_) => {
                info!("Successfully uploaded image");
            }
            Err(e) => {
                warn!("Error uploading image: {:?}", e);
                return Err("Unable to upload developer disk image".to_string());
            }
        }
        match mim.mount_image(
            dmg_path.clone(),
            "Developer".to_string(),
            format!("{}.signature", dmg_path.clone()).to_string(),
        ) {
            Ok(_) => {
                info!("Successfully mounted image");
            }
            Err(e) => {
                warn!("Error mounting image: {:?}", e);
                return Err("Unable to mount developer disk image".to_string());
            }
        }
        Ok(())
    }
}
