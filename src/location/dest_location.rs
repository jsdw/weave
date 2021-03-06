use lazy_static::lazy_static;
use regex::Regex;
use std::path::{ self, PathBuf };
use std::fmt;
use std::borrow::Cow;
use std::net::{ SocketAddr };
use crate::errors::{ Error };
use super::src_location::{ SrcLocation, Matches };
use super::utils::{ Protocol, SplitUrl, to_socket_addr };

/// A Destination location. This is what a request can be rerouted to.
/// On matching, we look at the pair of source and destination locations
/// in order to construct a `ResolvedLocation`, which is where the request
/// will actually be routed to.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct DestLocation(DestLocationInner);

#[derive(Debug,Clone,PartialEq,Eq)]
pub enum DestLocationInner {
    Url{ host_bits: String, path: String, query: String },
    Socket { address: SocketAddr },
    HttpStatusCode { code: hyper::StatusCode },
    FilePath(String)
}

impl DestLocation {
    /// Parse a string into a destination location. The corresponding source location is
    /// required as it may impact what is valid as a destination location.
    pub fn parse(original: impl AsRef<str>, src: &SrcLocation) -> Result<DestLocation, Error> {
        let input = original.as_ref().trim();

        // Starts with a '.' or '/', so will assume it's a filepath:
        if [Some('.'), Some(path::MAIN_SEPARATOR)].contains(&input.chars().next()) {
            return Ok(DestLocation(DestLocationInner::FilePath(input.to_owned())));
        }

        // Else, expect it to look like a URL (this normalises things as well,
        // adding back a protocol/host/port if missing):
        let src_protocol = src.protocol();

        // React based on the source protocol to form a desination location:
        match src_protocol {
            Protocol::Https | Protocol::HttpStatusCode => {
                // This should be checked when parsing the source location and so is probably an error
                // if we get here, but for safety we do the check and return a reasonable message:
                return Err(err!("The source protocol cannot be {} or {}", Protocol::Https, Protocol::HttpStatusCode))
            },
            Protocol::Http => {
                // Is the destination a status code? Try parsing that first.
                if let Some(statuscode_str) = parse_statuscode_str(input) {
                    let code = statuscode_str.parse()?;
                    return Ok(DestLocation(DestLocationInner::HttpStatusCode{ code }))
                }

                // Otherwise, assume that the destination is a valid URL..
                let url = SplitUrl::parse(input)?;
                let dest_protocol = url.protocol.unwrap_or(Protocol::Http);
                if !&[Protocol::Http, Protocol::Https, Protocol::HttpStatusCode].contains(&dest_protocol) {
                    return Err(err!("Given a source protocol of '{}', the destination protocol should be '{}', '{}' or '{}'",
                                    src_protocol, Protocol::Http, Protocol::Https, Protocol::HttpStatusCode))
                }

                let host_bits = if let Some(port) = url.port {
                    format!("{}://{}:{}", dest_protocol, url.host, port)
                } else {
                    format!("{}://{}", dest_protocol, url.host)
                };

                Ok(DestLocation(DestLocationInner::Url{
                    host_bits, path: url.path.into_owned(), query: url.query.to_owned()
                }))
            },
            Protocol::Tcp => {
                let url = SplitUrl::parse(input)?;
                let dest_protocol = url.protocol.unwrap_or(src_protocol);
                if dest_protocol != src_protocol {
                    return Err(err!("The destination protocol should match the source protocol \
                                     of '{}'", src_protocol))
                }
                if url.path != "/" {
                    return Err(err!("The destination cannot have a path when the source protocol \
                                     is '{}'", src_protocol))
                }
                if url.query != "" {
                    return Err(err!("The destination cannot have a query string when the source \
                                     protocol is '{}'", src_protocol))
                }

                // Use the source port if a destination port isn't provided since
                // it's the best hint that we have (and a not-unreasonable one):
                let port = url.port.unwrap_or(src.port());

                let socket_addr = match to_socket_addr(&url.host, port) {
                    Ok(addr) => addr,
                    Err(e) => return Err(e)
                };

                Ok(DestLocation(DestLocationInner::Socket {
                    address: socket_addr
                }))
            }
        }
    }
    /// If the destination location is just a TCP socket address,
    /// We can ask for it here.
    pub fn socket_addr(&self) -> Option<SocketAddr> {
        match &self.0 {
            DestLocationInner::Socket { address } => Some(*address),
            _ => None
        }
    }
    /// Output a resolved location given Matches from a source location.
    pub fn resolve(&self, matches: &Matches) -> ResolvedLocation {
        match &self.0 {
            DestLocationInner::Url{ host_bits, path, query } => {
                // Substitute in matches (to the path+query params):
                let mut path = expand_str_with_matches(matches, &path).into_owned();
                let mut query = expand_str_with_matches(matches, &query).into_owned();

                // Append the rest of the path onto the new URL:
                let path_tail = matches.path_tail();
                if !path_tail.is_empty() {
                    if path.ends_with('/') {
                        path.push_str(path_tail.trim_start_matches('/'));
                    } else {
                        if !path_tail.starts_with('/') { path.push('/'); }
                        path.push_str(path_tail);
                    }
                }

                // Append any query params that don't exist in the dest location already:
                let query_copy = query.clone();
                let current_query: Vec<_> = query_pairs(&query_copy).collect();
                for (key, val) in query_pairs(matches.query()) {
                    if current_query.iter().all(|(k,_)| k != &key) {
                        if !query.is_empty() {
                            query.push('&');
                        }
                        query.push_str(key);
                        if !val.is_empty() {
                            query.push('=');
                            query.push_str(val);
                        }
                    }
                }

                // Put everything together to get our final output URL:
                let url = if query.is_empty() {
                    format!("{}{}", host_bits, path)
                } else {
                    format!("{}{}?{}", host_bits, path, query)
                };
                ResolvedLocation::Url(url)
            },
            DestLocationInner::FilePath(path) => {
                // Substitute in matches (to any part of the path):
                let mut path: PathBuf = expand_str_with_matches(matches, &path).into_owned().into();

                // Append the rest of the path onto the new file path:
                let bits = matches.path_tail().split('/').filter(|s| !s.is_empty());
                let mut appended = 0;
                for bit in bits {
                    // Ignore bits that would do nothing:
                    if bit == "." {
                        continue
                    }
                    // Only allow going up in the path if we've gone down:
                    else if bit == ".." {
                        if appended > 0 {
                            path.pop();
                            appended -= 1;
                        }
                    }
                    // Append ordinary path pieces:
                    else {
                        path.push(bit);
                        appended += 1;
                    }
                }

                ResolvedLocation::FilePath(path)
            },
            DestLocationInner::Socket{ address } => {
                // If we are directed at a socket address, we have no matches to
                // substitute so we just resolve it to a URL, assuming HTTP protocol:
                ResolvedLocation::Url(format!("http://{}", address))
            },
            DestLocationInner::HttpStatusCode{ code } => {
                // Status code destinations just resolve to a code:
                ResolvedLocation::HttpStatusCode(*code)
            }
        }

    }
}

