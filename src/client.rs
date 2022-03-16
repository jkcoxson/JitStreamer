// jkcoxson

use rusty_libimobiledevice::{
    debug,
    instproxy::InstProxyClient,
    libimobiledevice::{get_device, get_udid_list, Device},
    plist::{Plist, PlistDictIter},
};
use tokio::{io::AsyncWriteExt, net::TcpStream};

use crate::backend::DeserializedClient;

const SERVICE_NAME: &str = "12:34:56:78:90:AB@fe80::de52:85ff:fece:c422._apple-mobdev2._tcp";

pub struct Client {
    pub ip: String,
    pub udid: String,
}

impl Client {
    #[allow(dead_code)]
    pub fn new(ip: String, udid: String) -> Client {
        Client { ip, udid }
    }

    pub async fn connect(&self) -> Result<Device, ()> {
        // Determine if device is in the muxer
        for _ in 0..20 {
            // Get the UDID list
            match get_udid_list() {
                Ok(udids) => {
                    // If the device is in the UDID list, connect to it
                    if udids.contains(&self.udid) {
                        return Ok(match get_device(self.udid.clone()) {
                            Ok(device) => device,
                            Err(_) => {
                                return Err(());
                            }
                        });
                    } else {
                        // Send a request to usbmuxd2 to connect to it
                        let mut stream = match TcpStream::connect("127.0.0.1:32498").await {
                            Ok(stream) => stream,
                            Err(_) => {
                                return Err(());
                            }
                        };
                        // Send the register packet
                        match stream
                            .write_all(
                                format!("1\n{}\n{}\n{}\n", self.udid, SERVICE_NAME, self.ip)
                                    .as_bytes(),
                            )
                            .await
                        {
                            _ => (),
                        };
                        // Wait for half a second for it to register
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    }
                }
                Err(_) => {
                    return Err(());
                }
            }
        }
        Err(())
    }

    pub async fn disconnect(&self) -> Result<(), ()> {
        // Determine if device is in the muxer
        match get_udid_list() {
            Ok(udids) => {
                // If the device is in the UDID list, send the disconnect packet
                if udids.contains(&self.udid) {
                    let mut stream = match TcpStream::connect("127.0.0.1:32498").await {
                        Ok(stream) => stream,
                        Err(_) => {
                            return Err(());
                        }
                    };
                    // Send the unregister packet
                    match stream
                        .write_all(
                            format!("0\n{}\n{}\n{}\n", self.udid, SERVICE_NAME, "0.0.0.0")
                                .as_bytes(),
                        )
                        .await
                    {
                        _ => (),
                    };
                    return Ok(());
                } else {
                    return Ok(());
                }
            }
            Err(_) => {
                return Err(());
            }
        }
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

        let mut p_iter = PlistDictIter::from(lookup_results);
        let mut apps = Vec::new();
        loop {
            let app = match p_iter.next_item() {
                Some(app) => app,
                None => break,
            };
            apps.push(app.0);
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
                    self.upload_dev_dmg().await?;
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

        // Get current directory
        let current_dir = match std::env::current_dir() {
            Ok(dir) => dir.canonicalize().unwrap().to_string_lossy().to_string(),
            Err(_) => {
                return Err("Unable to get current directory".to_string());
            }
        };

        // Check if directory exists
        let path = format!("{}/dmg/{}.dmg", &current_dir, &ios_version);
        if std::path::Path::new(&path).exists() {
            return Ok(path);
        }
        // Download versions.json from GitHub
        debug!("Downloading iOS dictionary...");
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
        let ios_dmg_url = match versions.get(ios_version.clone()) {
            Some(x) => x.as_str().unwrap().to_string(),
            None => return Err("DMG library does not contain your iOS version".to_string()),
        };
        // Download DMG zip
        debug!("Downloading iOS {} DMG...", ios_version.clone());
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
        let tmp_path = format!("{}/dmg/tmp", &current_dir);
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
                dmg_path = entry.path();
            }
        }
        // Move DMG to JIT Shipper directory
        let ios_dmg = dmg_path.join("DeveloperDiskImage.dmg");
        std::fs::rename(ios_dmg, format!("{}/dmg/{}.dmg", &current_dir, ios_version)).unwrap();
        let ios_sig = dmg_path.join("DeveloperDiskImage.dmg.signature");
        std::fs::rename(
            ios_sig,
            format!("{}/dmg/{}.dmg.signature", &current_dir, ios_version),
        )
        .unwrap();

        // Remove tmp path
        std::fs::remove_dir_all(tmp_path).unwrap();
        println!(
            "Successfully downloaded and extracted iOS {} developer disk image",
            ios_version
        );

        // Return DMG path
        Ok(format!("{}/dmg/{}.dmg", &current_dir, ios_version))
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

impl From<&DeserializedClient> for Client {
    fn from(client: &DeserializedClient) -> Self {
        Client {
            ip: client.ip.clone(),
            udid: client.udid.clone(),
        }
    }
}
