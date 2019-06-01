#![feature(async_await, await_macro)]

#[macro_use] mod errors;
mod routes;
mod location;
mod matcher;
mod logging;

use std::env;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use clap::{ App, AppSettings };
use hyper::{Client, Body, Request, Response, Server, rt};
use hyper::service::service_fn;
use hyper_tls::HttpsConnector;
use tokio::fs;
use ansi_term::Color::{ Green, Red, Yellow };

// 0.3 futures and a compat layer to bridge to 0.1:
use futures::future::{ FutureExt, TryFutureExt };
use futures::compat::*;

use routes::{ Route };
use location::{ ResolvedLocation };
use matcher::Matcher;
use errors::{ Error };

use log::{ debug, info, warn, error };

static EXAMPLES: &str = "EXAMPLES:

Serve static files from `./client/files` on `localhost:8080`, and redirect HTTP
requests starting with `localhost:8080/api` to `localhost:9090`:

weave 8080 to ./client/files and 8080/api to 9090
# Examples of routing given the above:
# http://localhost:8080/api/foo => http://localhost:9090/foo
# http://localhost:8080/api/bar/wibble => http://localhost:9090/bar/wibble
# http://localhost:8080/ => ./client/files/index.html
# http://localhost:8080/somefile => ./client/files/somefile
# http://localhost:8080/path/to/somefile => ./client/files/path/to/somefile


Visit google by navigating to `localhost:8080`:

weave 8080 to https://www.google.com
# Examples of routing given the above:
# http://localhost:8080/ => https://www.google.com/
# http://localhost:8080/favicon.ico => https://www.google.com/favicon.ico
# http://localhost:8080/favicon.ico/bar => https://www.google.com/favicon.ico/bar


Visit google by navigating to `localhost:8080/foo`:

weave 8080/foo to https://www.google.com
# Examples of routing given the above:
# http://localhost:8080/ => No route matches this
# http://localhost:8080/foo => https://www.google.com
# http://localhost:8080/foo/favicon.ico => https://www.google.com/favicon.ico


Serve files in your cwd by navigating to `0.0.0.0:8080` (makes them available to
anything that can see your machine):

weave 0.0.0.0:8080 to ./
# Examples of routing given the above:
# http://0.0.0.0:8080/ => ./index.html
# http://0.0.0.0:8080/somefile => ./somefile
# http://0.0.0.0:8080/path/to/somefile => ./path/to/somefile


Serve exactly `/favicon.ico` using a local file, but the rest of the site via
`localhost:9000`:

weave =8080/favicon.ico to ./favicon.ico and 8080 to 9090
# Examples of routing given the above:
# http://localhost:8080/ => http://localhost:9090
# http://localhost:8080/favicon.ico => ./favicon.ico
# http://localhost:8080/favicon.ico/bar => http://localhost:9090/favicon.ico/bar


Match any API version provided and move it to the end of the destination path:

weave '8080/(version)/api' to 'https://some.site/api/(version)'
# Examples of routing given the above:
# http://localhost:8080/v1/api => https://some.site/api/v1
# http://localhost:8080/v1/api/foo => https://some.site/api/v1/foo
# http://localhost:8080/wibble/api/foo => https://some.site/api/wibble/foo


Serve JSON files in a local folder as exactly `api/(filename)/v1` to mock a
simple API:

weave '=8080/api/(filename)/v1' to './files/(filename).json'
# Examples of routing given the above:
# http://localhost:8080/api/foo/v1 => ./files/foo.json
# http://localhost:8080/api/bar/v1 => ./files/bar.json
# http://localhost:8080/api/bar/v1/wibble => No route matches this


Match paths ending in `/api/(filename)` and serve up JSON files from a
local folder:

weave '=8080/(base..)/api/(filename)' to './files/(filename).json'
# Examples of routing given the above:
# http://localhost:8080/1/2/3/api/foo => ./files/foo.json
# http://localhost:8080/wibble/api/foo => ./files/foo.json
# http://localhost:8080/bar/api/foo => ./files/foo.json
# http://localhost:8080/api/foo => No route matches this


