// jkcoxson

pub const VERSION: &str = "0.1.2";

use backend::Backend;
use bytes::BufMut;
use futures::TryStreamExt;
use log::{info, warn};
use plist_plus::Plist;
use serde_json::Value;
use std::{net::SocketAddr, str::FromStr, sync::Arc};
use tokio::{
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
mod packets;

#[tokio::main]
async fn main() {
    println!("Starting JitStreamer...");

    env_logger::init();
    println!("Logger initialized");

    let config = config::Config::load();
    let static_dir = config.static_path.clone();
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
        .and_then(move |addr| potential_pair(addr, potential_backend.clone()));

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
        .or(version_route)
        .or(census_route)
        .or(unregister_route)
        .or(admin_route);
    let ssl_routes = routes.clone();

    let addr: std::net::SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .expect("Invalid address");
    if config.ssl_port.is_some() {
        let addr: std::net::SocketAddr = format!("{}:{}", config.host, config.ssl_port.unwrap())
            .parse()
            .expect("Invalid address");
        println!("Hosting with HTTPS");
        tokio::spawn(async move {
            warp::serve(ssl_routes)
                .tls()
                .cert_path(config.ssl_cert.unwrap())
                .key_path(config.ssl_key.unwrap())
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
    Ok(VERSION)
}

async fn census(backend: Arc<Mutex<Backend>>) -> Result<impl Reply, Rejection> {
    let lock = backend.lock().await;
    Ok(packets::census_response(
        lock.counter.clone(),
        lock.deserialized_clients.len(),
        VERSION.to_string(),
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
            return Ok(packets::upload_response(
                false,
                "Device did not respond to pairing test",
            ));
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
        return Ok(packets::status_packet(false, false));
    }
    if !backend.check_ip(&addr.unwrap().to_string()) {
        return Ok(packets::status_packet(false, false));
    }
    match backend.get_by_ip(&addr.unwrap().ip().to_string()) {
        Some(_) => return Ok(packets::status_packet(true, true)),
        None => return Ok(packets::status_packet(true, false)),
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
    drop(backend);

    let (tx, mut rx) = mpsc::channel(1);

    tokio::task::spawn_blocking(move || {
        let mut i = 5;
        loop {
            match client.attach_debugger(pid) {
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
