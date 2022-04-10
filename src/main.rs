// jkcoxson

use backend::Backend;
use bytes::BufMut;
use futures::TryStreamExt;
use rusty_libimobiledevice::{debug};
use plist_plus::Plist;
use std::{net::SocketAddr, str::FromStr, sync::Arc};
use tokio::sync::Mutex;
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
mod packets;

#[tokio::main]
async fn main() {
    println!("Starting JitStreamer...");
    let config = config::Config::load();
    let static_dir = config.static_path.clone();
    let current_dir = std::env::current_dir().expect("failed to read current directory");
    let backend = Arc::new(Mutex::new(backend::Backend::load(&config)));
    let upload_backend = backend.clone();
    let status_backend = backend.clone();
    let list_apps_backend = backend.clone();
    let shortcuts_launch_backend = backend.clone();
    let shortcuts_unregister_backend = backend.clone();

    // Upload route
    let upload_route = warp::path("upload")
        .and(warp::post())
        .and(warp::multipart::form().max_length(5_000_000))
        .and(warp::filters::addr::remote())
        .and_then(move |form, addr| upload_file(form, addr, upload_backend.clone()));

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

    // Version route
    let version_route = warp::path("version")
        .and(warp::get())
        .and_then(|| version_route());

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

    // Assemble routes for service
    let routes = root_redirect()
        .or(warp::fs::dir(current_dir.join(static_dir)))
        .or(upload_route)
        .or(status_route)
        .or(list_apps_route)
        .or(shortcuts_launch_route)
        .or(version_route)
        .or(unregister_route)
        .or(admin_route);

    let addr: std::net::SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .expect("Invalid address");
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
    Ok("0.1.1")
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
    debug!("Device list requested");
    let mut backend = backend.lock().await;
    if let None = addr {
        debug!("No address provided");
        return Ok(packets::list_apps_response(
            false,
            "Unable to get IP address",
            vec![],
        ));
    }
    if !backend.check_ip(&addr.unwrap().to_string()) {
        debug!("Address not allowed");
        return Ok(packets::list_apps_response(
            false,
            "Address not allowed, connect to the VLAN",
            vec![],
        ));
    }
    let client = match backend.get_by_ip(&addr.unwrap().ip().to_string()) {
        Some(client) => client,
        None => {
            debug!("No client found with the given IP");
            return Ok(packets::list_apps_response(
                false,
                "No client found with the given IP, please register your device",
                vec![],
            ));
        }
    };
    drop(backend);
    let v = match client.get_apps().await {
        Ok(v) => v,
        Err(e) => {
            debug!("Unable to get apps");
            return Ok(packets::list_apps_response(
                false,
                &format!("Unable to get apps: {}", e).to_string(),
                vec![],
            ));
        }
    };

    // Trim the list of apps
    let mut apps = vec![];
    for i in v {
        if i.starts_with("com.apple") {
            continue;
        }
        if i.starts_with("com.google") {
            continue;
        }
        if i.to_lowercase().contains("dolphin") {
            apps.insert(0, i);
            continue;
        }
        if i.to_lowercase().contains("utm") {
            apps.insert(0, i);
            continue;
        }
        if i.to_lowercase().contains("provenance") {
            apps.insert(0, i);
            continue;
        }
        if i.to_lowercase().contains("delta") {
            apps.insert(0, i);
            continue;
        }
        apps.push(i);
    }

    Ok(packets::list_apps_response(true, "", apps))
}

async fn shortcuts_run(
    app: String,
    addr: Option<SocketAddr>,
    backend: Arc<Mutex<Backend>>,
) -> Result<impl Reply, Rejection> {
    debug!("Device has sent request to launch {}", app);
    let mut backend = backend.lock().await;
    if let None = addr {
        debug!("No address provided");
        return Ok(packets::launch_response(false, "Unable to get IP address"));
    }
    if !backend.check_ip(&addr.unwrap().to_string()) {
        debug!("Address not allowed");
        return Ok(packets::list_apps_response(
            false,
            "Address not allowed, connect to the VLAN",
            vec![],
        ));
    }
    let client = match backend.get_by_ip(&addr.unwrap().ip().to_string()) {
        Some(client) => client,
        None => {
            debug!("No client found with the given IP");
            return Ok(packets::launch_response(
                false,
                "No client found with the given IP, please register your device",
            ));
        }
    };
    drop(backend);

    match client.debug_app(app.clone()).await {
        Ok(_) => {
            return Ok(packets::launch_response(true, ""));
        }
        Err(e) => {
            return Ok(packets::launch_response(false, &e));
        }
    };
}

async fn shortcuts_unregister(
    addr: Option<SocketAddr>,
    backend: Arc<Mutex<Backend>>,
) -> Result<impl Reply, Rejection> {
    debug!("Device has sent request unregister");
    let mut backend = backend.lock().await;
    if let None = addr {
        debug!("No address provided");
        return Ok(packets::launch_response(false, "Unable to get IP address"));
    }
    if !backend.check_ip(&addr.unwrap().to_string()) {
        debug!("Address not allowed");
        return Ok(packets::list_apps_response(
            false,
            "Address not allowed, connect to the VLAN",
            vec![],
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