`and` can be used to serve any number of routes simultaneously.

";


/// Our application entry point:
fn main() {
    logging::init();
    debug!("Starting");
    if let Err(e) = run() {
        error!("{}", e);
    }
}

/// Run the application, returning early on any synchronous errors:
fn run() -> Result<(), Error> {

    let (routes, other_args) = routes::from_args(env::args().skip(1)).map_err(|e| {
        err!("failed to parse routes: {}", e)
    })?;

    let _ = App::new("weave")
        .author("James Wilson <james@jsdw.me>")
        .about("A lightweight HTTP router and file server.")
        .version("0.2")
        .after_help(EXAMPLES)
        .usage("weave SOURCE to DEST [and SOURCE to DEST ...]")
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
    let mut map = HashMap::new();
    for route in routes {
        let socket_addr = route.src_socket_addr()?;
        let rs: &mut Vec<Route> = map.entry(socket_addr).or_default();
        rs.push(route);
    }

    // Spawn a server on each SocketAddr to handle incoming requests:
    let server = async {
        for (socket_addr, routes) in map {
            let handler = handle_requests(socket_addr, routes)
                .unit_error()
                .boxed()
                .compat();
            rt::spawn(handler);
        }
        Ok(())
    };

    // Kick off these async things:
    hyper::rt::run(server.boxed().compat());
    Ok(())
}

/// Handle incoming requests by matching on routes and dispatching as necessary
async fn handle_requests(socket_addr: SocketAddr, routes: Vec<Route>) {

    let socket_addr_outer = socket_addr.clone();

    let matcher = Arc::new(Matcher::new(routes));
    let socket_addr = Arc::new(socket_addr);

    let make_service = move || {
        let socket_addr = Arc::clone(&socket_addr);
        let matcher = Arc::clone(&matcher);

        service_fn(move |req| {
            let socket_addr = Arc::clone(&socket_addr);
            let matcher = Arc::clone(&matcher);
            let handler = async {
                let res_fut = handle_request(req, socket_addr, matcher);
                // We don't return any errors, so need to tell Rust
                // what the error type would be:
                Result::<_,Error>::Ok(res_fut.await)
            };
            handler.boxed().compat()
        })
    };

    let server = Server::bind(&socket_addr_outer)
        .serve(make_service)
        .compat();

    if let Err(e) = server.await {
        error!("{}", e);
    }
}

/// Handle a single request, given a matcher that defines how to map from input to output:
async fn handle_request<'a>(req: Request<Body>, socket_addr: Arc<SocketAddr>, matcher: Arc<Matcher>) -> Response<Body> {
    let before_time = std::time::Instant::now();
    let src_path = format!("{}{}", socket_addr, req.uri());
    let dest_path = matcher.resolve(req.uri());

    match dest_path {
        None => {
            let duration = before_time.elapsed();
            let not_found_string = format!("[no matching routes] {} in {:#?}", src_path, duration);
            warn!("{}", Red.paint(not_found_string));
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
                    let status_col =
                        if status_code >= 200 && status_code < 300 { Green }
                        else if status_code >= 300 && status_code < 400 { Yellow }
                        else { Red };

                    let info_string = format!("[{}] {} to {} in {:#?}",
                        resp.status().as_str(),
                        src_path,
                        dest_path.to_string(),
                        duration);
                    info!("{}", status_col.paint(info_string));
                    resp
                },
                Err(err) => {
                    let duration = before_time.elapsed();
                    let error_string = format!("[500] {} to {} ({}) in {:#?}",
                        src_path,
                        dest_path.to_string(),
                        err,
                        duration);
                    warn!("{}", Red.paint(error_string));
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
            // Supoprt HTTPS (8 DNS worker threads):
            let https = HttpsConnector::new(8)?;
            // Proxy the request through and pass back the response:
            let response = Client::builder()
                .build(https)
                .request(req)
                .compat()
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
                mime = Some(mime_guess::guess_mime_type(&p));
                file = fs::read(p).compat().map_err(|e| err!("{}", e)).await;
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