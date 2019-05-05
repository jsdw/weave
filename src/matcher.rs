use hyper::Uri;
use url::Url;
use std::cmp::Reverse;
use std::path::PathBuf;
use crate::routes::{ Route, Location };

#[derive(Debug)]
pub struct Matcher {
    routes: Vec<Route>
}

impl Matcher {
    /// Build a new matcher given some routes we'd like to
    /// match on:
    pub fn new(mut routes: Vec<Route>) -> Matcher {
        // Longest src paths first, so that we match on the most
        // specific path first:
        routes.sort_by_cached_key(|r| Reverse(r.src.path().to_owned()));
        Matcher { routes }
    }

    /// Match a Uri against the routes provided. This returns
    /// the Location to serve up.
    pub fn resolve(&self, uri: &Uri) -> Option<Location> {

        let path = uri.path();

        // Find a matching route:
        let route = self.routes.iter().find(|&route| {
            path.starts_with(route.src.path())
        })?;

        // The tail end of the path that wasn't matched on:
        let rest_of_path = &path[ route.src.path().len().. ];

        // Merge the incoming URI with the rest of the path and the
        // destination location, and return it:
        Some(match route.dest.clone() {
            Location::Url(url) => {
                Location::Url(merge_uri_with_url(rest_of_path, uri, url))
            },
            Location::FilePath(path) => {
                Location::FilePath(merge_uri_with_path(rest_of_path, uri, path))
            }
        })

    }
}

fn merge_uri_with_url(tail: &str, uri: &Uri, mut url: Url) -> Url {

    let curr_path = url.path().trim_end_matches('/');
    let tail_path = tail.trim_start_matches('/');
    let combined_path = format!("{}/{}", curr_path, tail_path);

    url.set_path(&combined_path);

    let curr_query = url.query()
        .map(|q| q.to_owned())
        .unwrap_or(String::new());
    let uri_query = uri.query()
        .map(|q| q.to_owned())
        .unwrap_or(String::new());

    if !curr_query.is_empty() && !uri_query.is_empty() {
        url.set_query(Some(&format!("{}&{}", curr_query, uri_query)));
    } else if !curr_query.is_empty() || !uri_query.is_empty() {
        url.set_query(Some(&format!("{}{}", curr_query, uri_query)));
    } else {
        url.set_query(None);
    }

    url
}

fn merge_uri_with_path(tail: &str, _uri: &Uri, mut path: PathBuf) -> PathBuf {

    let bits = tail.split('/').filter(|s| !s.is_empty());
    let mut appended = 0;

    for bit in bits {
        // Ignore bits that would do nothing:
        if bit == "." || bit.is_empty() {
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

    path
}

#[cfg(test)]
mod test {

    use std::str::FromStr;
    use hyper::Uri;
    use url::Url;
    use std::path::PathBuf;

    use super::*;

    fn uri (s: &str) -> Uri { s.parse().unwrap() }
    fn url (u: &str) -> Url { Url::from_str(u).unwrap() }
    fn path (s: &str) -> PathBuf { s.into() }

    #[test]
    fn merge_with_urls() {
        let cases = vec![
            // Some basic checks:
            ("/bar", uri("/"), url("http://localhost"), url("http://localhost/bar")),
            ("/bar/", uri("/"), url("http://localhost:8080/"), url("http://localhost:8080/bar/")),
            ("bar/", uri("/"), url("http://localhost"), url("http://localhost/bar/")),
            // Merging paths:
            ("/bar", uri("/"), url("http://localhost/lark"), url("http://localhost/lark/bar")),
            ("/bar/", uri("/"), url("http://localhost/lark/foo"), url("http://localhost/lark/foo/bar/")),
            ("bar/", uri("/"), url("http://localhost/lark"), url("http://localhost/lark/bar/")),
            // Merging paths and query strings:
            ("/bar", uri("/"), url("http://localhost/lark?a=2"), url("http://localhost/lark/bar?a=2")),
            ("/bar/", uri("/?b=hi"), url("http://localhost/lark/foo?a=bye"), url("http://localhost/lark/foo/bar/?a=bye&b=hi")),
            // TODO: Override query params with those from URI rather than just combining them:
            ("bar/", uri("/?b=hi&c=2"), url("http://localhost/lark?c=6"), url("http://localhost/lark/bar/?c=6&b=hi&c=2")),
        ];

        for (tail, uri, url, expected) in cases {
            assert_eq!(merge_uri_with_url(tail, &uri, url), expected);
        }
    }

    #[test]
    fn merge_with_paths() {
        let cases = vec![
            // Basic checks:
            ("/bar", path("../foo"), path("../foo/bar")),
            ("/bar/wibble", path("../foo"), path("../foo/bar/wibble")),
            ("/bar/wibble/something.jpg", path("../foo"), path("../foo/bar/wibble/something.jpg")),
            // '..' parts can only undo additional parts of the path:
            ("/../../../", path("../foo"), path("../foo")),
            ("/../bar", path("../foo"), path("../foo/bar")),
            ("/../../bar", path("../foo"), path("../foo/bar")),
            ("/bar/../", path("../foo"), path("../foo/")),
            ("/bar/../lark", path("../foo"), path("../foo/lark")),
            ("./bar/.././lark", path("../foo"), path("../foo/lark")),
        ];

        for (tail, path, expected) in cases {
            assert_eq!(merge_uri_with_path(tail, &uri("/"), path), expected);
        }
    }

}