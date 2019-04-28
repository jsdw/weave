#[macro_use] mod errors;
mod routes;

use structopt::{ StructOpt };
use errors::{ Error };
use std::env;

// Commands will look like:
//
// weave 8080 to foo.com/bar and 9090 to /lark --something 2 --help

#[derive(StructOpt, Debug)]
#[structopt(name = "weave", about = "A small and simple CLI router")]
/// Usage: weave [src] to [dest]
struct Opts {
    /// Enable verbose logging
    #[structopt(short = "v", long = "verbose")]
    verbose: bool
}

fn main() -> Result<(), Error> {

    let (routes, other_args) = routes::from_args(env::args().skip(1)).map_err(|e| {
        err!("Failed to parse routes: {}", e)
    })?;

    let opts = Opts::from_iter(other_args);

    println!("routes: {:?}, opts: {:?}", routes, opts);

    Ok(())
}
