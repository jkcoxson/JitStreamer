// jkcoxson

use backend::Backend;
use bytes::BufMut;
use futures::TryStreamExt;
use rusty_libimobiledevice::plist::Plist;
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
    let config = config::Config::load();
    let current_dir = std::env::current_dir().expect("failed to read current directory");
    let backend = Arc::new(Mutex::new(backend::Backend::load(&config)));
    let upload_backend = backend.clone();
    let status_backend = backend.clone();
    let list_apps_backend = backend.clone();
    let shortcuts_launch_backend = backend.clone();

    // Upload route
    let upload_route = warp::path("upload")
        .and(warp::post())
        .and(warp::multipart::form().max_length(5_000_000))
        .and(warp::filters::addr::remote())
        .and_then(move |form, addr| upload_file(form, addr, upload_backend.clone()));

    // Status route
    let status_route = warp::path("status")
        .and(warp::post())
        .and(warp::filters::addr::remote())
        .and_then(move |addr| status(addr, status_backend.clone()));

    // Admin route
    let admin_route = warp::path("admin").map(|| {
        warp::redirect(Uri::from_static(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ", // haha gottem
        ))
    });

    // Shortcuts route
    let list_apps_route = warp::path!("shortcuts" / "list_apps")
        .and(warp::get())
        .and(warp::filters::addr::remote())
        .and_then(move |addr| list_apps(addr, list_apps_backend.clone()));

    let shortcuts_launch_route = warp::path!("shortcuts" / "launch" / String)
        .and(warp::post())
        .and(warp::filters::addr::remote())
        .and_then(move |query, addr| shortcuts_run(query, addr, shortcuts_launch_backend.clone()));

    let routes = root_redirect()
        .or(warp::fs::dir(current_dir.join("../JitStreamerSite/dist")))
        .or(upload_route)
        .or(status_route)
        .or(list_apps_route)
        .or(shortcuts_launch_route)
        .or(admin_route);

    let addr: std::net::SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .expect("Invalid address");
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

async fn upload_file(
    form: FormData,
    address: Option<SocketAddr>,
    backend: Arc<Mutex<Backend>>,
) -> Result<impl Reply, Rejection> {
    let mut backend = backend.lock().await;
    let parts: Vec<Part> = form.try_collect().await.map_err(|e| {
        eprintln!("form error: {}", e);
        warp::reject::reject()
    })?;

    for p in parts {
        if p.name() == "file" {
            let value = p
                .stream()
                .try_fold(Vec::new(), |mut vec, data| {
                    vec.put(data);
                    async move { Ok(vec) }
                })
                .await
                .map_err(|e| {
                    eprintln!("reading file error: {}", e);
                    warp::reject::reject()
                })?;

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
                Ok(s) => s.get_string_val().unwrap(),
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
            match backend.write_pairing_file(plist.to_string(), &udid) {
                Ok(_) => {}
                Err(_) => {
                    return Ok(packets::upload_response(
                        false,
                        "Unable to save pairing file",
                    ));
                }
            }
            // Make sure that the client is valid before adding it to the backend
            match backend.get_by_ip(&address.ip().to_string()) {
                Some(client) => {
                    if client.udid == udid {
                        return Ok(packets::upload_response(true, ""));
                    }
                }
                None => {
                    return Ok(packets::upload_response(false, "Invalid client"));
                }
            }

            match backend.register_client(address.ip().to_string(), udid.clone()) {
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
    if !addr.unwrap().to_string().starts_with(&backend.allowed_ip) {
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
    println!("Device list requested");
    let mut backend = backend.lock().await;
    if let None = addr {
        println!("No address provided");
        return Ok(packets::list_apps_response(
            false,
            "Unable to get IP address",
            vec![],
        ));
    }
    if !addr.unwrap().to_string().starts_with(&backend.allowed_ip) {
        println!("Address not allowed");
        return Ok(packets::list_apps_response(
            false,
            "Address not allowed, connect to the VLAN",
            vec![],
        ));
    }
    let client = match backend.get_by_ip(&addr.unwrap().ip().to_string()) {
        Some(client) => client,
        None => {
            println!("No client found with the given IP");
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
        Err(_) => {
            println!("Unable to get apps");
            return Err(warp::reject());
        }
    };

    Ok(packets::list_apps_response(true, "", v))
}

async fn shortcuts_run(
    app: String,
    addr: Option<SocketAddr>,
    backend: Arc<Mutex<Backend>>,
) -> Result<impl Reply, Rejection> {
    println!("Device has sent request to launch {}", app);
    let mut backend = backend.lock().await;
    if let None = addr {
        println!("No address provided");
        return Ok(packets::launch_response(false, "Unable to get IP address"));
    }
    if !addr.unwrap().to_string().starts_with(&backend.allowed_ip) {
        println!("Address not allowed");
        return Ok(packets::launch_response(
            false,
            "Address not allowed, connect to the VLAN",
        ));
    }
    let client = match backend.get_by_ip(&addr.unwrap().ip().to_string()) {
        Some(client) => client,
        None => {
            println!("No client found with the given IP");
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
            println!("Unable to run app");
            return Ok(packets::launch_response(false, &e));
        }
    };
}
