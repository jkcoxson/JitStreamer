// jkcoxson

pub const SHORTCUT_VERSION: &str = "0.1.2";

use backend::Backend;
use bytes::BufMut;
use futures::TryStreamExt;
use log::{info, warn};
use plist_plus::Plist;
use serde_json::Value;
use std::{net::SocketAddr, str::FromStr, sync::Arc};
use tokio::{
    io::AsyncWriteExt,
    sync::{mpsc, Mutex},
    time::timeout,
};
use warp::{
    filters::BoxedFilter,
    http::Uri,
    multipart::{FormData, Part},
    path::FullPath,
    redirect, Filter, Rejection, Reply,
};

mod backend;
mod client;
mod config;
mod heartbeat;
mod messages;
mod netmuxd;
mod packets;

#[tokio::main]
async fn main() {
    println!("Starting JitStreamer...");

    env_logger::init();
    println!("Logger initialized");

    let config = config::Config::load();
    let static_dir = config.paths.static_path.clone();
    let current_dir = std::env::current_dir().expect("failed to read current directory");
    let backend = Arc::new(Mutex::new(backend::Backend::load(&config)));
    let upload_backend = backend.clone();
    let potential_backend = backend.clone();
    let potential_follow_up_backend = backend.clone();
    let status_backend = backend.clone();
    let list_apps_backend = backend.clone();
    let shortcuts_launch_backend = backend.clone();
    let shortcuts_unregister_backend = backend.clone();
    let attach_backend = backend.clone();
    let census_backend = backend.clone();
    let install_app_backend = backend.clone();

    let cors = warp::cors().allow_any_origin();

    // Status route
    let status_route = warp::path("status")
        .and(warp::get())
        .and(warp::filters::addr::remote())
        .and_then(move |addr| status(addr, status_backend.clone()));

    // Admin route
    let admin_route = warp::path("admin").map(|| {
        warp::redirect(Uri::from_static(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ", // haha gottem
        ))
    });

    // Upload route
    let upload_route = warp::path("upload")
        .and(warp::post())
        .and(warp::multipart::form().max_length(5_000_000))
        .and(warp::filters::addr::remote())
        .and_then(move |form, addr| upload_file(form, addr, upload_backend.clone()));

    // Potential route
    let potential_route = warp::path("potential")
        .and(warp::get())
        .and(warp::filters::addr::remote())
        .and_then(move |addr| potential_pair(addr, potential_backend.clone()))
        .with(cors);

    // Potential follow up route
    let potential_follow_up_route = warp::path!("potential_follow_up" / u16)
        .and(warp::post())
        .and(warp::body::content_length_limit(1024 * 1024 * 10))
        .and(warp::body::bytes())
        .and_then(move |code: u16, bytes: bytes::Bytes| {
            potential_follow_up(bytes, code, potential_follow_up_backend.clone())
        });

    // Version route
    let version_route = warp::path("version")
        .and(warp::get())
        .and_then(|| version_route());

    // Census route
    let census_route = warp::path("census")
        .and(warp::get())
        .and_then(move || census(census_backend.clone()));

    // Shortcuts route
    let list_apps_route = warp::path!("shortcuts" / "list_apps")
        .and(warp::get())
        .and(warp::filters::addr::remote())
        .and_then(move |addr| list_apps(addr, list_apps_backend.clone()));

    let shortcuts_launch_route = warp::path!("shortcuts" / "launch" / String)
        .and(warp::post())
        .and(warp::filters::addr::remote())
        .and_then(move |query, addr| shortcuts_run(query, addr, shortcuts_launch_backend.clone()));

    let unregister_route = warp::path!("shortcuts" / "unregister")
        .and(warp::post())
        .and(warp::filters::addr::remote())
        .and_then(move |addr| shortcuts_unregister(addr, shortcuts_unregister_backend.clone()));

    let attach_route = warp::path!("attach" / u16)
        .and(warp::post())
        .and(warp::filters::addr::remote())
        .and_then(move |code: u16, addr| attach_debugger(code, addr, attach_backend.clone()));

    let netmuxd_route = warp::path("netmuxd")
        .and(warp::post())
        .and(warp::filters::addr::remote())
        .and_then(move |addr| netmuxd_connect(addr, backend.clone()));

    let install_app_route = warp::path!("install" / "app")
        .and(warp::post())
        .and(warp::filters::addr::remote())
        .and(warp::body::content_length_limit(1024 * 1024 * 10))
        .and(warp::body::bytes())
        .and_then(move |addr, bytes: bytes::Bytes| {
            install_app(addr, install_app_backend.clone(), bytes)
        });

    // Assemble routes for service
    let routes = root_redirect()
        .or(warp::fs::dir(current_dir.join(static_dir)))
        .or(status_route)
        .or(upload_route)
        .or(potential_route)
        .or(potential_follow_up_route)
        .or(list_apps_route)
        .or(shortcuts_launch_route)
        .or(attach_route)
        .or(netmuxd_route)
        .or(install_app_route)
        .or(version_route)
        .or(census_route)
        .or(unregister_route)
        .or(admin_route);
    let ssl_routes = routes.clone();

    let addr: std::net::SocketAddr =
        format!("{}:{}", config.web_server.host, config.web_server.port)
            .parse()
            .expect("Invalid address");
    if config.web_server.ssl_port.is_some() {
        let addr: std::net::SocketAddr = format!(
            "{}:{}",
            config.web_server.host,
            config.web_server.ssl_port.unwrap()
        )
        .parse()
        .expect("Invalid address");
        println!("Hosting with HTTPS");
        tokio::spawn(async move {
            warp::serve(ssl_routes)
                .tls()
                .cert_path(config.web_server.ssl_cert.unwrap())
                .key_path(config.web_server.ssl_key.unwrap())
                .run(addr)
                .await;
        });
    }
    println!("Ready!\n");
    warp::serve(routes).run(addr).await;
}

