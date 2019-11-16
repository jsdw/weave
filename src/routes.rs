use std::net::{ SocketAddr };
use crate::errors::{ Error };
use crate::location::{ SrcLocation, DestLocation, Protocol };

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

        // CLI options (-f, --foo) must come after route spec. When we see
        // something that looks like a CLI option, abandon route building and
        // return the rest of the args (including this one):
        if peeked.starts_with("-") {
            break
        }

        let peeked = peeked.to_owned();
        if let Ok(src) = SrcLocation::parse(peeked.clone()) {

            // we've parsed more:
            expects_more = false;

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
                DestLocation::parse(&dest, &src).map_err(|e| {
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
    fn dest_url (u: &str) -> DestLocation { DestLocation::parse(u).unwrap() }
    fn src_url (u: &str) -> SrcLocation { u.parse().unwrap() }

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
                        src: src_url("http://localhost:8080/"),
                        dest: dest_url("http://localhost:9090")
                    }
                ],
                0
            ),
            (
                vec![s("8080/foo/bar"), s("to"), s("9090/foo"), s("more"), s("args")],
                vec![
                    Route {
                        src: src_url("http://localhost:8080/foo/bar"),
                        dest: dest_url("http://localhost:9090/foo")
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
                        src: src_url("http://localhost:8080/foo/bar"),
                        dest: dest_url("http://localhost:9090/foo")
                    },
                    Route {
                        src: src_url("http://localhost:9091/"),
                        dest: dest_url("http://localhost:9090/lark")
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