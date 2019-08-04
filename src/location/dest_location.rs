use url::Url;
use std::str::FromStr;
use std::path::{ self, PathBuf };
use std::fmt;
use std::borrow::Cow;
use crate::errors::{ Error };

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