fn root_redirect() -> BoxedFilter<(impl Reply,)> {
    warp::path::full()
        .and_then(move |path: FullPath| async move {
            let path = path.as_str();

            // do not redirect if the path ends in a trailing slash
            // or contains a period (indicating a specific file, e.g. style.css)
            if path.ends_with("/") || path.contains(".") {
                return Err(warp::reject());
            }

            Ok(redirect::redirect(
                Uri::from_str(&[path, "/"].concat()).unwrap(),
            ))
        })
        .boxed()
}

async fn version_route() -> Result<impl Reply, Rejection> {
    Ok(SHORTCUT_VERSION)
}

async fn census(backend: Arc<Mutex<Backend>>) -> Result<impl Reply, Rejection> {
    let lock = backend.lock().await;
    Ok(packets::census_response(
        lock.counter.clone(),
        lock.deserialized_clients.len(),
        SHORTCUT_VERSION.to_string(),
    ))
}

async fn upload_file(
    form: FormData,
    address: Option<SocketAddr>,
    backend: Arc<Mutex<Backend>>,
) -> Result<impl Reply, Rejection> {
    let lock = backend.lock().await;
    let parts: Vec<Part> = match form.try_collect().await {
        Ok(parts) => parts,
        Err(_) => return Ok(packets::upload_response(false, "Form error")),
    };

    for p in parts {
        if p.name() == "file" {
            let value = match p
                .stream()
                .try_fold(Vec::new(), |mut vec, data| {
                    vec.put(data);
                    async move { Ok(vec) }
                })
                .await
            {
                Ok(value) => value,
                Err(_) => return Ok(packets::upload_response(false, "File error")),
            };

            // Get string from value
            let value = match String::from_utf8(value) {
                Ok(value) => value,
                Err(_) => {
                    return Ok(packets::upload_response(false, "Unable to read file"));
                }
            };
            // Attempt to parse it as an Apple Plist
            let plist: Plist = Plist::from_xml(value.clone()).unwrap();
            let udid = match plist.dict_get_item("UDID") {
                Ok(s) => match s.get_string_val() {
                    Ok(s) => s,
                    Err(_) => {
                        return Ok(packets::upload_response(
                            false,
                            "Unable to read UDID from Plist",
                        ));
                    }
                },
                _ => {
                    return Ok(packets::upload_response(false, "Invalid pairing file!"));
                }
            };
            let address = match address {
                Some(address) => address,
                None => {
                    return Ok(packets::upload_response(false, "No address provided"));
                }
            };
            let plist: Plist = Plist::from_xml(value).unwrap();
            // Save the plist to the plist storage directory
            match lock.write_pairing_file(plist.to_string(), &udid) {
                Ok(_) => {}
                Err(_) => {
                    return Ok(packets::upload_response(
                        false,
                        "Unable to save pairing file",
                    ));
                }
            }
            drop(lock);
            // Make sure that the client is valid before adding it to the backend
            match backend::Backend::test_new_client(&address.ip().to_string(), &udid).await {
                Ok(_) => {}
                Err(_) => {
                    return Ok(packets::upload_response(
                        false,
                        "Device did not respond to pairing test",
                    ));
                }
            }
            let mut lock = backend.lock().await;
            match lock.register_client(address.ip().to_string(), udid.clone()) {
                Ok(_) => {}
                Err(_) => {
                    return Ok(packets::upload_response(false, "Client already registered"));
                }
            }
            return Ok(packets::upload_response(true, ""));
        }
    }
    return Ok(packets::upload_response(false, "No file found"));
}

