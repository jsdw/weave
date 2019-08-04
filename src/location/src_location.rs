use url::Host;
use regex::Regex;
use lazy_static::lazy_static;
use std::str::FromStr;
use std::fmt;
use std::net::{ SocketAddr, ToSocketAddrs };
use crate::errors::{ Error };

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
    pub fn parse(original: impl AsRef<str>) -> Result<SrcLocation, Error> {

        lazy_static!{
            // Are we matching on parts of the path? (.*?) is a non greedy match, to match as little
            // as possible, which is necessary to support multiple match patterns.
            static ref HOST_AND_PORT_RE: Regex = Regex::new(r"^(.*):([0-9]+)$").expect("host_and_port_re");
        }

        let input: &str = original.as_ref();

        // Does the input begin with "="? Exact matches only if it does
        let (exact, input) = if input.starts_with('=') {
            (true, &input[1..])
        } else {
            (false, input)
        };

        // Did we specify an input protocol? It should be nothing or http
        let (protocol, input) = if let Some(n) = input.find("://") {
            (&input[0..n], &input[n+3..])
        } else {
            ("http", input)
        };
        if protocol != "http" {
            return Err(err!("Incalid protocol: expected 'http'"))
        }

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
            (host, port)
        } else if let Ok(n) = host_and_port.parse() {
            ("localhost", n)
        } else {
            (host_and_port, 80)
        };

        // Path should always begin with "/":
        let path_and_query = if input.starts_with("/") { input.to_owned() } else { format!("/{}", input) };
        let (path, _query) = split_path_and_query(&path_and_query);

        // Host default to localhost if not provided:
        let host = Host::parse(if host.is_empty() { "localhost" } else { host })?;
        // Parse path_and_query into pieces to build a regex from:
        let path_pieces = parse_path(path);
        // Did we find any patterns?
        let has_patterns = path_pieces.iter().any(|p| if let PathPiece::Pattern{..} = p { true } else { false });
        // Make the regex:
        let path_regex = convert_path_pieces_to_regex(path_pieces, exact);

        // and hand this all back:
        Ok(SrcLocation {
            host,
            path: path.to_owned(),
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

impl PartialEq for SrcLocation {
    fn eq(&self, other: &Self) -> bool {
        self.host == other.host &&
        self.port == other.port &&
        self.exact == other.exact &&
        self.has_patterns == other.has_patterns &&
        self.path_regex.as_str() == other.path_regex.as_str()
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
        let host = if self.port == 80 { format!("{}",self.host) } else { format!("{}:{}", self.host, self.port) };
        write!(f, "{}{}", host, self.path)
    }
}

/// Split path_and_query into separate path and query pieces, noting that a '?' can appear
/// inside a pattern and refusing to match on that.
fn split_path_and_query(path_and_query: &str) -> (&str, &str) {
    lazy_static!{
        static ref QUERY_PARAMS_RE: Regex = Regex::new(r"\?([^)]|$)").expect("query_params_re");
    }

    // Split on '?' if exists outside of a match point.
    // @TODO: Do something useful with query params.
    if let Some(m) = QUERY_PARAMS_RE.find(path_and_query) {
        let n = m.start();
        (&path_and_query[0..n], &path_and_query[n+1..])
    } else {
        (path_and_query, "")
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
