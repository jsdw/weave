use url::Url;
use regex::Regex;
use lazy_static::lazy_static;
use std::str::FromStr;
use std::path::{ self, PathBuf };
use std::fmt;
use std::borrow::Cow;
use crate::errors::{ Error };

/// A source location. It should be something that looks a little
/// like a URL, so that we know what interface and port to listen on, and
/// what path to match on incoming requests if any.
#[derive(Debug,Clone)]
pub struct SrcLocation {
    pub url: Url,
    pub path_regex: Option<Regex>,
    /// Do we want this to be for exact matches only?
    pub exact: bool
}

impl SrcLocation {
    pub fn parse(input: impl AsRef<str>) -> Result<SrcLocation, Error> {
        // Starts with '=' means exact match. chop off if found.
        let mut input: &str = input.as_ref();
        let mut exact = false;
        if input.starts_with("=") {
            input = &input[1..];
            exact = true;
        }

        // Assume something like a URL has been provided:
        let url = parse_url(input)?;

        // Does the path contain match points (eg {foo}, {bar..}, {lark:.*})?
        // If so, form a regex based on those. If not, build simple regex to
        // just match the beginning:
        let path_regex = convert_path_to_regex(url.path(), exact);

        Ok(SrcLocation {
            url,
            path_regex,
            exact
        })
    }
}

impl PartialEq for SrcLocation {
    fn eq(&self, other: &Self) -> bool {
        self.url == other.url
    }
}

impl FromStr for SrcLocation {
    type Err = Error;
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        SrcLocation::parse(input)
    }
}

impl fmt::Display for SrcLocation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.url.fmt(f)
    }
}

/// If a path contains match points (eg {foo}, {bar..}, {lark:.*}),
/// convert it into a regex that matches on those. If not, convert
/// into a regex that matches the beginning of a path.
fn convert_path_to_regex(path: &str, exact: bool) -> Option<Regex> {
    lazy_static!{
        // Are we matching on parts of the path? (.*?) is a non greedy match, to match as little
        // as possible, which is necessary to support multiple match patterns.
        static ref MATCH_POINT_RE: Regex = Regex::new(r"(.*?)\(([a-zA-Z][a-zA-Z0-9_-]*)(\.\.)?\)").expect("match_point_re");
    }

    let mut is_regex = false;
    let mut re_expr: String = String::new();
    let mut last_idx = 0;

    // Assemble a regex string if we find matchers:
    for cap in MATCH_POINT_RE.captures_iter(path) {
        is_regex = true;
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

    // Return None if there are no matchers, or push end of string
    // onto regex if there were:
    if is_regex {
        re_expr.push_str(&regex::escape(&path[last_idx..]))
    } else {
        return None
    }

    // Allow trailing chars if not exact, else prohibit:
    let regex_string = if exact {
        format!("^{}$", re_expr)
    } else {
        format!("^{}", re_expr)
    };

    let re = Regex::new(&regex_string).expect("invalid convert regex built up");
    Some(re)
}

/// A Destination location. This is what a request can be rerouted to.
/// On matching, we look at the pair of source and destination locations
/// in order to construct a `ResolvedLocation`, which is where the request
/// will actually be routed to.
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum DestLocation {
    Url(Url),
    FilePath(String),
}

impl DestLocation {
    pub fn parse(input: impl AsRef<str>) -> Result<DestLocation, Error> {
        let s = input.as_ref().trim().to_owned();

        // Starts with a '.' or '/', so will assume it's a filepath:
        if [Some('.'), Some(path::MAIN_SEPARATOR)].contains(&s.chars().next()) {
            return Ok(DestLocation::FilePath(s.into()));
        }

        // Else, assume something like a URL has been provided:
        let url = parse_url(s)?;
        Ok(DestLocation::Url(url))
    }
}

impl FromStr for DestLocation {
    type Err = Error;
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        DestLocation::parse(input)
    }
}

impl fmt::Display for DestLocation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DestLocation::Url(url) => url.fmt(f),
            DestLocation::FilePath(path) => path.fmt(f)
        }
    }
}

/// A ResolvedLocation is the destination of a given request, formed by
/// looking at the source and destination locations provided.
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum ResolvedLocation {
    Url(Url),
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

/// Parse something that looks like a URL into one:
fn parse_url(input: impl AsRef<str>) -> Result<Url, Error> {
    let mut s = Cow::Borrowed(input.as_ref());

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
        s = Cow::Owned(new_s);
    }

    // Doesn't have a scheme? Add one.
    if let None = s.find("://") {
        let mut new_s = "http://".to_owned();
        new_s.push_str(&s);
        s = Cow::Owned(new_s);
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

    Ok(url)
}


#[cfg(test)]
mod test {

    use super::*;

    fn u (u: &str) -> DestLocation { DestLocation::Url(Url::from_str(u).unwrap()) }

    #[test]
    fn dest_location_can_parse_valid_inputs() {

        let urls = vec![
            // Absolute filepaths are ok:
            ("/foo/bar", DestLocation::FilePath("/foo/bar".into())),
            // Relative filepaths are ok:
            ("./foo/bar", DestLocation::FilePath("./foo/bar".into())),
            // More Relative filepaths are ok:
            ("../foo/bar", DestLocation::FilePath("../foo/bar".into())),
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