use hyper::Uri;
use std::cmp::Reverse;
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

fn merge_uri_with_url(tail: &str, uri: &Uri, url: Url) -> Url {
    unimplemented!()
}

fn merge_uri_with_path(tail: &str, uri: &Uri, path: PathBuf) -> PathBuf {
    unimplemented!()
}