impl fmt::Display for DestLocation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.0 {
            DestLocationInner::Url{ host_bits, path, query } => {
                if query.is_empty() {
                    write!(f, "{}{}", host_bits, path)
                } else {
                    write!(f, "{}{}?{}", host_bits, path, query)
                }
            },
            DestLocationInner::FilePath(path) => {
                path.fmt(f)
            },
            DestLocationInner::HttpStatusCode{ code } => {
                write!(f, "statuscode://{}", code)
            }
            DestLocationInner::Socket { address } => {
                address.fmt(f)
            }
        }
    }
}

/// A ResolvedLocation is the destination of a given request, formed by
/// looking at the source and destination locations provided.
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum ResolvedLocation {
    Url(String),
    HttpStatusCode(hyper::StatusCode),
    FilePath(PathBuf)
}

impl fmt::Display for ResolvedLocation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ResolvedLocation::Url(url) => url.fmt(f),
            ResolvedLocation::FilePath(path) => path.to_string_lossy().fmt(f),
            ResolvedLocation::HttpStatusCode(code) => write!(f, "statuscode://{}", code)
        }
    }
}

/// Given a str and some Matches, return a string with the matches substituted into it.
fn expand_str_with_matches<'a>(matches: &Matches, s: &'a str) -> Cow<'a,str> {
    lazy_static!{
        // Are we matching on parts of the path?
        static ref MATCH_NAME_RE: Regex = Regex::new(r"\(([a-zA-Z][a-zA-Z0-9_-]*)\)").expect("match_point_re");
    }

    // @TODO: Figure out lifetimes to avoid returning owned strings in closure:
    MATCH_NAME_RE.replace_all(s, |cap: &regex::Captures| -> String {
        let replace_name = cap.get(1).unwrap().as_str();
        if let Some(replacement) = matches.get(replace_name) {
            replacement.to_owned()
        } else {
            cap.get(0).unwrap().as_str().to_owned()
        }
    })
}

/// Given a query fragment, return pairs of query params.
fn query_pairs<'a>(query: &'a str) -> impl Iterator<Item=(&'a str, &'a str)> {
    query.split('&').filter(|part| !part.is_empty()).map(|part| {
        if let Some(mid) = part.find('=') {
            (&part[0..mid],&part[mid+1..])
        } else {
            (part, "")
        }
    })
}