async fn potential_pair(
    addr: Option<SocketAddr>,
    backend: Arc<Mutex<Backend>>,
) -> Result<impl Reply, Rejection> {
    let mut backend = backend.lock().await;
    if let None = addr {
        return Ok(packets::potential_pair_response(
            false,
            "No address provided",
            0,
        ));
    }
    if !backend.check_ip(&addr.unwrap().to_string()) {
        return Ok(packets::potential_pair_response(
            false,
            "Invalid IP, join from the VLAN",
            0,
        ));
    }

    let code = backend.potential_pair(addr.unwrap().to_string());
    info!("A potential pair code was generated: {}", code);
    Ok(packets::potential_pair_response(true, "", code))
}

async fn potential_follow_up(
    form: bytes::Bytes,
    code: u16,
    backend: Arc<Mutex<Backend>>,
) -> Result<impl Reply, Rejection> {
    // Parse form to a string
    let value = match String::from_utf8(form.to_vec()) {
        Ok(form) => form,
        Err(_) => {
            return Ok(packets::potential_pair_response(false, "Invalid UTF-8", 0));
        }
    };

    let mut lock = backend.lock().await;
    let ip = match lock.check_code(code) {
        Some(ip) => ip,
        None => {
            return Ok(packets::potential_follow_up_response(false, "Invalid code"));
        }
    }
    .split(":")
    .next()
    .unwrap()
    .to_string();

    // Attempt to parse it as an Apple Plist
    let plist: Plist = Plist::from_xml(value.clone()).unwrap();
    let udid = match plist.dict_get_item("UDID") {
        Ok(s) => match s.get_string_val() {
            Ok(s) => s,
            Err(_) => {
                return Ok(packets::upload_response(
                    false,
                    "Unable to read UDID from Plist",
                ));
            }
        },
        _ => {
            return Ok(packets::potential_follow_up_response(
                false,
                "Invalid pairing file!",
            ));
        }
    };
    let plist: Plist = Plist::from_xml(value).unwrap();
    // Save the plist to the plist storage directory
    match lock.write_pairing_file(plist.to_string(), &udid) {
        Ok(_) => {}
        Err(_) => {
            return Ok(packets::upload_response(
                false,
                "Unable to save pairing file",
            ));
        }
    }
    drop(lock);
    // Make sure that the client is valid before adding it to the backend
    match backend::Backend::test_new_client(&ip, &udid).await {
        Ok(_) => {}
        Err(_) => {
            return Ok(packets::upload_response(false, messages::PAIRING_TEST));
        }
    }
    let mut lock = backend.lock().await;
    match lock.register_client(ip, udid.clone()) {
        Ok(_) => {}
        Err(_) => {
            return Ok(packets::upload_response(false, "Client already registered"));
        }
    }
    lock.remove_code(code);
    return Ok(packets::upload_response(true, ""));
}

