use url::Url;
use std::str::FromStr;
use std::path::PathBuf;
use std::net::{ SocketAddr, ToSocketAddrs };
use std::fmt;
use crate::errors::{ Error };

/// Take some args and hand back a vector of Routes we've parsed out of them,
/// plus an Iterator of unused args:
pub fn from_args<I: IntoIterator<Item=String>>(args: I) -> Result<(Vec<Route>, impl Iterator<Item=String>), Error> {
    let mut routes = vec![];

    // If the first arg is a Location, expect the next two args to be
    // 'to' and another Location. Each time the subsequent arg is 'and',
    // look for the same again.
    let mut args = args.into_iter().peekable();
    let mut expects_more = false;
    while let Some(peeked) = args.peek() {
        let peeked = peeked.clone();
        if let Ok(src) = Location::parse(&peeked) {

            // we've parsed more:
            expects_more = false;

            // `src` needs to be a Url, not a FilePath
            let src = match src {
                Location::Url(url) => url,
                Location::FilePath(path) => {
                    return Err(err!("The location {} is not a valid route", path.to_string_lossy()))
                }
            };

            // Next arg is valid Location (we peeked), so assume
            // 'loc to loc' triplet and err if not.
            args.next();

            // The next arg after 'src' loc should be the word 'to'.
            // If it's not, hand back an error:
            let next_is_joiner = if let Some(joiner) = args.next() {
                if joiner.trim() != "to" {
                    false
                } else {
                    true
                }
            } else {
                false
            };
            if !next_is_joiner {
                return Err(err!("Expecting the word 'to' after the location '{}'", peeked));
            }

            // The arg following the 'to' should be another location
            // or something is wrong:
            let dest = if let Some(dest) = args.next() {
                Location::parse(&dest).map_err(|e| {
                    err!("Error parsing '{}': {}", dest, e)
                })
            } else {
                Err(err!("Expecting a destination location to be provided after '{} to'", peeked))
            }?;

            // If we've made it this far, we have a Route:
            routes.push(Route {
                src,
                dest
            });

            // Now, we either break or the next arg is 'and':
            let next_is_and = if let Some(and) = args.peek() {
                if and.trim() != "and" {
                    false
                } else {
                    true
                }
            } else {
                false
            };
            if !next_is_and {
                break
            } else {
                // We expect another valid route now:
                expects_more = true;
                // consume the 'and' if we see it:
                args.next();
            }

        } else {
            // No more Route-like args so break out of this loop:
            break
        }
    }

    // we've seen an 'and', but then failed to parse a location:
    if expects_more {
        return Err(err!("'and' not followed by a subsequent route"));
    }

    // Hand back our routes, plus the rest of the args
    // that we haven't iterated yet, if things were
    // successful:
    Ok(( routes, args ))
}

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct Route {
    pub src: Url,
    pub dest: Location
}

impl Route {
    pub fn src_socket_addr(&self) -> Result<SocketAddr, Error> {
        let mut addrs = self.src.to_socket_addrs().map_err(|e| {
            err!("Cannot parse socket address to listen on: {}", e)
        })?;

        if let Some(addr) = addrs.next() {
            Ok(addr)
        } else {
            Err(err!("Cannot parse socket address to listen on"))
        }
    }
}


#[derive(Debug,Clone,PartialEq,Eq)]
pub enum Location {
    Url(Url),
    FilePath(PathBuf)
}

impl Location {
    fn parse(input: impl AsRef<str>) -> Result<Location, Error> {
        let mut s = input.as_ref().trim().to_owned();

        // Starts with a '.' or '/', so will assume it's a filepath:
        if [Some('.'), Some('/')].contains(&s.chars().next()) {
            return Ok(Location::FilePath(s.into()));
        }

        // From here on, we assume that something like a URL
        // has been provided..

        // Starts with a port (eg `8080/foo/bar` or `:8080`)?
        // Add a host:
        let port_bit = {
            let no_path = if let Some(idx) = s.find('/') {
                &s[..idx]
            } else {
                &s
            };
            if no_path.starts_with(":") {
                &no_path[1..]
            } else {
                no_path
            }
        };
        if let Ok(_) = port_bit.parse::<u16>() {
            let mut new_s = "localhost:".to_owned();
            new_s.push_str(if s.starts_with(":") { &s[1..] } else { &s });
            s = new_s;
        }

        // Doesn't have a scheme? Add one.
        if let None = s.find("://") {
            let mut new_s = "http://".to_owned();
            new_s.push_str(&s);
            s = new_s;
        }

        // Now, attempt to parse to a URL:
        let url: Url = if let Ok(url) = s.parse() {
            Ok(url)
        } else {
            Err(err!("Not a valid URL"))
        }?;

        // Complain if the URL scheme is wrong:
        if !["http", "https"].contains(&url.scheme()) {
            return Err(err!("Invalid scheme, expecting http or https only"));
        }

        Ok(Location::Url(url))
    }
}

