// jkcoxson

use std::{net::IpAddr, str::FromStr};

use rusty_libimobiledevice::{debug, idevice::Device, services::instproxy::InstProxyClient};

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
    pub async fn connect(&self) -> Result<Device, String> {
        // Determine if device is in the muxer
        let ip = match IpAddr::from_str(&self.ip) {
            Ok(ip) => ip,
            Err(e) => {
                debug!("Error parsing ip: {}", e);
                return Err("Unable to parse ip".to_string());
            }
        };
        let device = Device::new(self.udid.clone(), true, Some(ip), 0).unwrap();
        debug!("Starting heartbeat {}", self.udid);

        let heartbeat = match device.new_heartbeat_client("JitStreamer".to_string()) {
            Ok(heartbeat) => heartbeat,
            Err(e) => {
                debug!("Error creating heartbeat: {:?}", e);
                return Err("Unable to create heartbeat".to_string());
            }
        };
        tokio::task::spawn_blocking(move || {
            debug!("Starting heartbeat loop");
            loop {
                match heartbeat.receive_with_timeout(15000) {
                    Ok(plist) => {
                        debug!("Received heartbeat: {:?}", plist);
                        // let mut response = Plist::new_dict();
                        // match response.dict_set_item("Command", "Polo".into()) {
                        //     Ok(_) => {}
                        //     Err(e) => {
                        //         debug!("Error setting response: {:?}", e);
                        //         return;
                        //     }
                        // }
                        match heartbeat.send(plist) {
                            Ok(_) => {}
                            Err(e) => {
                                debug!("Error sending response: {:?}", e);
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        debug!("Error receiving heartbeat: {:?}", e);
                        break;
                    }
                }
            }
        });

        Ok(device)
    }

    pub async fn get_apps(&self) -> Result<Vec<String>, String> {
        let device = match self.connect().await {
            Ok(device) => device,
            Err(_) => {
                return Err("Unable to connect to device".to_string());
            }
        };

        let instproxy_client = match device.new_instproxy_client("jitstreamer".to_string()) {
            Ok(instproxy) => instproxy,
            Err(e) => {
                debug!("Error starting instproxy: {:?}", e);
                return Err("Unable to start instproxy".to_string());
            }
        };
        let mut client_opts = InstProxyClient::options_new();
        InstProxyClient::options_add(
            &mut client_opts,
            vec![("ApplicationType".to_string(), Plist::new_string("Any"))],
        );
        InstProxyClient::options_set_return_attributes(
            &mut client_opts,
            vec![
                "CFBundleIdentifier".to_string(),
                "CFBundleExecutable".to_string(),
                "Container".to_string(),
            ],
        );
        let lookup_results = match instproxy_client.lookup(vec![], client_opts) {
            Ok(apps) => apps,
            Err(e) => {
                debug!("Error looking up apps: {:?}", e);
                return Err("Unable to lookup apps".to_string());
            }
        };

        let p_iter = lookup_results.into_iter();
        let mut apps = Vec::new();

        for i in p_iter {
            apps.push(i.key.unwrap());
        }

        Ok(apps)
    }

    pub async fn debug_app(&self, app: String) -> Result<(), String> {
        let device = match self.connect().await {
            Ok(device) => device,
            Err(_) => {
                return Err("Unable to connect to device".to_string());
            }
        };

        let instproxy_client = match device.new_instproxy_client("idevicedebug".to_string()) {
            Ok(instproxy) => instproxy,
            Err(e) => {
                debug!("Error starting instproxy: {:?}", e);
                return Err("Unable to start instproxy".to_string());
            }
        };

        let mut client_opts = InstProxyClient::options_new();
        InstProxyClient::options_add(
            &mut client_opts,
            vec![("ApplicationType".to_string(), Plist::new_string("Any"))],
        );
        InstProxyClient::options_set_return_attributes(
            &mut client_opts,
            vec![
                "CFBundleIdentifier".to_string(),
                "CFBundleExecutable".to_string(),
                "Container".to_string(),
            ],
        );
        let lookup_results = match instproxy_client.lookup(vec![app.clone()], client_opts) {
            Ok(apps) => apps,
            Err(e) => {
                debug!("Error looking up apps: {:?}", e);
                return Err("Unable to lookup apps".to_string());
            }
        };
        let lookup_results = lookup_results.dict_get_item(&app).unwrap();

        let working_directory = match lookup_results.dict_get_item("Container") {
            Ok(p) => p,
            Err(_) => {
                debug!("App not found");
                return Err("App not found".to_string());
            }
        };

        let working_directory = match working_directory.get_string_val() {
            Ok(p) => p,
            Err(_) => {
                debug!("App not found");
                return Err("App not found".to_string());
            }
        };
        debug!("Working directory: {}", working_directory);

        let bundle_path = match instproxy_client.get_path_for_bundle_identifier(app) {
            Ok(p) => p,
            Err(e) => {
                debug!("Error getting path for bundle identifier: {:?}", e);
                return Err("Unable to get path for bundle identifier".to_string());
            }
        };

        // Attempt to create a debug server 3 times before giving up
        let mut debug_server = None;
        for _ in 1..4 {
            let ds = match device.new_debug_server("jitstreamer") {
                Ok(d) => Some(d),
                Err(_) => {
                    match self.upload_dev_dmg().await {
                        Ok(_) => {
                            debug!("Successfully uploaded dev.dmg");
                        }
                        Err(e) => {
                            debug!("Error uploading dev.dmg: {:?}", e);
                            return Err(format!("Unable to upload dev.dmg: {:?}", e));
                        }
                    };
                    None
                }
            };
            if ds.is_some() {
                debug_server = ds;
                break;
            }
        }

        if debug_server.is_none() {
            return Err("Unable to start debug server".to_string());
        }
        let debug_server = debug_server.unwrap();

        match debug_server.send_command("QSetMaxPacketSize: 1024".into()) {
            Ok(res) => {
                debug!("Successfully set max packet size: {:?}", res);
            }
            Err(e) => {
                debug!("Error setting max packet size: {:?}", e);
                return Err("Unable to set max packet size".to_string());
            }
        }

        match debug_server.send_command(format!("QSetWorkingDir: {}", working_directory).into()) {
            Ok(res) => {
                debug!("Successfully set working directory: {:?}", res);
            }
            Err(e) => {
                debug!("Error setting working directory: {:?}", e);
                return Err("Unable to set working directory".to_string());
            }
        }

        match debug_server.set_argv(vec![bundle_path.clone(), bundle_path.clone()]) {
            Ok(res) => {
                debug!("Successfully set argv: {:?}", res);
            }
            Err(e) => {
                debug!("Error setting argv: {:?}", e);
                return Err("Unable to set argv".to_string());
            }
        }

        match debug_server.send_command("qLaunchSuccess".into()) {
            Ok(res) => debug!("Got launch response: {:?}", res),
            Err(e) => {
                debug!("Error checking if app launched: {:?}", e);
                return Err("Unable to check if app launched".to_string());
            }
        }

        match debug_server.send_command("D".into()) {
            Ok(res) => debug!("Detaching: {:?}", res),
            Err(e) => {
                debug!("Error detaching: {:?}", e);
                return Err("Unable to detach".to_string());
            }
        }

        Ok(())
    }

    pub async fn get_ios_version(&self) -> Result<String, String> {
        let device = match self.connect().await {
            Ok(device) => device,
            Err(_) => {
                return Err("Unable to connect to device".to_string());
            }
        };

        let lockdown_client = match device.new_lockdownd_client("ideviceimagemounter".to_string()) {
            Ok(lckd) => {
                debug!("Successfully connected to lockdownd");
                lckd
            }
            Err(e) => {
                debug!("Error starting lockdown service: {:?}", e);
                return Err("Unable to start lockdown".to_string());
            }
        };

        let ios_version =
            match lockdown_client.get_value("ProductVersion".to_string(), "".to_string()) {
                Ok(ios_version) => ios_version.get_string_val().unwrap(),
                Err(e) => {
                    debug!("Error getting iOS version: {:?}", e);
                    return Err("Unable to get iOS version".to_string());
                }
            };

        debug!("iOS version: {}", ios_version);

        Ok(ios_version)
    }

    pub async fn get_dmg_path(&self) -> Result<String, String> {
        let ios_version = self.get_ios_version().await?;

        // Check if directory exists
        let path = std::path::Path::new(&self.dmg_path).join(format!("{}.dmg", &ios_version));
        debug!("Checking if {} exists", path.display());
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
            debug!("Downloading iOS dictionary...");
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
        debug!("Downloading iOS {} DMG...", ios_version.clone());
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
        debug!("tmp path {}", tmp_path);
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

    pub async fn upload_dev_dmg(&self) -> Result<(), String> {
        let device = match self.connect().await {
            Ok(device) => device,
            Err(_) => {
                return Err("Unable to connect to device".to_string());
            }
        };
        let dmg_path = self.get_dmg_path().await?;

        let mut lockdown_client =
            match device.new_lockdownd_client("ideviceimagemounter".to_string()) {
                Ok(lckd) => {
                    debug!("Successfully connected to lockdownd");
                    lckd
                }
                Err(e) => {
                    debug!("Error starting lockdown service: {:?}", e);
                    return Err("Unable to start lockdown".to_string());
                }
            };

        let service = match lockdown_client
            .start_service("com.apple.mobile.mobile_image_mounter".to_string())
        {
            Ok(service) => {
                debug!("Successfully started com.apple.mobile.mobile_image_mounter");
                service
            }
            Err(e) => {
                debug!(
                    "Error starting com.apple.mobile.mobile_image_mounter: {:?}",
                    e
                );
                return Err("Unable to start com.apple.mobile.mobile_image_mounter".to_string());
            }
        };

        let mim = match device.new_mobile_image_mounter(&service) {
            Ok(mim) => {
                debug!("Successfully started mobile_image_mounter");
                mim
            }
            Err(e) => {
                debug!("Error starting mobile_image_mounter: {:?}", e);
                return Err("Unable to start mobile_image_mounter".to_string());
            }
        };

        debug!("Uploading DMG from: {}", dmg_path);
        debug!(
            "signature: {}",
            format!("{}.signature", dmg_path.clone()).to_string()
        );
        match mim.upload_image(
            dmg_path.clone(),
            "Developer".to_string(),
            format!("{}.signature", dmg_path.clone()).to_string(),
        ) {
            Ok(_) => {
                debug!("Successfully uploaded image");
            }
            Err(e) => {
                debug!("Error uploading image: {:?}", e);
                return Err("Unable to upload developer disk image".to_string());
            }
        }
        match mim.mount_image(
            dmg_path.clone(),
            "Developer".to_string(),
            format!("{}.signature", dmg_path.clone()).to_string(),
        ) {
            Ok(_) => {
                debug!("Successfully mounted image");
            }
            Err(e) => {
                debug!("Error mounting image: {:?}", e);
                return Err("Unable to mount developer disk image".to_string());
            }
        }
        Ok(())
    }
}
