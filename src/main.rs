#![feature(async_await, await_macro)]

#[macro_use] mod errors;
mod routes;
mod matcher;

use std::env;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use clap::{ App, AppSettings };
use hyper::{Client, Body, Request, Response, Server, rt};
use hyper::service::service_fn;
use hyper_tls::HttpsConnector;
use ansi_term::Color::Red;

// 0.3 futures and a compat layer to bridge to 0.1:
use futures::future::{ FutureExt, TryFutureExt };
use futures::compat::*;

use routes::{ Route, Location };
use matcher::Matcher;
use errors::{ Error };

static EXAMPLES: &str = "EXAMPLES:

weave 8080 to foo.com

This forwards HTTP traffic destined for localhost:8080 to foo.com.

weave 192.168.0.222:8080 to foo.com:9090

This forward HTTP traffic destined for your local interface 192.168.0.222
(port 8080) to foo.com:9090.

weave 8080/bar to ./foo/bar and 8080/foo to foo.com:8080

This serves the content on your local filesystem at ./foo/bar on
localhost:8080/bar, and content on foo.com:8080 on localhost:8080/foo.

";

fn run() -> Result<(), Error> {

    let (routes, other_args) = routes::from_args(env::args().skip(1)).map_err(|e| {
        err!("failed to parse routes: {}", e)
    })?;

    let _ = App::new("weave")
        .author("James Wilson <james@jsdw.me>")
        .about("A lightweight HTTP router and file server.")
        .version("0.1")
        .after_help(EXAMPLES)
        .usage("weave SOURCE to DEST [and SOURCE to DEST ...]")
        .setting(AppSettings::NoBinaryName)
        .get_matches_from(other_args);

    if routes.is_empty() {
        return Err(err!("No routes have been provided. Use -h or --help for more information"));
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
                let res = await!(res_fut);
                if let Err(e) = &res {
                    eprintln!("Error: {:?}", e);
                }
                res
            };
            handler.boxed().compat()
        })
    };

    let server = Server::bind(&socket_addr_outer)
        .serve(make_service)
        .compat();

    if let Err(e) = await!(server) {
        eprintln!("{}: {}", Red.paint("error"), e);
    }
}

async fn handle_request<'a>(req: Request<Body>, socket_addr: Arc<SocketAddr>, matcher: Arc<Matcher>) -> Result<Response<Body>, Error> {
    let mut req = req;
    let loc = matcher.resolve(req.uri());

    match loc {
        // Proxy to the URI our request matched against:
        Some(Location::Url(url)) => {
            // Set the request URI to our new destination:
            *req.uri_mut() = format!("{}", url).parse().unwrap();
            // Remove the host header (it's set according to URI if not present):
            req.headers_mut().remove("host");
            // Supoprt HTTPS (8 DNS worker threads):
            let https = HttpsConnector::new(8)?;
            // Proxy the request through and pass back the response:
            let response = await!(Client::builder().build(https).request(req).compat())?;
            Ok(response)
        },
        // Proxy to the filesystem:
        Some(Location::FilePath(path)) => {
            Ok(Response::new(Body::from("hello world")))
        },
        // 404, not found!
        None => {
            Ok(Response::new(Body::from("hello world")))
        }
    }
}

/// Catch any errors from running and report them back:
fn main() {
    if let Err(e) = run() {
        eprintln!("{}: {}", Red.paint("error"), e);
    }
}