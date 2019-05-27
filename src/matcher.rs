use hyper::Uri;
use url::Url;
use lazy_static::lazy_static;
use regex::Regex;
use std::cmp::{ Ordering };
use std::path::PathBuf;
use std::borrow::{ Borrow, Cow };
use crate::routes::{ Route };
use crate::location::{ DestLocation, ResolvedLocation };

#[derive(Debug, Clone)]
pub struct Matcher {
    routes: Vec<Route>
}

impl Matcher {
    /// Build a new matcher given some routes we'd like to match on:
    pub fn new(mut routes: Vec<Route>) -> Matcher {
        // Ordering:
        // 1. basic exact match (longest first)
        // 2. regex exact match (in order declared)
        // 3. basic prefix (longest first)
        // 4. regex prefix (in order declared)
        routes.sort_by(|a, b| {
            // Put all exact matching routes first:
            a.src.exact.cmp(&b.src.exact).reverse().then_with(|| {
                match (a.src.path_regex.is_some(), b.src.path_regex.is_some()) {
                    // If regex, put that last, but maintain
                    // ordering within regex'd paths:
                    (true, true)   => Ordering::Equal,
                    (false, true)  => Ordering::Less,
                    (true, false)  => Ordering::Greater,
                    // If neither is regex, reverse sort based on path length
                    // to put longer paths first:
                    (false, false) => {
                        a.src.url.path().len()
                            .cmp(&b.src.url.path().len())
                            .reverse()
                    }
                }
            })
        });
        Matcher { routes }
    }

    /// Match a Uri against the routes provided. This returns
    /// the Location to serve up.
    pub fn resolve(&self, uri: &Uri) -> Option<ResolvedLocation> {
        // Find a matching route. We assume routes are ordered and
        // the first match wins.
        self.routes.iter().find_map(|route| resolve_route(uri, route))
    }
}

fn resolve_route(uri: &Uri, route: &Route) -> Option<ResolvedLocation> {
    let path = uri.path();

    // Attempt to match on provided regex:
    if let Some(re) = &route.src.path_regex {
        let re_captures = re.captures(path);
        if let Some(captures) = re_captures {
            let rest_of_path = &path[ captures.get(0).unwrap().end().. ];
            Some(match route.dest.clone() {
                DestLocation::Url(url) => {
                    let expanded_url = expand_url_with_captures(&captures, url);
                    ResolvedLocation::Url(merge_tail_and_uri_with_url(rest_of_path, uri, expanded_url))
                },
                DestLocation::FilePath(path) => {
                    let expanded_path = expand_path_with_captures(&captures, path);
                    ResolvedLocation::FilePath(merge_tail_with_path(rest_of_path, expanded_path))
                }
            })
        } else {
            None
        }
    }
    // No regex, so see whether incoming path starts with route src:
    else if (route.src.exact && path == route.src.url.path())
            || (!route.src.exact && path.starts_with(route.src.url.path())) {
        let rest_of_path = &path[ route.src.url.path().len().. ];
        Some(match route.dest.clone() {
            DestLocation::Url(url) => {
                ResolvedLocation::Url(merge_tail_and_uri_with_url(rest_of_path, uri, url))
            },
            DestLocation::FilePath(filepath) => {
                ResolvedLocation::FilePath(merge_tail_with_path(rest_of_path, filepath.into()))
            }
        })
    }
    // The URI failed to match this route:
    else {
        None
    }
}

fn expand_url_with_captures(captures: &regex::Captures, mut url: Url) -> Url {
    let new_path = expand_str_with_captures(captures, url.path()).into_owned();
    url.set_path(&new_path);
    url
}

fn expand_path_with_captures(captures: &regex::Captures, path: String) -> PathBuf {
    let new_path = expand_str_with_captures(captures, &path);
    let s: &str = new_path.borrow();
    s.into()
}

fn expand_str_with_captures<'a>(captures: &regex::Captures, s: &'a str) -> Cow<'a, str> {
    lazy_static!{
        // Are we matching on parts of the path?
        static ref MATCH_NAME_RE: Regex = Regex::new(r"\(([a-zA-Z][a-zA-Z0-9_-]*)\)").expect("match_point_re");
    }

    // @TODO: Figure out lifetimes to avoid returning owned strings in closure:
    MATCH_NAME_RE.replace_all(s, |cap: &regex::Captures| -> String {
        let replace_name = cap.get(1).unwrap().as_str();
        if let Some(replacement) = captures.name(replace_name) {
            replacement.as_str().to_owned()
        } else {
            cap.get(0).unwrap().as_str().to_owned()
        }
    })
}

