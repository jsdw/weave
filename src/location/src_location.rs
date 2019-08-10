use hyper::Uri;
use lazy_static::lazy_static;
use regex::Regex;
use url::Host;
use std::cmp::Ordering;
use std::str::FromStr;
use std::fmt;
use std::net::{ SocketAddr, ToSocketAddrs };
use crate::errors::{ Error };
use super::utils::{ SplitUrl };

/// A source location. It should be something that looks a little
/// like a URL, so that we know what interface and port to listen on, and
/// what path to match on incoming requests if any.
#[derive(Debug,Clone)]
pub struct SrcLocation {
    /// Host:
    host: Host<String>,
    /// Port:
    port: u16,
    /// Raw path as entered, for display purposes:
    path: String,
    /// Match on paths using this regex:
    path_regex: Regex,
    /// Do we want this to be for exact matches only?
    exact: bool,
    /// Does this path have patterns in?
    has_patterns: bool
}

impl SrcLocation {
    /// Parse a string into a source location.
    pub fn parse(original: impl AsRef<str>) -> Result<SrcLocation, Error> {
        let input: &str = original.as_ref();

        // Does the input begin with "="? Exact matches only if it does
        let (exact, input) = if input.starts_with('=') {
            (true, &input[1..])
        } else {
            (false, input)
        };

        // Split the URL into pieces:
        let SplitUrl { protocol, host, port, path, .. } = SplitUrl::parse(input)?;

        if protocol != "http" {
            return Err(err!("Invalid protocol: expected 'http'"))
        }

        // Parse the path into pieces to build a regex from:
        let path_pieces = parse_path(&path);
        // Did we find any patterns?
        let has_patterns = path_pieces.iter().any(|p| if let PathPiece::Pattern{..} = p { true } else { false });
        // Make the regex:
        let path_regex = convert_path_pieces_to_regex(path_pieces, exact);

        // and hand this all back:
        Ok(SrcLocation {
            host,
            path: path.into_owned(),
            port,
            path_regex,
            exact,
            has_patterns
        })
    }
    /// Match an incoming request and give back a map of key->value pairs
    /// found in performing the match.
    pub fn match_uri<'a, 'b: 'a>(&'a self, uri: &'b Uri) -> Option<Matches<'a>> {

        let request_path = uri.path();
        let request_query = uri.query().unwrap_or("");

        // Try to match the incoming path on the regex:
        if let Some(captures) = self.path_regex.captures(request_path) {
            let path_tail = &request_path[ captures.get(0).unwrap().end().. ];
            Some(Matches {
                captures,
                path_tail,
                query: request_query
            })
        }
        // If we can't, this route is not a match:
        else {
            None
        }

    }
    /// Hand back a socket address that we can listen on for this route.
    pub fn to_socket_addr(&self) -> Result<SocketAddr, Error> {
        match self.host {
            Host::Ipv4(addr) => Ok(SocketAddr::from((addr,self.port))),
            Host::Ipv6(addr) => Ok(SocketAddr::from((addr,self.port))),
            Host::Domain(ref s) => {
                // This does a potentially blocking lookup.
                let mut addrs = (&**s, self.port).to_socket_addrs().map_err(|e| {
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

// Ordering:
// 1. basic exact match (longest first)
// 2. regex exact match (in order declared)
// 3. basic prefix (longest first)
// 4. regex prefix (in order declared)
impl Ord for SrcLocation {
    fn cmp(&self, other: &Self) -> Ordering {
        // Put all exact matching routes first:
        self.exact.cmp(&other.exact).reverse().then_with(|| {
            match (self.has_patterns, other.has_patterns) {
                // If regex, put that last, but maintain
                // ordering within regex'd paths:
                (true, true)   => Ordering::Equal,
                (false, true)  => Ordering::Less,
                (true, false)  => Ordering::Greater,
                // If neither is regex, reverse sort based on path length
                // to put longer paths first:
                (false, false) => {
                    self.path_regex.as_str().len()
                        .cmp(&other.path_regex.as_str().len())
                        .reverse()
                }
            }
        })
    }
}
impl PartialOrd for SrcLocation {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}


impl PartialEq for SrcLocation {
    fn eq(&self, other: &Self) -> bool {
        self.host == other.host &&
        self.port == other.port &&
        self.exact == other.exact &&
        self.has_patterns == other.has_patterns &&
        self.path_regex.as_str() == other.path_regex.as_str()
    }
}
impl Eq for SrcLocation { }

impl FromStr for SrcLocation {
    type Err = Error;
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        SrcLocation::parse(input.to_owned())
    }
}

impl fmt::Display for SrcLocation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.port == 80 {
            write!(f, "{}{}", self.host, self.path)
        } else {
            write!(f, "{}:{}{}", self.host, self.port, self.path)
        }
    }
}

/// Parse a path into pieces containing either raw strings or patterns to match on:
fn parse_path(path: &str) -> Vec<PathPiece> {
    lazy_static!{
        // Are we matching on parts of the path? (.*?) is a non greedy match, to match as little
        // as possible, which is necessary to support multiple match patterns.
        static ref MATCH_POINT_RE: Regex = Regex::new(r"(.*?)(\(([a-zA-Z][a-zA-Z0-9_-]*)(\.\.)?\))").expect("match_point_re");
    }

    // Next, find the patterns in our path:
    let mut last_idx = 0;
    let mut path_pieces = vec![];
    for cap in MATCH_POINT_RE.captures_iter(path) {

        let path_str = cap.get(1).unwrap().as_str();
        let all_pattern = cap.get(2).unwrap();
        let name = cap.get(3).unwrap().as_str();
        let greedy = cap.get(4).is_some();

        if !path_str.is_empty() {
            path_pieces.push(PathPiece::Str(path_str))
        }
        path_pieces.push(PathPiece::Pattern {
            name,
            greedy
        });
        last_idx = all_pattern.end();
    }

    // Consume the rest of the string:
    path_pieces.push(PathPiece::Str(&path[last_idx..]));

    path_pieces
}
enum PathPiece<'a> {
    Str(&'a str),
    Pattern{
        name: &'a str,
        greedy: bool
    }
}

/// Convert a path into something that matches incoming paths, and return
/// whether or not any pattern matching is used at all.
fn convert_path_pieces_to_regex(path_pieces: Vec<PathPiece>, exact: bool) -> Regex {

    let mut re_expr: String = String::new();

    // A selection of regexps to match different pattern flavours:
    static GREEDY: &str = "(?P<{}>.+?)";
    static NONGREEDY: &str = "(?P<{}>[^/]+)";

    // Assemble a regex string if we find matchers:
    for piece in path_pieces {
        match piece {
            PathPiece::Str(s) => {
                re_expr.push_str(&regex::escape(s));
            },
            PathPiece::Pattern{ name, greedy } => {
                let re_str = match greedy {
                    true    => GREEDY,
                    false    => NONGREEDY,
                };
                re_expr.push_str(&re_str.replace("{}", name));
            }
        }
    }

    // Allow trailing chars if not exact, else prohibit:
    let regex_string = if exact {
        format!("^{}$", re_expr)
    } else {
        format!("^{}", re_expr)
    };

    Regex::new(&regex_string).expect("invalid convert regex built up")
}

/// Present matches back, given a path to match on.
pub struct Matches<'a> {
    captures: regex::Captures<'a>,
    path_tail: &'a str,
    query: &'a str
}

impl Matches<'_> {
    pub fn get(&self, name: &str) -> Option<&str> {
        self.captures.name(name).map(|m| m.as_str())
    }
    pub fn path_tail(&self) -> &str {
        self.path_tail
    }
    pub fn query(&self) -> &str {
        self.query
    }
}