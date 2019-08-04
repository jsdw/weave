use url::Host;
use regex::Regex;
use lazy_static::lazy_static;
use std::str::FromStr;
use std::path::{ self, PathBuf };
use std::fmt;
use std::net::{ SocketAddr, ToSocketAddrs };
use std::borrow::Cow;
use crate::errors::{ Error };

/// A source location. It should be something that looks a little
/// like a URL, so that we know what interface and port to listen on, and
/// what path to match on incoming requests if any.
#[derive(Debug,Clone)]
pub struct SrcLocation {
    /// The original location:
    original: String,
    /// Host:
    host: Host<String>,
    /// Port:
    port: u16,
    /// Match on paths using this regex:
    path_regex: Regex,
    /// Do we want this to be for exact matches only?
    exact: bool,
    /// Does this path have patterns in?
    has_patterns: bool
}

impl SrcLocation {
    pub fn parse(original: String) -> Result<SrcLocation, Error> {

        lazy_static!{
            // Are we matching on parts of the path? (.*?) is a non greedy match, to match as little
            // as possible, which is necessary to support multiple match patterns.
            static ref HOST_AND_PORT_RE: Regex = Regex::new(r"^(.*):([0-9]+)$").expect("host_and_port_re");
        }

        let input: &str = &*original;

        let (exact, input) = if input.starts_with('=') {
            (true, &input[1..])
        } else {
            (false, input)
        };

        let (protocol, input) = if let Some(n) = input.find("://") {
            (&input[0..n], &input[n+3..])
        } else {
            ("http", input)
        };

        if protocol != "http" {
            return Err(err!("Incalid protocol: expected 'http'"))
        }

        let (host_and_port, input) = if let Some(n) = input.find("/") {
            (&input[0..n], &input[n..])
        } else {
            (input, "")
        };

        let (host, port) = if let Some(caps) = HOST_AND_PORT_RE.captures(host_and_port) {
            let mut host = caps.get(1).unwrap().as_str();
            if host.is_empty() { host = "localhost" }
            let port = caps.get(2).unwrap().as_str().parse().unwrap();
            (host, port)
        } else {
            (input, 80)
        };

        let host = Host::parse(host)?;
        let (has_patterns, path_regex) = convert_path_to_regex(input, exact);

        Ok(SrcLocation {
            original,
            host,
            port,
            path_regex,
            exact,
            has_patterns
        })
    }
    pub fn is_exact(&self) -> bool {
        self.exact
    }
    pub fn has_patterns(&self) -> bool {
        self.has_patterns
    }
    pub fn path_regex(&self) -> &Regex {
        &self.path_regex
    }
    pub fn path_len(&self) -> usize {
        self.path_regex.as_str().len()
    }
    pub fn to_socket_addr(&self) -> Result<SocketAddr, Error> {
        match self.host {
            Host::Ipv4(addr) => Ok(SocketAddr::from((addr,self.port))),
            Host::Ipv6(addr) => Ok(SocketAddr::from((addr,self.port))),
            Host::Domain(ref s) => {
                // This does a potentially blocking lookup.
                let mut addrs = s.to_socket_addrs().map_err(|e| {
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
}

impl PartialEq for SrcLocation {
    fn eq(&self, other: &Self) -> bool {
        self.original == other.original
    }
}

impl FromStr for SrcLocation {
    type Err = Error;
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        SrcLocation::parse(input.to_owned())
    }
}

impl fmt::Display for SrcLocation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.original.fmt(f)
    }
}

/// Convert a path into something that matches incoming paths, and return
/// whether or not any pattern matching is used at all.
fn convert_path_to_regex(path: &str, exact: bool) -> (bool,Regex) {
    lazy_static!{
        // Are we matching on parts of the path? (.*?) is a non greedy match, to match as little
        // as possible, which is necessary to support multiple match patterns.
        static ref MATCH_POINT_RE: Regex = Regex::new(r"(.*?)\(([a-zA-Z][a-zA-Z0-9_-]*)(\.\.)?\)").expect("match_point_re");
    }

    let mut has_matches = false;
    let mut re_expr: String = String::new();
    let mut last_idx = 0;

    // Assemble a regex string if we find matchers:
    for cap in MATCH_POINT_RE.captures_iter(path) {
        has_matches = true;
        last_idx = cap.get(0).unwrap().end();

        let raw = cap.get(1).unwrap().as_str();
        let match_name = cap.get(2).unwrap().as_str();
        let match_all = cap.get(3);

        re_expr.push_str(&regex::escape(raw));

        if match_all.is_some() {
            // If '..' put after name, non-greedily match as much as we
            // can (but allow subsequent captures to capture their bits too):
            re_expr.push_str(&format!("(?P<{}>.+?)", match_name));
        } else {
            // Else, match everything to the next '/':
            re_expr.push_str(&format!("(?P<{}>[^/]+)", match_name));
        }
    }

    // push end of string onto regex:
    re_expr.push_str(&regex::escape(&path[last_idx..]));

    // Allow trailing chars if not exact, else prohibit:
    let regex_string = if exact {
        format!("^{}$", re_expr)
    } else {
        format!("^{}", re_expr)
    };

    (has_matches, Regex::new(&regex_string).expect("invalid convert regex built up"))
}