fn merge_tail_and_uri_with_url(tail: &str, uri: &Uri, mut url: Url) -> Url {

    if !tail.is_empty() {
        let curr_path = url.path().trim_end_matches('/');
        let tail_path = tail.trim_start_matches('/');
        let combined_path = format!("{}/{}", curr_path, tail_path);
        url.set_path(&combined_path);
    }

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

fn merge_tail_with_path(tail: &str, mut path: PathBuf) -> PathBuf {

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
    use crate::location::{ SrcLocation, DestLocation };

    use super::*;

    fn uri (s: &str) -> Uri { s.parse().unwrap() }
    fn url (u: &str) -> Url { Url::from_str(u).unwrap() }
    fn resolved_url (u: &str) -> ResolvedLocation { ResolvedLocation::Url(url(u)) }
    fn path (s: &str) -> PathBuf { s.into() }

    #[test]
    fn basic_merging_with_urls() {
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
            assert_eq!(merge_tail_and_uri_with_url(tail, &uri, url), expected);
        }
    }

    #[test]
    fn basic_merging_with_paths() {
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
            assert_eq!(merge_tail_with_path(tail, path), expected);
        }
    }

    #[test]
    fn exact_prefix_means_exact() {
        let routes = vec![
            Route {
                src: SrcLocation::parse("=8080/foo/bar").unwrap(),
                dest: DestLocation::parse("9090/1").unwrap()
            },
            // This path is longer, and so can accidentally be sorted
            // before the above if path length is taken into account
            // when it shouldn't be:
            Route {
                src: SrcLocation::parse("=8080/favicon.ico").unwrap(),
                dest: DestLocation::parse("9090/2").unwrap()
            }
        ];

        let matcher = Matcher::new(routes);
        let cases = vec![
            (uri("/foo/bar"), Some(resolved_url("http://localhost:9090/1"))),
            (uri("/favicon.ico"), Some(resolved_url("http://localhost:9090/2"))),
            // These aren't exact matches and so should fail to match:
            (uri("/foo/bar/"), None),
            (uri("/foo/bar/wibble"), None),
            (uri("/favicon.ico/"), None),
            (uri("/favicon.ico/wibble"), None),
        ];

        for (uri, expected) in cases {
            let res = matcher.resolve(&uri);
            assert_eq!(res, expected, "original URI: {}", uri);
        }
    }

    #[test]
    fn dont_add_trailing_slash_to_exact_match() {
        let routes = vec![
            Route {
                src: SrcLocation::parse("8080/hello/bar").unwrap(),
                dest: DestLocation::parse("9090/wibble/bar").unwrap()
            },
            Route {
                src: SrcLocation::parse("8080/hello/bar.json").unwrap(),
                dest: DestLocation::parse("9090/wibble/bar.json").unwrap()
            },
            Route {
                src: SrcLocation::parse("=8080/hello/wibble").unwrap(),
                dest: DestLocation::parse("9090/hi/wibble").unwrap()
            },
            Route {
                src: SrcLocation::parse("=8080/hello/wibble.json").unwrap(),
                dest: DestLocation::parse("9090/hi/wibble.json").unwrap()
            },
        ];

        let matcher = Matcher::new(routes);
        let cases = vec![
            (uri("/hello/bar"), resolved_url("http://localhost:9090/wibble/bar")),
            (uri("/hello/bar.json"), resolved_url("http://localhost:9090/wibble/bar.json")),
            (uri("/hello/wibble"), resolved_url("http://localhost:9090/hi/wibble")),
            (uri("/hello/wibble.json"), resolved_url("http://localhost:9090/hi/wibble.json")),
        ];

        for (uri, expected) in cases {
            let res = matcher.resolve(&uri);
            assert_eq!(res, Some(expected), "original URI: {}", uri);
        }
    }

    #[test]
    fn match_first_available_regex_pattern() {
        let routes = vec![
            Route {
                src: SrcLocation::parse("8080/(foo)/bar").unwrap(),
                dest: DestLocation::parse("9090/bar/(foo)/1").unwrap()
            },
            // This path is longer, and so can accidentally be sorted
            // before the above if path length is taken into account
            // when it shouldn't be:
            Route {
                src: SrcLocation::parse("8080/(foo)/(bar)").unwrap(),
                dest: DestLocation::parse("9090/(bar)/(foo)/2").unwrap()
            }
        ];

        let matcher = Matcher::new(routes);
        let res = matcher.resolve(&uri("/hello/bar"));
        let expected = resolved_url("http://localhost:9090/bar/hello/1");
        assert_eq!(res, Some(expected));
    }

    #[test]
    fn match_exact_regex_over_prefix() {
        let routes = vec![
            // This basic prefix route should not be picked:
            Route {
                src: SrcLocation::parse("8080/hello/bar/").unwrap(),
                dest: DestLocation::parse("9090/wibble/0/").unwrap()
            },
            // This regex path should be picked, because exact regex routes
            // should always match over prefix routes:
            Route {
                src: SrcLocation::parse("=8080/(hello)/(bar)/wibble").unwrap(),
                dest: DestLocation::parse("9090/wibble/1/").unwrap()
            },
        ];

        let matcher = Matcher::new(routes);
        let res = matcher.resolve(&uri("/hello/bar/wibble"));
        let expected = resolved_url("http://localhost:9090/wibble/1/");
        assert_eq!(res, Some(expected));
    }

    #[test]
    fn match_exact_over_prefix() {
        let routes = vec![
            // This basic prefix route should not be picked:
            Route {
                src: SrcLocation::parse("8080/foo").unwrap(),
                dest: DestLocation::parse("9090/1").unwrap()
            },
            // This shorter but exact route should be picked:
            Route {
                src: SrcLocation::parse("=8080/foo").unwrap(),
                dest: DestLocation::parse("9090/2").unwrap()
            },
        ];

        let matcher = Matcher::new(routes);
        let res = matcher.resolve(&uri("/foo"));
        let expected = resolved_url("http://localhost:9090/2");
        assert_eq!(res, Some(expected));
    }

    #[test]
    fn regex_with_urls() {
        let routes = vec![
            // The first route is not regex based; this should be ignored
            // in favour of exact regex ones where applicable:
            Route {
                src: SrcLocation::parse("8080/hello/bar/").unwrap(),
                dest: DestLocation::parse("9090/wibble/0/").unwrap()
            },
            // Regex based but *not* exact (no trailing '='), so should
            // be less specific than all of the below:
            Route {
                src: SrcLocation::parse("8080/(foo)/bar").unwrap(),
                dest: DestLocation::parse("9090/bar/(foo)/nonexact").unwrap()
            },
            Route {
                src: SrcLocation::parse("=8080/(foo)/bar").unwrap(),
                dest: DestLocation::parse("9090/bar/(foo)/1").unwrap()
            },
            // Multiple captures helps test that we have built up the
            // right regex to match on in the first place, since greediness
            // can lead to only the last capture being spotted:
            Route {
                src: SrcLocation::parse("=8080/(foo)/(bar)").unwrap(),
                dest: DestLocation::parse("9090/(bar)/(foo)/2").unwrap()
            },
            Route {
                src: SrcLocation::parse("=8080/(foo)/(bar)/wibble").unwrap(),
                dest: DestLocation::parse("9090/wibble/(bar)/(foo).json3").unwrap()
            },
            // This should capture anything with at least one '/' in the middle:
            Route {
                src: SrcLocation::parse("=8080/(foo..)/(bar)/boom").unwrap(),
                dest: DestLocation::parse("9090/boom/(bar)/(foo)/4").unwrap()
            },
            // This should capture anything with 'BOOM' in the middle
            Route {
                src: SrcLocation::parse("=8080/(foo..)/BOOM/(bar..)").unwrap(),
                dest: DestLocation::parse("9090/(foo)/exploding/(bar)").unwrap()
            },
        ];

        let matcher = Matcher::new(routes);

        let cases = vec![
            (uri("/hello/bar"), resolved_url("http://localhost:9090/bar/hello/1")),
            (uri("/hello/baz"), resolved_url("http://localhost:9090/baz/hello/2")),
            (uri("/hello/bar/wibble"), resolved_url("http://localhost:9090/wibble/bar/hello.json3")),
            // Should fall back to the non-regex route:
            (uri("/hello/bar/lark"), resolved_url("http://localhost:9090/wibble/0/lark")),
            // Should fall back to the first route with '..'s:
            (uri("/foo/bar/lark/wibble/boom"), resolved_url("http://localhost:9090/boom/wibble/foo/bar/lark/4")),
            // We can replace single parts of a path using multiple '..'s:
            (uri("/1/2/3/BOOM/4/5"), resolved_url("http://localhost:9090/1/2/3/exploding/4/5")),
            (uri("/1/BOOM/2/3/4/5"), resolved_url("http://localhost:9090/1/exploding/2/3/4/5")),
            // We can match the non-exact regex only if the route matches nothing else:
            (uri("/foo/bar/lark/wibble"), resolved_url("http://localhost:9090/bar/foo/nonexact/lark/wibble")),
        ];

        for (uri, expected) in cases {
            let res = matcher.resolve(&uri);
            assert_eq!(res, Some(expected), "original URI: {}", uri);
        }
    }

}