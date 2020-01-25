use std::net::{ SocketAddr };
use crate::errors::{ Error };
use crate::location::{ SrcLocation, DestLocation, Protocol };

/// Take some args and hand back a vector of Routes we've parsed out of them,
/// plus an Iterator of unused args:
pub fn from_args(args: &[String]) -> Result<(Vec<Route>, &[String]), Error> {

    // Split args we care about apart from CLI opts starting with '-':
    let (args, rest) = args.iter()
        .enumerate()
        .find(|(_,arg)| arg.starts_with("-"))
        .map_or_else(|| (args, &[][..]), |(n,_)| args.split_at(n));

    // The last argument shouldn't be "and":
    if args.last().map_or(false, |l| l == "and") {
        return Err(err!("'and' not followed by a subsequent route"));
    }

    // Iterate the potential route args to build routes.
    let mut routes = vec![];
    let mut idx = 0;
    let mut and_comes_next = false;
    static NOTHING: &str = "nothing";
    while idx < args.len() {

        // We've parsed at least one route, and expect to see
        // 'and' if we have mroe routes to parse now.
        if and_comes_next {
            let and_str = &*args[idx];
            if and_str != "and" {
                return Err(err!("I expect to see 'and' expected between routes, but I was given '{}' instead", and_str));
            }
            idx += 1;
        }
        // Every subsequent route will require
        // "and" separating it from the last one:
        and_comes_next = true;

        let arg = &args[idx];

        // "nothing" can take the place of an entire route:
        if arg == NOTHING {
            // ignore a trailing "nothing"
            if idx == args.len() - 1 {
                idx += 1;
                continue
            }
            // ignore "nothing and" (the "and" bit is handled on the next loop)
            if &*args[idx+1] == "and" {
                idx += 1;
                continue
            }
            // Specifically, we don't ignore "nothing to _",
            // since that is handled later.
        }

        // Make sure we have enough args to form a route.
        if idx + 1 >= args.len() {
            return Err(err!("Not enough args provided; routes of the form \
                            '[src] to [dest]' are expected, but I was given '{}'", arg));
        }
        if idx + 2 >= args.len() {
            return Err(err!("Not enough args provided; routes of the form \
                            '[src] to [dest]' are expected, but I was given '{} {}'", arg, args[idx+1]));
        }

        let src_str = arg;
        let to_str = &*args[idx+1];
        let dest_str = &*args[idx+2];
        idx += 3;

        // Parse the source location:
        let src = match SrcLocation::parse(src_str.clone()) {
            Ok(src) => src,
            Err(e) => { return Err(err!("'{}' is not a valid source location: {}", src_str, e)) }
        };

        // Expect "to" to separate src and dest:
        if to_str != "to" {
            return Err(err!("'{}' should be followed by 'to' and \
                             then a destination location", src_str))
        }

        // Parse the dest location:
        let dest = match DestLocation::parse(dest_str, &src) {
            Ok(dest) => dest,
            Err(e) => { return Err(err!("'{}' is not a valid destination location: {}", dest_str, e)) }
        };

        // Push these to a new route:
        routes.push(Route {
            src,
            dest
        });

    }

    // Hand back our routes, plus the rest of the args that we haven't iterated yet,
    // if things were successful:
    Ok(( routes, rest ))
}

#[derive(Debug,Clone,PartialEq)]
pub struct Route {
    pub src: SrcLocation,
    pub dest: DestLocation
}

impl Route {
    pub fn protocol(&self) -> Protocol {
        self.src.protocol()
    }
    pub fn src_socket_addr(&self) -> Result<SocketAddr, Error> {
        self.src.to_socket_addr()
    }
    /// TCP destinations have a socket address we can
    /// talk to them on. HTTP(s) destinations do not.
    pub fn dest_socket_addr(&self) -> Option<SocketAddr> {
        self.dest.socket_addr()
    }
}

#[cfg(test)]
mod test {

    use super::*;