async fn status(
    addr: Option<SocketAddr>,
    backend: Arc<Mutex<Backend>>,
) -> Result<impl Reply, Rejection> {
    let mut backend = backend.lock().await;
    if let None = addr {
        return Ok(packets::status_packet(false, false, false, ""));
    }
    if !backend.check_ip(&addr.unwrap().to_string()) {
        return Ok(packets::status_packet(false, false, false, ""));
    }
    match backend.get_by_ip(&addr.unwrap().ip().to_string()) {
        Some(client) => {
            // Check if the client is mounting
            let mut mounts = match backend.mounts.lock() {
                Ok(m) => m,
                Err(_) => {
                    warn!("Mutex poisoned!!");
                    return Ok(packets::status_packet(true, true, false, ""));
                }
            };

            match mounts.get(&client.udid) {
                Some(m) => {
                    let m = m.to_string();
                    if !m.is_empty() {
                        // Remove it from the HashMap
                        mounts.remove(&client.udid);
                    }
                    return Ok(packets::status_packet(true, true, true, &m));
                }
                None => {
                    return Ok(packets::status_packet(true, true, false, ""));
                }
            }
        }
        None => return Ok(packets::status_packet(true, false, false, "")),
    };
}

async fn list_apps(
    addr: Option<SocketAddr>,
    backend: Arc<Mutex<Backend>>,
) -> Result<impl Reply, Rejection> {
    info!("Device list requested");
    let mut lock = backend.lock().await;
    if let None = addr {
        warn!("No address provided");
        return Ok(packets::list_apps_response(
            false,
            "Unable to get IP address",
            serde_json::Value::Object(serde_json::Map::new()),
            serde_json::Value::Object(serde_json::Map::new()),
        ));
    }
    if !lock.check_ip(&addr.unwrap().to_string()) {
        warn!("Address not allowed");
        return Ok(packets::list_apps_response(
            false,
            "Address not allowed, connect to the VLAN",
            serde_json::Value::Object(serde_json::Map::new()),
            serde_json::Value::Object(serde_json::Map::new()),
        ));
    }
    let client = match lock.get_by_ip(&addr.unwrap().ip().to_string()) {
        Some(client) => client,
        None => {
            warn!("No client found with the given IP");
            return Ok(packets::list_apps_response(
                false,
                "No client found with the given IP, please register your device",
                serde_json::Value::Object(serde_json::Map::new()),
                serde_json::Value::Object(serde_json::Map::new()),
            ));
        }
    };
    drop(lock);

    let (tx, mut rx) = mpsc::channel(1);

    tokio::task::spawn_blocking(move || {
        let v = match client.get_apps() {
            Ok(v) => v,
            Err(e) => {
                warn!("Unable to get apps");
                tx.blocking_send(Err(packets::list_apps_response(
                    false,
                    &format!("Unable to get apps: {}", e).to_string(),
                    serde_json::Value::Object(serde_json::Map::new()),
                    serde_json::Value::Object(serde_json::Map::new()),
                )))
                .unwrap();
                return;
            }
        };
        tx.blocking_send(Ok(v)).unwrap();
    });

    let v = match rx.recv().await.unwrap() {
        Ok(v) => v,
        Err(e) => return Ok(e),
    };

    // Trim the list of apps
    let mut prefered_apps = Value::Object(serde_json::Map::new());
    let mut apps: Value = Value::Object(serde_json::Map::new());
    let mut count = 0;
    for i in v {
        let i = i.plist;
        let name = i
            .clone()
            .dict_get_item("CFBundleDisplayName")
            .unwrap()
            .get_string_val()
            .unwrap();
        let bundle_id = i
            .clone()
            .dict_get_item("CFBundleIdentifier")
            .unwrap()
            .get_string_val()
            .unwrap();
        if bundle_id.contains("com.apple") {
            continue;
        }
        if backend::Backend::prefered_app(&name) {
            prefered_apps[&name] = serde_json::Value::String(bundle_id);
        } else {
            apps[&name] = serde_json::Value::String(bundle_id);
        }
        count += 1;
    }

    let mut lock = backend.lock().await;
    lock.counter.fetched += count;

    let res = packets::list_apps_response(true, "", apps, prefered_apps);
    Ok(res)
}

