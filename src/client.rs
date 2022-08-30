// jkcoxson

use log::{info, warn};
use plist_plus::Plist;
use rusty_libimobiledevice::{idevice::Device, services::instproxy::InstProxyClient};
use std::{
    collections::HashMap,
    net::IpAddr,
    str::FromStr,
    sync::{Arc, Mutex},
};

use crate::{
    heartbeat::Heart,
    messages::{DETACH, LOOKUP_APPS, MOUNTING, START_DEBUG_SERVER, START_INSTPROXY},
};

pub struct Client {
    pub ip: String,
    pub udid: String,
    pub pairing_file: String,
    pub dmg_path: String,
    pub heart: Arc<Mutex<Heart>>,
    pub mounts: Arc<Mutex<HashMap<String, String>>>,
}

impl Client {
    #[allow(dead_code)]
    pub fn new(
        ip: String,
        udid: String,
        pairing_file: String,
        dmg_path: String,
        heart: Arc<Mutex<Heart>>,
        mounts: Arc<Mutex<HashMap<String, String>>>,
    ) -> Client {
        Client {
            ip,
            udid,
            pairing_file,
            dmg_path,
            heart,
            mounts,
        }
    }

    /// Connects to a given device and runs preflight operations.
    pub fn connect(&self) -> Result<Device, String> {
        // Determine if device is in the muxer
        let ip = match IpAddr::from_str(&self.ip) {
            Ok(ip) => ip,
            Err(e) => {
                warn!("Error parsing ip: {}", e);
                return Err("Unable to parse ip".to_string());
            }
        };
        let device = Device::new(self.udid.clone(), Some(ip), 0);
        info!("Starting heartbeat {}", self.udid);

        // Start heartbeat
        (*self.heart.lock().unwrap()).start(&device);

        Ok(device)
    }

    pub fn get_apps(&self) -> Result<Plist, String> {
        let device = match self.connect() {
            Ok(device) => device,
            Err(_) => {
                return Err("Unable to connect to device".to_string());
            }
        };

        let instproxy_client = match device.new_instproxy_client("jitstreamer") {
            Ok(instproxy) => instproxy,
            Err(e) => {
                warn!("Error starting instproxy: {:?}", e);
                (*self.heart.lock().unwrap()).kill(device.get_udid());
                return Err(format!("{} {:?}", START_INSTPROXY, e));
            }
        };
        let client_opts = InstProxyClient::create_return_attributes(
            vec![("ApplicationType", Plist::new_string("Any"))],
            vec!["CFBundleIdentifier", "CFBundleDisplayName"],
        );
        let lookup_results = match instproxy_client.lookup(vec![], Some(client_opts)) {
            Ok(apps) => apps,
            Err(e) => {
                warn!("Error looking up apps: {:?}", e);
                (*self.heart.lock().unwrap()).kill(device.get_udid());
                return Err(format!("{} {:?}", LOOKUP_APPS, e));
            }
        };

        (*self.heart.lock().unwrap()).kill(device.get_udid());

        Ok(lookup_results)
    }