    fn s (s: &str) -> String { s.to_owned() }
    fn route(src: &str, dest: &str) -> Route {
        let src: SrcLocation = src.parse().unwrap();
        Route {
            src: src.clone(),
            dest: DestLocation::parse(dest, &src).unwrap()
        }
    }

    #[test]
    fn routes_can_be_parsed() {

        let routes = vec![
            (
                vec![],
                vec![],
                0
            ),
            (
                vec![s("--other")],
                vec![],
                1
            ),
            (
                vec![s("8080"), s("to"), s("9090")],
                vec![
                    route("http://localhost:8080/", "http://localhost:9090")
                ],
                0
            ),
            // We can use "nothing" alone but it is a noop
            (
                vec![s("nothing")],
                vec![],
                0
            ),
            // We can use "nothing" alone but it is a noop
            (
                vec![s("nothing"), s("-arg")],
                vec![],
                1
            ),
            // We can use "nothing" in place of an entire route to allow slightly
            // easier programmatic route providing:
            (
                vec![s("nothing"), s("and"), s("8080"), s("to"), s("9090")],
                vec![
                    route("http://localhost:8080/", "http://localhost:9090")
                ],
                0
            ),
            // We can use "nothing" at the end as well:
            (
                vec![s("8080"), s("to"), s("9090"), s("and"), s("nothing")],
                vec![
                    route("http://localhost:8080/", "http://localhost:9090")
                ],
                0
            ),
            // We can use "nothing" in the middle as well:
            (
                vec![
                    s("8080"), s("to"), s("9090"), s("and"),
                    s("nothing"), s("and"),
                    s("8081"), s("to"), s("9091"), s("and"),
                    s("nothing"), s("and"),
                    s("nothing"), s("and"),
                    s("8082"), s("to"), s("9092"),
                ],
                vec![
                    route("http://localhost:8080/", "http://localhost:9090"),
                    route("http://localhost:8081/", "http://localhost:9091"),
                    route("http://localhost:8082/", "http://localhost:9092"),
                ],
                0
            ),
            // "nothing" can take the place of a destination (it'll return 404):
            (
                vec![
                    s("8081"), s("to"), s("nothing"), s("and"),
                    s("8082"), s("to"), s("statuscode://403")
                ],
                vec![
                    route("http://localhost:8081/", "statuscode://404"),
                    route("http://localhost:8082/", "statuscode://403"),
                ],
                0
            ),
            (
                vec![s("8080/foo/bar"), s("to"), s("9090/foo"), s("--more"), s("args")],
                vec![
                    route("http://localhost:8080/foo/bar", "http://localhost:9090/foo")
                ],
                2
            ),
            (
                vec![s("8080/foo/bar"), s("to"), s("9090/foo"), s("and"),
                     s("9091"), s("to"), s("9090/lark"),
                     s("-more"), s("args")],
                vec![
                    route("http://localhost:8080/foo/bar", "http://localhost:9090/foo"),
                    route("http://localhost:9091/", "http://localhost:9090/lark")
                ],
                2
            ),
        ];

        for (a,b,left) in routes {
            match from_args(&a) {
                Err(e) => panic!("Could not parse {:?}: {}", a, e),
                Ok(r) => {
                    assert_eq!(r.0, b, "Unexpected parse of {:?}: {:?}", a, r.0);
                    let actual_left = r.1.len();
                    assert_eq!(actual_left, left, "Wrong number of remaining for {:?}; expected {}, got {}", a, left, actual_left);
                }
            }
        }
    }

    #[test]
    fn routes_cant_be_parsed() {
        let bad_routes = vec![
            vec![s("9090")],
            vec![s("9090"), s("to")],
            vec![s("9090"), s("to"), s("9091"), s("and")],
            vec![s("9090"), s("to"), s("9091"), s("and"), s("--option")],
        ];
        for r in bad_routes {
            let parsed = from_args(&r);
            assert!(
                parsed.is_err(),
                "Args {:?} should not successfully parse, but parsed to {:?}",
                r, parsed.map(|(a,b)| (a, b))
            );
        }
    }

}