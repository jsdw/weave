use lazy_static::lazy_static;
use regex::Regex;
use std::str::FromStr;
use std::path::{ self, PathBuf };
use std::fmt;
use std::borrow::Cow;
use crate::errors::{ Error };
use super::src_location::{ Matches };
use super::utils::{ SplitUrl };

/// A Destination location. This is what a request can be rerouted to.
/// On matching, we look at the pair of source and destination locations
/// in order to construct a `ResolvedLocation`, which is where the request
/// will actually be routed to.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct DestLocation(DestLocationInner);

#[derive(Debug,Clone,PartialEq,Eq)]
pub enum DestLocationInner {
    Url{ host_bits: String, path: String, query: String },
    FilePath(String)
}

impl DestLocation {
    /// Parse a string into a destination location.
    pub fn parse(original: impl AsRef<str>) -> Result<DestLocation, Error> {
        let input = original.as_ref().trim();

        // Starts with a '.' or '/', so will assume it's a filepath:
        if [Some('.'), Some(path::MAIN_SEPARATOR)].contains(&input.chars().next()) {
            return Ok(DestLocation(DestLocationInner::FilePath(input.to_owned())));
        }

        // Else, expect it to look like a URL (this normalises things as well,
        // adding back a protocol/host/port if missing):
        let url = SplitUrl::parse(input)?;

        // Complain if the protocol is wrong:
        if !["http", "https"].contains(&url.protocol) {
            return Err(err!("Invalid protocol, expecting http or https only"));
        }

        let host_bits = if url.port == 80 {
            format!("{}://{}", url.protocol, url.host)
        } else {
            format!("{}://{}:{}", url.protocol, url.host, url.port)
        };

        Ok(DestLocation(DestLocationInner::Url{
            host_bits, path: url.path.into_owned(), query: url.query.to_owned()
        }))
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
                        if !current_query.is_empty() { query.push('&'); }
                        query.push_str(key);
                        query.push('=');
                        query.push_str(val);
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
            }
            DestLocationInner::FilePath(path) => {
                path.fmt(f)
            }
        }
    }
}

impl FromStr for DestLocation {
    type Err = Error;
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        DestLocation::parse(input)
    }
}

/// A ResolvedLocation is the destination of a given request, formed by
/// looking at the source and destination locations provided.
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum ResolvedLocation {
    Url(String),
    FilePath(PathBuf)
}

impl fmt::Display for ResolvedLocation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ResolvedLocation::Url(url) => url.fmt(f),
            ResolvedLocation::FilePath(path) => path.to_string_lossy().fmt(f)
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
    query.split('&').map(|part| {
        if let Some(mid) = part.find('=') {
            (&part[0..mid],&part[mid+1..])
        } else {
            (part, "")
        }
    })
}

#[cfg(test)]
mod test {

    use super::*;

    fn u (u: &str) -> DestLocation { DestLocation::parse(u).unwrap() }

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
            let actual_loc: Result<DestLocation, _> = actual.parse();
            assert!(actual_loc.is_ok(), "Location could not be parsed: '{}', result: {:?}", actual, actual_loc);
            assert_eq!(actual_loc.unwrap(), expected, "(Original was '{}')", actual);
        }
    }

    #[test]
    fn dest_location_wont_parse_invalid_urls() {
        let urls = vec![
            // Don't know this protocol:
            "foobar://example.com"
        ];

        for actual in urls {
            let actual_loc: Result<DestLocation, _> = actual.parse();
            assert!(actual_loc.is_err(), "This invalid location should not have successfully parsed: {}", actual);
        }
    }

}