async fn shortcuts_run(
    app: String,
    addr: Option<SocketAddr>,
    backend: Arc<Mutex<Backend>>,
) -> Result<impl Reply, Rejection> {
    info!("Device has sent request to launch {}", app);
    let mut lock = backend.lock().await;
    if let None = addr {
        warn!("No address provided");
        return Ok(packets::launch_response(false, "Unable to get IP address"));
    }
    if !lock.check_ip(&addr.unwrap().to_string()) {
        warn!("Address not allowed");
        return Ok(packets::launch_response(
            false,
            "Address not allowed, connect to the VLAN",
        ));
    }
    let client = match lock.get_by_ip(&addr.unwrap().ip().to_string()) {
        Some(client) => client,
        None => {
            warn!("No client found with the given IP");
            return Ok(packets::launch_response(
                false,
                "No client found with the given IP, please register your device",
            ));
        }
    };
    lock.counter.launched += 1;
    drop(lock);

    let (tx, mut rx) = mpsc::channel(1);

    tokio::task::spawn_blocking(move || {
        match client.debug_app(app.clone()) {
            Ok(_) => {
                tx.blocking_send(packets::launch_response(true, ""))
                    .unwrap();
            }
            Err(e) => {
                tx.blocking_send(packets::launch_response(false, &e))
                    .unwrap();
            }
        };
    });

    Ok(rx.recv().await.unwrap())
}

async fn attach_debugger(
    pid: u16,
    addr: Option<SocketAddr>,
    backend: Arc<Mutex<Backend>>,
) -> Result<impl Reply, Rejection> {
    info!("Device has sent request to attach to process {}", pid);
    let mut backend = backend.lock().await;
    if let None = addr {
        warn!("No address provided");
        return Ok(packets::attach_response(false, "Unable to get IP address"));
    }
    if !backend.check_ip(&addr.unwrap().to_string()) {
        warn!("Address not allowed");
        return Ok(packets::attach_response(
            false,
            "Address not allowed, connect to the VLAN",
        ));
    }
    let client = match backend.get_by_ip(&addr.unwrap().ip().to_string()) {
        Some(client) => client,
        None => {
            warn!("No client found with the given IP");
            return Ok(packets::attach_response(
                false,
                "No client found with the given IP, please register your device",
            ));
        }
    };
    backend.counter.attached += 1;
    let mounts = backend.mounts.clone();
    drop(backend);

    let (tx, mut rx) = mpsc::channel(1);

    tokio::task::spawn_blocking(move || {
        let mut i = 5;
        loop {
            match client.attach_debugger(pid, mounts.clone()) {
                Ok(_) => match tx.blocking_send(packets::attach_response(true, "")) {
                    Ok(_) => break,
                    Err(e) => {
                        warn!("Unable to send response: {}", e);
                        break;
                    }
                },
                Err(e) => {
                    if i == 0 {
                        match tx.blocking_send(packets::attach_response(false, &e)) {
                            Ok(_) => (),
                            Err(e) => {
                                warn!("Unable to send response: {}", e);
                            }
                        }
                        break;
                    }
                    i -= 1;
                }
            };
        }
    });

    match timeout(std::time::Duration::from_secs(60), rx.recv()).await {
        Ok(x) => match x {
            Some(x) => Ok(x),
            None => Ok(packets::attach_response(false, "Timeout")),
        },
        Err(_) => {
            warn!("Unable to receive response");
            Ok(packets::attach_response(
                false,
                "Unable to receive response",
            ))
        }
    }
}

async fn shortcuts_unregister(
    addr: Option<SocketAddr>,
    backend: Arc<Mutex<Backend>>,
) -> Result<impl Reply, Rejection> {
    info!("Device has sent request unregister");
    let mut backend = backend.lock().await;
    if let None = addr {
        warn!("No address provided");
        return Ok(packets::launch_response(false, "Unable to get IP address"));
    }
    if !backend.check_ip(&addr.unwrap().to_string()) {
        warn!("Address not allowed");
        return Ok(packets::unregister_response(
            false,
            "Address not allowed, connect to the VLAN",
        ));
    }
    match backend.unregister_client(addr.unwrap().ip().to_string()) {
        Ok(_) => return Ok(packets::unregister_response(true, "")),
        Err(_) => {
            return Ok(packets::unregister_response(
                false,
                "Device not found in database",
            ))
        }
    }
}