/// Match a statuscode://123 or "nothing" input:
fn parse_statuscode_str(s: &str) -> Option<&str> {
    if s == "nothing" {
        return Some("404")
    }
    static START: &str = "statuscode://";
    if s.starts_with(START) {
        Some(&s[START.len()..])
    } else {
        None
    }
}

#[cfg(test)]
mod test {

    use super::*;

    fn u (u: &str) -> DestLocation {
        let src: SrcLocation = "http://localhost:1234".parse().unwrap();
        DestLocation::parse(u, &src).unwrap()
    }
    fn code (n: u16) -> DestLocation {
        DestLocation(DestLocationInner::HttpStatusCode{ code: hyper::StatusCode::from_u16(n).unwrap() })
    }

    #[test]
    fn dest_location_can_parse_valid_inputs() {

        let urls = vec![
            // Absolute filepaths are ok:
            ("/foo/bar", u("/foo/bar")),
            // Relative filepaths are ok:
            ("./foo/bar", u("./foo/bar")),
            // More Relative filepaths are ok:
            ("../foo/bar", u("../foo/bar")),
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
            // IP + path is ok:
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
            ("  \t example.com\t \t", u("http://example.com/")),
            // Status code locations are ok:
            ("statuscode://404", code(404)),
            // Status code locations are ok:
            ("statuscode://204", code(204)),
        ];

        for (actual, expected) in urls {
            let src: SrcLocation = "localhost".parse().unwrap();
            let actual_loc: Result<DestLocation, _> = DestLocation::parse(actual, &src);
            assert!(actual_loc.is_ok(), "Location could not be parsed: '{}', result: {:?}", actual, actual_loc);
            assert_eq!(actual_loc.unwrap(), expected, "(Original was '{}')", actual);
        }
    }

    #[test]
    fn dest_location_wont_parse_invalid_urls() {
        let urls = vec![
            // Don't know this protocol:
            "foobar://example.com",
            // Statuscode should be a number:
            "statuscode://abc",
            "statuscode://100/abc",
        ];

        for actual in urls {
            let src: SrcLocation = "localhost".parse().unwrap();
            let actual_loc: Result<DestLocation, _> = DestLocation::parse(actual, &src);
            assert!(actual_loc.is_err(), "This invalid location should not have successfully parsed: {}", actual);
        }
    }

    #[test]
    fn dest_location_relates_to_src() {
        const VALID: bool = true;
        const INVALID: bool = false;

        let routes = vec![
            (VALID, "tcp://localhost:22", "tcp://localhost:2222"),
            (VALID, "tcp://localhost:22", "localhost:2222"),
            (VALID, "tcp://localhost:22", "2222"), // assume localhost for dest if not given
            (VALID, "http://localhost:22", "2222"), // assume localhost for dest if not given
            (VALID, "tcp://localhost:22", "localhost"), // assume same port as src if not given
            (VALID, "http://localhost", "localhost:2222"),
            (INVALID, "https://localhost", "localhost:2222"), // https is not a valid src protocol
            (INVALID, "tcp://localhost", "localhost:22"), // src needs port if TCP
            (INVALID, "tcp://127.0.0.1:2222", "http://localhost"), // protocol mismatch
            (INVALID, "http://127.0.0.1:2222", "tcp://localhost"), // protocol mismatch
            (INVALID, "tcp://localhost/foo", "80"), // no paths allowed on TCP
            (INVALID, "tcp://localhost", "80/foo"), // no paths allowed on TCP
            (VALID, "http://127.0.0.1:2222", "statuscode://123"), // HTTP can route to a statuscode
            (INVALID, "tcp://127.0.0.1:2222", "statuscode://123"), // TCP cannot route to a statuscode
            (VALID, "http://127.0.0.1:2222", "nothing"), // HTTP can route to nothing (statuscode 404)
            (INVALID, "tcp://127.0.0.1:2222", "nothing"), // What would "nothing" mean for TCP?
        ];

        for (is_valid, src, dest) in routes {
            let src_l: SrcLocation = match src.parse() {
                Ok(src) => src,
                Err(e) => {
                    if is_valid {
                        assert!(true, "{} should be valid src but got error: {}", src, e);
                    }
                    continue
                }
            };
            let dest_l = DestLocation::parse(dest, &src_l);
            if is_valid {
                assert!(dest_l.is_ok(), "{} => {} should be VALID but got error: {}", src, dest, dest_l.unwrap_err());
            } else {
                assert!(dest_l.is_err(), "{} => {} should be INVALID", src, dest);
            }
        }
    }

}