impl FromStr for Location {
    type Err = Error;
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Location::parse(input)
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Location::Url(url) => url.fmt(f),
            Location::FilePath(path) => path.to_string_lossy().fmt(f)
        }
    }
}

#[cfg(test)]
mod test {

    use super::*;

    fn s (s: &str) -> String { s.to_owned() }
    fn u (u: &str) -> Location { Location::Url(Url::from_str(u).unwrap()) }

    #[test]
    fn location_can_parse_valid_urls() {

        let urls = vec![
            // Absolute filepaths are ok:
            ("/foo/bar", Location::FilePath("/foo/bar".into())),
            // Relative filepaths are ok:
            ("./foo/bar", Location::FilePath("./foo/bar".into())),
            // More Relative filepaths are ok:
            ("../foo/bar", Location::FilePath("../foo/bar".into())),
            // Just a port is OK
            ("8080", u("http://localhost:8080/")),
            // A port and path is OK
            ("8080/foo/bar", u("http://localhost:8080/foo/bar")),
            // Just a colon + port is OK
            (":8080", u("http://localhost:8080/")),
            // A colon + port and path is OK
            (":8080/foo/bar", u("http://localhost:8080/foo/bar")),
            // localhost is OK
            ("localhost", u("http://localhost/")),
            // localhost + port is ok:
            ("http://localhost:8080", u("http://localhost:8080/")),
            // IP + parth is ok:
            ("http://127.0.0.1/foo", u("http://127.0.0.1/foo")),
            // can parse IP:
            ("http://127.0.0.1:8080/foo", u("http://127.0.0.1:8080/foo")),
            // default scheme, valid IP addr:
            ("127.0.0.1", u("http://127.0.0.1/")),
            // IP + port parses:
            ("127.0.0.1:8080", u("http://127.0.0.1:8080/")),
            // A standard hostname missing port and scheme:
            ("example.com", u("http://example.com/")),
            // Spaces either side will be ignored:
            ("  \t example.com\t \t", u("http://example.com/"))
        ];

        for (actual, expected) in urls {
            let actual_loc: Result<Location, _> = actual.parse();
            assert!(actual_loc.is_ok(), "Location could not be parsed: '{}', result: {:?}", actual, actual_loc);
            assert_eq!(actual_loc.unwrap(), expected, "(Original was '{}')", actual);
        }
    }

    #[test]
    fn location_wont_parse_invalid_urls() {
        let urls = vec![
            // Don't know this protocol:
            "foobar://example.com"
        ];

        for actual in urls {
            let actual_loc: Result<Location, _> = actual.parse();
            assert!(actual_loc.is_err(), "This invalid location should not have successfully parsed: {}", actual);
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
                    Route {
                        from: u("http://localhost:8080/"),
                        to: u("http://localhost:9090")
                    }
                ],
                0
            ),
            (
                vec![s("8080/foo/bar"), s("to"), s("9090/foo"), s("more"), s("args")],
                vec![
                    Route {
                        from: u("http://localhost:8080/foo/bar"),
                        to: u("http://localhost:9090/foo")
                    }
                ],
                2
            ),
            (
                vec![s("8080/foo/bar"), s("to"), s("9090/foo"), s("and"),
                     s("9091"), s("to"), s("9090/lark"),
                     s("more"), s("args")],
                vec![
                    Route {
                        from: u("http://localhost:8080/foo/bar"),
                        to: u("http://localhost:9090/foo")
                    },
                    Route {
                        from: u("http://localhost:9091/"),
                        to: u("http://localhost:9090/lark")
                    }
                ],
                2
            ),
        ];

        for (a,b,left) in routes {
            match from_args(a.clone()) {
                Err(e) => panic!("Could not parse {:?}: {}", a, e),
                Ok(r) => {
                    assert_eq!(r.0, b, "Unexpected parse of {:?}: {:?}", a, r.0);
                    let actual_left = r.1.count();
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
            vec![s("9090"), s("to"), s("--option")],
            vec![s("9090"), s("to"), s("9091"), s("and")],
            vec![s("9090"), s("to"), s("9091"), s("and"), s("--option")],
        ];
        for r in bad_routes {
            let parsed = from_args(r.clone());
            assert!(
                parsed.is_err(),
                "Args {:?} should not successfully parse, but parsed to {:?}",
                r, parsed.map(|(a,b)| (a, b.collect::<Vec<_>>()))
            );
        }
    }

}