async fn netmuxd_connect(
    addr: Option<SocketAddr>,
    backend: Arc<Mutex<Backend>>,
) -> Result<impl Reply, Rejection> {
    info!("Device has sent request to connect to netmuxd");
    let addr = match addr {
        Some(addr) => addr,
        None => {
            warn!("No address provided");
            return Ok("Unable to get IP address");
        }
    };
    let mut backend = backend.lock().await;
    if !backend.check_ip(&addr.to_string()) {
        warn!("Address not allowed");
        return Ok("Address not allowed, connect to the VLAN");
    }
    let client = match backend.get_by_ip(&addr.ip().to_string()) {
        Some(client) => client,
        None => {
            warn!("No client found with the given IP");
            return Ok("No client found with the given IP, please register your device");
        }
    };
    let udid = client.udid.clone();
    let netmuxd_address = backend.netmuxd_address.clone();

    if netmuxd_address.is_none() {
        warn!("No netmuxd address provided");
        return Ok("No netmuxd address provided");
    }
    let netmuxd_address = netmuxd_address.unwrap();

    backend.counter.netmuxd += 1;

    drop(backend);

    // Determine if the muxer already contains the client
    match rusty_libimobiledevice::idevice::get_device(udid.clone()) {
        Ok(_) => {
            info!("Device already connected to netmuxd");
            return Ok("ok");
        }
        Err(_) => (),
    }

    // Send the packet to netmuxd
    let packet: Vec<u8> = match netmuxd::add_device_packet(addr.ip().to_string(), udid) {
        Ok(packet) => packet.into(),
        Err(_) => {
            warn!("Unable to build netmuxd packet");
            return Ok("Unable to build netmuxd packet");
        }
    };

    // Determine if the address is TCP or Unix
    match netmuxd_address.parse::<std::net::SocketAddr>() {
        Ok(addr) => {
            let stream = match tokio::net::TcpStream::connect(addr).await {
                Ok(stream) => stream,
                Err(e) => {
                    warn!("Unable to connect to netmuxd: {}", e);
                    return Ok("Unable to connect to netmuxd");
                }
            };
            // Send the packet
            let mut stream = tokio::io::BufWriter::new(stream);
            match stream.write_all(&packet).await {
                Ok(_) => (),
                Err(e) => {
                    warn!("Unable to send packet to netmuxd: {}", e);
                    return Ok("Unable to send packet to netmuxd");
                }
            };

            match stream.flush().await {
                Ok(_) => (),
                Err(e) => {
                    warn!("Unable to flush packet to netmuxd: {}", e);
                    return Ok("Unable to flush packet to netmuxd");
                }
            };
        }
        Err(_) => {
            let stream = match tokio::net::UnixStream::connect(netmuxd_address).await {
                Ok(stream) => stream,
                Err(e) => {
                    warn!("Unable to connect to netmuxd: {}", e);
                    return Ok("Unable to connect to netmuxd");
                }
            };
            // Send the packet
            let mut stream = tokio::io::BufWriter::new(stream);
            match stream.write_all(&packet).await {
                Ok(_) => (),
                Err(e) => {
                    warn!("Unable to send packet to netmuxd: {}", e);
                    return Ok("Unable to send packet to netmuxd");
                }
            };

            match stream.flush().await {
                Ok(_) => (),
                Err(e) => {
                    warn!("Unable to flush packet to netmuxd: {}", e);
                    return Ok("Unable to flush packet to netmuxd");
                }
            };
        }
    };

    return Ok("ok");
}

async fn install_app(
    addr: Option<SocketAddr>,
    backend: Arc<Mutex<Backend>>,
    ipa: bytes::Bytes,
) -> Result<impl Reply, Rejection> {
    info!("Device has sent request to install app");
    let addr = match addr {
        Some(addr) => addr,
        None => {
            warn!("No address provided");
            return Ok(packets::install_response(false, "Unable to get IP address"));
        }
    };
    let mut backend = backend.lock().await;
    if !backend.check_ip(&addr.to_string()) {
        warn!("Address not allowed");
        return Ok(packets::install_response(
            false,
            "Address not allowed, connect to the VLAN",
        ));
    }
    let client = match backend.get_by_ip(&addr.ip().to_string()) {
        Some(client) => client,
        None => {
            warn!("No client found with the given IP");
            return Ok(packets::install_response(
                false,
                "No client found with the given IP, please register your device",
            ));
        }
    };

    let (tx, mut rx) = mpsc::channel(1);

    tokio::task::spawn_blocking(move || {
        match client.install_app(ipa.to_vec()) {
            Ok(_) => {
                tx.blocking_send(packets::install_response(true, ""))
                    .unwrap();
            }
            Err(e) => {
                tx.blocking_send(packets::install_response(false, &e))
                    .unwrap();
            }
        };
    });

    Ok(rx.recv().await.unwrap())
}
