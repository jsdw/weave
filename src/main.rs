#![feature(async_await, await_macro)]

#[macro_use] mod errors;
mod routes;
mod matcher;

use std::env;
use std::collections::HashMap;
use std::net::SocketAddr;
use clap::{ App, AppSettings };
use errors::{ Error };
use hyper::{Body, Response, Server, rt};
use hyper::service::service_fn_ok;
use ansi_term::Color::Red;

// 0.3 futures and a compat layer to bridge to 0.1:
use futures::future::{ FutureExt, TryFutureExt };
use futures::compat::*;

use routes::Route;

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

    let socket_addr2 = socket_addr.clone();
    let make_service = move || {

        let matcher = matcher::Matcher::new(routes.clone());
        let socket_addr = socket_addr2.clone();

        service_fn_ok(move |req| {

            let loc = matcher.resolve(req.uri());

            let msg = format!("{:?}: req: {:?}, loc: {:?}", socket_addr, req, loc);



            Response::new(Body::from(msg))
        })
    };

    if let Err(e) = await!(Server::bind(&socket_addr).serve(make_service).compat()) {
        eprintln!("{}: {}", Red.paint("error"), e);
    }
}

/// Catch any errors from running and report them back:
fn main() {
    if let Err(e) = run() {
        eprintln!("{}: {}", Red.paint("error"), e);
    }
}