#![feature(futures_api, async_await, await_macro)]

#[macro_use] mod errors;
mod routes;

use std::env;
use clap::{ App, AppSettings };
use errors::{ Error };
use hyper::{Body, Response, Server};
use hyper::service::service_fn_ok;
use ansi_term::Color::Red;

// 0.3 futures and a compat layer to bridge to 0.1:
use futures::future::{ FutureExt, TryFutureExt };
use futures::compat::*;

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

    println!("routes: {:?}", routes);

    // Go through routes. Try to convert each to a SockeetAddr. make map from SOURCE SocketAddr to
    // Route (to + from part). Start a hyper server for each SocketAddr. When request comes in to a
    // given SocketAddr, rty to match to source paths provided (sorted longest first). Proxy matches
    // to destination as appropriate. use something like hyper-staticfile for static file handling.

    // Construct our SocketAddr to listen on...
    let addr = ([127, 0, 0, 1], 3000).into();

    // And a MakeService to handle each connection...
    let make_service = || {
        service_fn_ok(|_req| {
            Response::new(Body::from("Hello World"))
        })
    };

    // Then bind and serve...
    let server = Server::bind(&addr)
        .serve(make_service)
        .compat();

    let fut = async {
        if let Err(e) = await!(server) {
            eprintln!("Error running server: {}", e);
        }

        Ok(())
    };

    // Finally, spawn `server` onto an Executor...
    hyper::rt::run(fut.boxed().compat());
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{}: {}", Red.paint("error"), e);
    }
}