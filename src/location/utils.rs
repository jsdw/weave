use lazy_static::lazy_static;
use regex::Regex;
use url::Host;
use std::borrow::Cow;
use std::str::FromStr;
use std::fmt;
use std::net::{ SocketAddr, ToSocketAddrs };
use crate::errors::{ Error };

/// Take something that looks a little like a URL and
/// find each part of the URL.
pub struct SplitUrl<'a> {
    pub protocol: Option<Protocol>,
    pub host: Host<String>,
    pub port: Option<u16>,
    pub path: Cow<'a, str>,
    pub query: &'a str
}

impl SplitUrl<'_> {
    pub fn parse<'a>(input: &'a str) -> Result<SplitUrl<'a>, Error> {
        lazy_static!{
            // Are we matching on parts of the path? (.*?) is a non greedy match, to match as little
            // as possible, which is necessary to support multiple match patterns.
            static ref HOST_AND_PORT_RE: Regex = Regex::new(r"^(.*):([0-9]+)$").expect("host_and_port_re");
        }

        // Did we specify an input protocol? It should be nothing or one of http,https,tcp
        let (protocol, input) = if let Some(n) = input.find("://") {
            let protocol = input[0..n].parse()?;
            (Some(protocol), &input[n+3..])
        } else {
            (None, input)
        };

        //  Let's find the host:port bit of the input..
        let (host_and_port, input) = if let Some(n) = input.find("/") {
            (&input[0..n], &input[n..])
        } else {
            (input, "")
        };

        // And then turn that into a host string and port number
        let (host, port) = if let Some(caps) = HOST_AND_PORT_RE.captures(host_and_port) {
            let host = caps.get(1).unwrap().as_str();
            let port = caps.get(2).unwrap().as_str().parse().unwrap();
            (host, Some(port))
        } else if let Ok(n) = host_and_port.parse() {
            ("localhost", Some(n))
        } else {
            (host_and_port, None)
        };

        // Host default to localhost if not provided:
        let host = Host::parse(if host.is_empty() { "localhost" } else { host })?;

        // Split remaining input into path and query parts:
        let (raw_path, query) = split_path_and_query(&input);

        // Normalise path if needed by adding prefix /:
        let path = if input.starts_with("/") {
            Cow::from(raw_path)
        } else {
            Cow::from(format!("/{}", raw_path))
        };

        Ok(SplitUrl {
            protocol,
            host,
            port,
            path,
            query
        })
    }
}

/// Split path_and_query into separate path and query pieces
fn split_path_and_query(path_and_query: &str) -> (&str, &str) {
    if let Some(idx) = path_and_query.find('?') {
        (&path_and_query[0..idx], &path_and_query[idx+1..])
    } else {
        (path_and_query, "")
    }
}

// Try to convert a Host and prot into a SocketAddr
pub fn to_socket_addr(host: &Host, port: u16) -> Result<SocketAddr, Error> {
    match host {
        Host::Ipv4(addr) => Ok(SocketAddr::from((*addr,port))),
        Host::Ipv6(addr) => Ok(SocketAddr::from((*addr,port))),
        Host::Domain(ref s) => {
            // This does a potentially blocking lookup.
            let mut addrs = (&**s, port).to_socket_addrs().map_err(|e| {
                err!("Cannot parse socket address to listen on: {}", e)
            })?;

            if let Some(addr) = addrs.next() {
                Ok(addr)
            } else {
                Err(err!("Cannot parse socket address to listen on"))
            }
        }
    }
}

/// The protocol that is being used
#[derive(Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub enum Protocol {
    Http,
    Https,
    Tcp
}

impl FromStr for Protocol {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.eq_ignore_ascii_case("http") {
            Ok(Protocol::Http)
        } else if s.eq_ignore_ascii_case("https") {
            Ok(Protocol::Https)
        } else if s.eq_ignore_ascii_case("tcp") {
            Ok(Protocol::Tcp)
        } else {
            Err(err!("{} is not a supported protocol", s))
        }
    }
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Protocol::Http => "http",
            Protocol::Https => "https",
            Protocol::Tcp => "tcp"
        })
    }
}