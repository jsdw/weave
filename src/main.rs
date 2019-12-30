#[macro_use] mod errors;
mod examples;
mod routes;
mod location;
mod matcher;
mod logging;

use std::env;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use clap::{ App, AppSettings, crate_version };
use hyper::{ Client, Body, Request, Response, Server };
use hyper::service::{ service_fn, make_service_fn };
use hyper_tls::HttpsConnector;
use tokio::{ self, fs };
use colored::*;
use futures_util::future::join_all;

use routes::{ Route };
use location::{ ResolvedLocation };
use matcher::Matcher;
use errors::{ Error };

use log::{ debug, info, warn, error };


/// Our application entry point:
#[tokio::main]
async fn main() {
    logging::init();
    debug!("Starting");
    if let Err(e) = run().await {
        error!("{}", e);
    }
}

/// Run the application, returning early on any synchronous errors:
async fn run() -> Result<(), Error> {

    let (routes, other_args) = routes::from_args(env::args().skip(1)).map_err(|e| {
        err!("failed to parse routes: {}", e)
    })?;

    let _ = App::new("weave")
        .author("James Wilson <james@jsdw.me>")
        .about("A lightweight HTTP router and file server.")
        .version(crate_version!())
        .after_help(&*examples::text())
        .usage("weave SOURCE to DEST [and SOURCE to DEST ...] [OPTIONS]")
        .setting(AppSettings::NoBinaryName)
        .get_matches_from(other_args);

    if routes.is_empty() {
        return Err(err!("No routes have been provided. Use -h or --help for more information"));
    }

    // Log our routes:
    for route in &routes {
        info!("Routing {} to {}", route.src, route.dest);
    }

    // Partition provided routes based on the SocketAddr we'll serve them on:
    let mut route_map = HashMap::new();
    for route in routes {
        let socket_addr = route.src_socket_addr()?;
        let rs: &mut Vec<Route> = route_map.entry(socket_addr).or_default();
        rs.push(route);
    }

    // Map each addr+route pair into a future that will handle requests:
    let servers = route_map.into_iter().map(|(socket_addr, routes)| {
        handle_requests(socket_addr, routes)
    });

    // Wait for these to finish (shouldn't happen unless they all fail):
    join_all(servers).await;
    Ok(())
}

/// Handle incoming requests by matching on routes and dispatching as necessary
async fn handle_requests(socket_addr: SocketAddr, routes: Vec<Route>) {

    let matcher = Arc::new(Matcher::new(routes));
    let make_service = make_service_fn(move |_| {
        let matcher = Arc::clone(&matcher);
        let svc = Ok::<_,Error>(service_fn(move |req| {
            let matcher = Arc::clone(&matcher);
            async move {
                let res = handle_request(req, &socket_addr, &matcher).await;
                // We don't return any errors, so need to tell Rust
                // what the error type would be:
                Result::<_,Error>::Ok(res)
            }
        }));

        // Return a Future:
        async { svc }
    });

    let server = Server::bind(&socket_addr).serve(make_service);
    if let Err(e) = server.await {
        error!("{}", e);
    }
}

/// Handle a single request, given a matcher that defines how to map from input to output:
async fn handle_request(req: Request<Body>, socket_addr: &SocketAddr, matcher: &Matcher) -> Response<Body> {
    let before_time = std::time::Instant::now();
    let src_path = format!("{}{}", socket_addr, req.uri());
    let dest_path = matcher.resolve(req.uri());

    match dest_path {
        None => {
            let duration = before_time.elapsed();
            let not_found_string = format!("[no matching routes] {} in {:#?}", src_path, duration);
            warn!("{}", not_found_string.red());
            Response::builder()
                .status(404)
                .body(Body::from("Weave: No routes matched"))
                .unwrap()
        },
        Some(dest_path) => {
            match do_handle_request(req, &dest_path).await {
                Ok(resp) => {
                    let duration = before_time.elapsed();
                    let status_code = resp.status().as_u16();

                    let info_string = format!("[{}] {} to {} in {:#?}",
                        resp.status().as_str(),
                        src_path,
                        dest_path.to_string(),
                        duration);

                    let info_string_colored =
                        if status_code >= 200 && status_code < 300 { info_string.green() }
                        else if status_code >= 300 && status_code < 400 { info_string.yellow() }
                        else { info_string.red() };

                    info!("{}", info_string_colored);
                    resp
                },
                Err(err) => {
                    let duration = before_time.elapsed();
                    let error_string = format!("[500] {} to {} ({}) in {:#?}",
                        src_path,
                        dest_path.to_string(),
                        err,
                        duration);
                    warn!("{}", error_string.red());
                    Response::builder()
                        .status(500)
                        .body(Body::from(format!("Weave: {}", err)))
                        .unwrap()
                }
            }
        }
    }

}

async fn do_handle_request(mut req: Request<Body>, dest_path: &ResolvedLocation) -> Result<Response<Body>, Error> {
    match dest_path {
        // Proxy to the URI our request matched against:
        ResolvedLocation::Url(url) => {
            // Set the request URI to our new destination:
            *req.uri_mut() = format!("{}", url).parse().unwrap();
            // Remove the host header (it's set according to URI if not present):
            req.headers_mut().remove("host");
            // Support HTTPS:
            let https = HttpsConnector::new();
            // Proxy the request through and pass back the response:
            let response = Client::builder()
                .build(https)
                .request(req)
                .await?;
            Ok(response)
        },
        // Proxy to the filesystem:
        ResolvedLocation::FilePath(path) => {

            let mut file = Err(err!("File not found"));
            let mut mime = None;

            for end in &["", "index.htm", "index.html"] {
                let mut p = path.clone();
                if !end.is_empty() { p.push(end) }
                mime = Some(mime_guess::from_path(&p).first_or_octet_stream());
                file = fs::read(p).await.map_err(|e| err!("{}", e));
                if file.is_ok() { break }
            }

            let response = match file {
                Ok(file) => {
                    Response::builder()
                        .status(200)
                        .header("Content-Type", mime.unwrap().as_ref())
                        .body(Body::from(file))
                        .unwrap()
                },
                Err(e) => {
                    let msg = format!("Weave: Could not read file '{}': {}", path.to_string_lossy(), e);
                    Response::builder()
                        .status(404)
                        .body(Body::from(msg))
                        .unwrap()
                }
            };
            Ok(response)
        }
    }
}
