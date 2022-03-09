// jkcoxson

use backend::Backend;
use bytes::BufMut;
use futures::TryStreamExt;
use packets::*;
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
mod config;
mod packets;

#[tokio::main]
async fn main() {
    let config = config::Config::load();
    let current_dir = std::env::current_dir().expect("failed to read current directory");
    let backend = Arc::new(Mutex::new(backend::Backend::load(
        current_dir.join(config.database_path),
    )));
    let status_backend = backend.clone();

    // Listen for /api/upload
    let upload_route = warp::path("upload")
        .and(warp::post())
        .and(warp::multipart::form().max_length(5_000_000))
        .and_then(move |form| upload_file(form, backend.clone()));

    // Listen for /status/
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

    let routes = root_redirect()
        .or(warp::fs::dir(current_dir.join("../JitStreamerSite/dist")))
        .or(upload_route)
        .or(status_route)
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
    backend: Arc<Mutex<Backend>>,
) -> Result<impl Reply, Rejection> {
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
            let value = String::from_utf8(value).unwrap();
            // Attempt to parse it as an Apple Plist
            let plist: Plist = Plist::from_xml(value).unwrap();
            let udid = plist.dict_get_item("UDID").unwrap();
            println!("{}", udid.get_string_val().unwrap());
        }
    }

    Ok("success")
}

async fn status(
    addr: Option<SocketAddr>,
    backend: Arc<Mutex<Backend>>,
) -> Result<impl Reply, Rejection> {
    let mut backend = backend.lock().await;
    Ok("success")
}