    pub fn debug_app(&self, app: String) -> Result<(), String> {
        let device = match self.connect() {
            Ok(device) => device,
            Err(_) => {
                return Err("Unable to connect to device".to_string());
            }
        };

        let instproxy_client = match device.new_instproxy_client("idevicedebug") {
            Ok(instproxy) => instproxy,
            Err(e) => {
                warn!("Error starting instproxy: {:?}", e);
                (*self.heart.lock().unwrap()).kill(device.get_udid());
                return Err(format!("{} {:?}", START_INSTPROXY, e));
            }
        };
        let client_opts = InstProxyClient::create_return_attributes(
            vec![("ApplicationType", Plist::new_string("Any"))],
            vec!["CFBundleIdentifier", "CFBundleExecutable", "Container"],
        );
        let lookup_results = match instproxy_client.lookup(vec![app.clone()], Some(client_opts)) {
            Ok(apps) => apps,
            Err(e) => {
                warn!("Error looking up apps: {:?}", e);
                (*self.heart.lock().unwrap()).kill(device.get_udid());
                return Err(format!("{} {:?}", LOOKUP_APPS, e));
            }
        };
        let lookup_results = lookup_results.dict_get_item(&app).unwrap();

        let working_directory = match lookup_results.dict_get_item("Container") {
            Ok(p) => p,
            Err(_) => {
                warn!("App not found");
                (*self.heart.lock().unwrap()).kill(device.get_udid());
                return Err("App not found".to_string());
            }
        };

        let working_directory = match working_directory.get_string_val() {
            Ok(p) => p,
            Err(_) => {
                warn!("App not found");
                (*self.heart.lock().unwrap()).kill(device.get_udid());
                return Err("App not found".to_string());
            }
        };
        info!("Working directory: {}", working_directory);

        let bundle_path = match instproxy_client.get_path_for_bundle_identifier(app) {
            Ok(p) => p,
            Err(e) => {
                warn!("Error getting path for bundle identifier: {:?}", e);
                (*self.heart.lock().unwrap()).kill(device.get_udid());
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
            // Check to see if the image is mounted already

            let mim = match device.new_mobile_image_mounter("jitstreamer") {
                Ok(mim) => {
                    info!("Successfully started mobile_image_mounter");
                    mim
                }
                Err(e) => {
                    warn!("Error starting mobile_image_mounter: {:?}", e);
                    return Err("Unable to start mobile_image_mounter".to_string());
                }
            };

            let path = match self.get_dmg_path() {
                Ok(p) => p,
                Err(e) => {
                    (*self.heart.lock().unwrap()).kill(device.get_udid());
                    return Err(e);
                }
            };

            let images = match mim.lookup_image("Developer") {
                Ok(images) => images,
                Err(e) => {
                    warn!("Error looking up images: {:?}", e);
                    (*self.heart.lock().unwrap()).kill(device.get_udid());
                    return Err("Unable to look up images".to_string());
                }
            };

            match images.dict_get_item("ImageSignature") {
                Ok(a) => match a.array_get_size() {
                    Ok(n) => {
                        if n > 0 {
                            warn!("Image already mounted, failed to start debug server");
                            (*self.heart.lock().unwrap()).kill(device.get_udid());
                            return Err(START_DEBUG_SERVER.to_string());
                        }
                    }
                    Err(_) => {
                        (*self.heart.lock().unwrap()).kill(device.get_udid());
                        return Err("Image plist in wrong format".to_string());
                    }
                },
                Err(_) => {
                    (*self.heart.lock().unwrap()).kill(device.get_udid());
                    return Err("Image plist in wrong format".to_string());
                }
            }

            let device = device.clone();
            let heart = self.heart.clone();
            let mounts = self.mounts.clone();
            tokio::task::spawn_blocking(move || {
                let mut i = 5;
                loop {
                    match Client::upload_dev_dmg(&device, &path, mounts.clone()) {
                        Ok(_) => {
                            (*heart.lock().unwrap()).kill(device.get_udid());
                            break;
                        }
                        Err(e) => {
                            warn!("Error uploading dmg: {:?}", e);
                            i -= 1;
                            if i == 0 {
                                (*heart.lock().unwrap()).kill(device.get_udid());
                                break;
                            }
                        }
                    }
                }
            });

            return Err(MOUNTING.to_string());
        }
        let debug_server = debug_server.unwrap();

        match debug_server.send_command("QSetMaxPacketSize: 1024".into()) {
            Ok(res) => {
                info!("Successfully set max packet size: {:?}", res);
            }
            Err(e) => {
                warn!("Error setting max packet size: {:?}", e);
                (*self.heart.lock().unwrap()).kill(device.get_udid());
                return Err("Unable to set max packet size".to_string());
            }
        }

        match debug_server.send_command(format!("QSetWorkingDir: {}", working_directory).into()) {
            Ok(res) => {
                info!("Successfully set working directory: {:?}", res);
            }
            Err(e) => {
                warn!("Error setting working directory: {:?}", e);
                (*self.heart.lock().unwrap()).kill(device.get_udid());
                return Err("Unable to set working directory".to_string());
            }
        }

        match debug_server.set_argv(vec![bundle_path.clone(), bundle_path]) {
            Ok(res) => {
                info!("Successfully set argv: {:?}", res);
            }
            Err(e) => {
                warn!("Error setting argv: {:?}", e);
                (*self.heart.lock().unwrap()).kill(device.get_udid());
                return Err("Unable to set argv".to_string());
            }
        }

        match debug_server.send_command("qLaunchSuccess".into()) {
            Ok(res) => info!("Got launch response: {:?}", res),
            Err(e) => {
                warn!("Error checking if app launched: {:?}", e);
                (*self.heart.lock().unwrap()).kill(device.get_udid());
                return Err("Unable to check if app launched".to_string());
            }
        }

        match debug_server.send_command("D".into()) {
            Ok(res) => info!("Detaching: {:?}", res),
            Err(e) => {
                warn!("Error detaching: {:?}", e);
                (*self.heart.lock().unwrap()).kill(device.get_udid());
                return Err(DETACH.to_string());
            }
        }

        (*self.heart.lock().unwrap()).kill(device.get_udid());

        Ok(())
    }

    pub fn attach_debugger(
        &self,
        pid: u16,
        mounts: Arc<Mutex<HashMap<String, String>>>,
    ) -> Result<(), String> {
        let device = self.connect()?;
        let debug_server = match device.new_debug_server("jitstreamer") {
            Ok(d) => d,
            Err(_) => {
                let path = match self.get_dmg_path() {
                    Ok(p) => p,
                    Err(_) => {
                        (*self.heart.lock().unwrap()).kill(device.get_udid());
                        return Err("Unable to get dmg path, the server was set up incorrectly!"
                            .to_string());
                    }
                };
                match Client::upload_dev_dmg(&device, &path, mounts) {
                    Ok(_) => match device.new_debug_server("jitstreamer") {
                        Ok(d) => d,
                        Err(_) => {
                            (*self.heart.lock().unwrap()).kill(device.get_udid());
                            return Err("Unable to get debug server".to_string());
                        }
                    },
                    Err(e) => {
                        warn!("Error uploading dmg: {:?}", e);
                        return Err("Unable to upload dmg".to_string());
                    }
                }
            }
        };

        let command = "vAttach;";

        // The PID will consist of 8 hex digits, so we need to pad it with 0s
        let pid = format!("{:X}", pid);
        let zeroes = 8 - pid.len();
        let pid = format!("{}{}", "0".repeat(zeroes), pid);
        let command = format!("{}{}", command, pid);
        info!("Sending command: {}", command);

        match debug_server.send_command(command.into()) {
            Ok(res) => info!("Successfully attached: {:?}", res),
            Err(e) => {
                warn!("Error attaching: {:?}", e);
                (*self.heart.lock().unwrap()).kill(device.get_udid());
                return Err("Unable to attach".to_string());
            }
        }

        match debug_server.send_command("D".into()) {
            Ok(res) => info!("Detaching: {:?}", res),
            Err(e) => {
                warn!("Error detaching: {:?}", e);
                (*self.heart.lock().unwrap()).kill(device.get_udid());
                return Err("Unable to detach".to_string());
            }
        }

        (*self.heart.lock().unwrap()).kill(device.get_udid());

        Ok(())
    }

    pub fn install_app(&self, _ipa: Vec<u8>) -> Result<(), String> {
        let device = self.connect()?;

        let _inst = device.new_instproxy_client("jitstreamer")?;

        todo!();
    }

    pub fn get_ios_version(&self) -> Result<String, String> {
        let device = match self.connect() {
            Ok(device) => device,
            Err(_) => {
                return Err("Unable to connect to device".to_string());
            }
        };

        let lockdown_client = match device.new_lockdownd_client("ideviceimagemounter") {
            Ok(lckd) => {
                info!("Successfully connected to lockdownd");
                lckd
            }
            Err(e) => {
                warn!("Error starting lockdown service: {:?}", e);
                return Err("Unable to start lockdown".to_string());
            }
        };

        let ios_version = match lockdown_client.get_value("ProductVersion", "") {
            Ok(ios_version) => ios_version.get_string_val().unwrap(),
            Err(e) => {
                warn!("Error getting iOS version: {:?}", e);
                return Err("Unable to get iOS version".to_string());
            }
        };

        info!("iOS version: {}", ios_version);

        Ok(ios_version)
    }

    pub fn get_dmg_path(&self) -> Result<String, String> {
        let ios_version = self.get_ios_version()?;

        // Check if directory exists
        let path = std::path::Path::new(&self.dmg_path).join(format!("{}.dmg", &ios_version));
        info!("Checking if {} exists", path.display());
        if path.exists() {
            return Ok(String::from(path.to_string_lossy()));
        }

        let mut ios_dmg_url = None;
        let dmg_libraries =
            ["https://raw.githubusercontent.com/jkcoxson/JitStreamer/master/versions.json"];
        for lib in dmg_libraries {
            if ios_dmg_url != None {
                break;
            }
            // Download versions.json from GitHub
            info!("Downloading iOS dictionary...");
            let response = match reqwest::blocking::get(lib) {
                Ok(response) => response,
                Err(_) => {
                    return Err("Error downloading versions.json".to_string());
                }
            };
            let contents = match response.text() {
                Ok(contents) => contents,
                Err(_) => {
                    return Err("Error reading versions.json".to_string());
                }
            };
            // Parse versions.json
            let versions: serde_json::Value = serde_json::from_str(&contents).unwrap();
            // Get DMG url
            ios_dmg_url = versions
                .get(ios_version.clone())
                .map(|x| x.as_str().unwrap().to_string());
        }

        if ios_dmg_url == None {
            return Err(format!(
                "Libraries did not contain a DMG for iOS {}",
                ios_version
            ));
        }

        // Download DMG zip
        info!("Downloading iOS {} DMG...", ios_version);
        let resp = match reqwest::blocking::get(ios_dmg_url.unwrap()) {
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
        let mut content = std::io::Cursor::new(match resp.bytes() {
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

    pub fn upload_dev_dmg(
        device: &Device,
        dmg_path: &String,
        mounts: Arc<Mutex<HashMap<String, String>>>,
    ) -> Result<(), String> {
        // Add the device to mounts
        if let Ok(mut mounts) = mounts.lock() {
            mounts.insert(device.get_udid(), "".to_string());
        }

        let mim = match device.new_mobile_image_mounter("jitstreamer") {
            Ok(mim) => {
                info!("Successfully started mobile_image_mounter");
                mim
            }
            Err(e) => {
                warn!("Error starting mobile_image_mounter: {:?}", e);
                if let Ok(mut mounts) = mounts.lock() {
                    mounts.insert(
                        device.get_udid(),
                        format!("Error starting mobile image mounter: {:?}", e),
                    );
                }
                return Err("Unable to start mobile_image_mounter".to_string());
            }
        };

        info!("Uploading DMG from: {}", dmg_path);
        info!("signature: {}", format!("{}.signature", dmg_path.clone()));
        match mim.upload_image(
            dmg_path.clone(),
            "Developer",
            format!("{}.signature", dmg_path.clone()),
        ) {
            Ok(_) => {
                info!("Successfully uploaded image");
            }
            Err(e) => {
                warn!("Error uploading image: {:?}", e);
                if let Ok(mut mounts) = mounts.lock() {
                    mounts.insert(device.get_udid(), format!("Error uploading image: {:?}", e));
                }
                return Err("Unable to upload developer disk image".to_string());
            }
        }
        match mim.mount_image(
            dmg_path.clone(),
            "Developer",
            format!("{}.signature", dmg_path.clone()),
        ) {
            Ok(_) => {
                info!("Successfully mounted image");
            }
            Err(e) => {
                warn!("Error mounting image: {:?}", e);
                if let Ok(mut mounts) = mounts.lock() {
                    mounts.insert(device.get_udid(), format!("Error mounting image: {:?}", e));
                }
                return Err("Unable to mount developer disk image".to_string());
            }
        }
        // Remove device from mounts
        if let Ok(mut mounts) = mounts.lock() {
            let _ = mounts.remove(&device.get_udid());
        }
        Ok(())
    }
}
