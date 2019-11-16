use hyper::Uri;
use crate::routes::{ Route };
use crate::location::{ ResolvedLocation };

#[derive(Debug, Clone)]
pub struct Matcher {
    routes: Vec<Route>
}

impl Matcher {
    /// Build a new matcher given some routes we'd like to match on:
    pub fn new(mut routes: Vec<Route>) -> Matcher {
        routes.sort_by(|a,b| a.src.cmp(&b.src));
        Matcher { routes }
    }

    /// Match a Uri against the routes provided. This returns
    /// the Location to serve up.
    pub fn resolve(&self, uri: &Uri) -> Option<ResolvedLocation> {
        // Find a matching route. We assume routes are ordered and
        // the first match wins.
        self.routes.iter().find_map(|route| {
            route.src.match_uri(uri).map(|matches| {
                route.dest.resolve(&matches)
            })
        })
    }
}

#[cfg(test)]
mod test {

    use hyper::Uri;
    use crate::location::{ SrcLocation, DestLocation, ResolvedLocation };

    use super::*;

    fn url (u: &str) -> Option<ResolvedLocation> { Some(ResolvedLocation::Url(u.to_owned())) }
    fn path (u: &str) -> Option<ResolvedLocation> { Some(ResolvedLocation::FilePath(u.to_owned().into())) }
    fn none () -> Option<ResolvedLocation> { None }
    fn test_route_matches(routes: Vec<(&str,&str)>, cases: Vec<(&str, Option<ResolvedLocation>)>) {
        let routes: Vec<Route> = routes.into_iter().map(|(src,dest)| {
            let src: SrcLocation = src.parse().unwrap();
            Route {
                src: src.clone(),
                dest: DestLocation::parse(dest, &src).unwrap()
            }
        }).collect();
        let matcher = Matcher::new(routes);
        for (input, expected) in cases {
            let input_uri: Uri = input.parse().unwrap();
            let res = matcher.resolve(&input_uri);
            assert_eq!(res, expected, "original URI: {}", input_uri);
        }
    }

    #[test]
    fn paths1() {
        test_route_matches(
            vec![
                ("8080", ".")
            ],
            vec![
                ("/", path(".")),
                ("/foo", path("./foo")),
                ("/foo/bar", path("./foo/bar")),
                ("/foo/./bar.html", path("./foo/bar.html")),
                ("/foo/bar/..", path("./foo")),
                ("/foo/bar/../../", path(".")),
                ("/foo/bar/../../../", path(".")),
                ("/foo/././bar/../../../", path(".")),
                ("/foo?hello", path("./foo")),
                ("/foo?hello=2", path("./foo")),
                ("/foo/?hello=2#wibble", path("./foo")),
            ]
        )
    }

    #[test]
    fn paths2() {
        test_route_matches(
            vec![
                ("8080/foo/bar", ".")
            ],
            vec![
                ("/", none()),
                ("/foo", none()),
                ("/foo/ba", none()),
                ("/foo/bar", path(".")),
                ("/foo/bar/", path(".")),
                ("/foo/bar/?foo=lark", path(".")),
                ("/foo/bar/wibble?foo=lark", path("./wibble")),
                ("/foo/bar/wibble/lark?foo=lark", path("./wibble/lark")),
                ("/foo/bar/wibble/lark/../", path("./wibble")),
                ("/foo/bar/wibble/lark/../../", path(".")),
                ("/foo/bar/wibble/lark/../../../", path(".")),
            ]
        )
    }

    #[test]
    fn exact_paths() {
        test_route_matches(
            vec![
                ("=8080/foo/bar", "/1"),
                ("=8080/foo/bar/wibble", "/2"),
                ("=8080/foo", "/3"),
            ],
            vec![
               ("/foo/bar", path("/1")),
               ("/foo/bar/wibble", path("/2")),
               ("/foo", path("/3")),
               ("/foo/", none()),
               ("/", none()),
               ("/foo/bar/", none()),
               ("/foo/bar/more", none()),
               ("/foo/bar/wibble/lark", none()),
            ]
        )
    }

    #[test]
    fn path_patterns1() {
        test_route_matches(
            vec![
                ("8080/(foo)/bar", "/wibble/(foo)")
            ],
            vec![
                ("/", none()),
                ("/foo", none()),
                ("/foo/ba", none()),
                ("/foo/bar", path("/wibble/foo")),
                ("/lark/bar", path("/wibble/lark")),
                ("/lark/banana/bar", none()),
            ]
        )
    }

    #[test]
    fn path_patterns2() {
        test_route_matches(
            vec![
                ("8080/a/(foo..)/c", "/(foo)/end.txt")
            ],
            vec![
                ("/", none()),
                ("/a", none()),
                ("/a/ba", none()),
                ("/a/bar", none()),
                ("/a/bar/c", path("/bar/end.txt")),
                ("/a/bar/lark/c", path("/bar/lark/end.txt")),
                ("/a/a/a/c/c/c", path("/a/a/end.txt/c/c")), // greedy match, rest appended to end
                ("/a/?foo/c", none()),
            ]
        )
    }

    #[test]
    fn path_patterns3() {
        test_route_matches(
            vec![
                ("8080/a/(foo..)/(bar)/(lark..)/c", "/1/(foo)/2/(bar)/3/(lark)/end")
            ],
            vec![
                ("/", none()),
                ("/a/foo/bar/lark", none()),
                ("/a/foo/bar/lark/c", path("/1/foo/2/bar/3/lark/end")),
                // non-greedy, so last capture grabs all:
                ("/a/foo/bar/lark1/lark2/c", path("/1/foo/2/bar/3/lark1/lark2/end")),
                ("/a/foo/bar/lark1/lark2/lark3/c", path("/1/foo/2/bar/3/lark1/lark2/lark3/end")),
            ]
        )
    }

    #[test]
    fn path_patterns4() {
        test_route_matches(
            vec![
                ("8080/a/(foo..)/(bar)/(lark)/c", "/1/(foo)/2/(bar)/3/(lark)/end")
            ],
            vec![
                ("/", none()),
                ("/a/foo/bar/lark", none()),
                ("/a/foo/bar/lark/c", path("/1/foo/2/bar/3/lark/end")),
                // First pattern has to grab all in order for routes to match:
                ("/a/foo/foo2/bar/lark/c", path("/1/foo/foo2/2/bar/3/lark/end")),
                ("/a/foo/foo2/foo3/bar/lark/c", path("/1/foo/foo2/foo3/2/bar/3/lark/end")),
            ]
        )
    }

    #[test]
    fn urls1() {
        test_route_matches(
            vec![
                ("1010", "9090"),
                (":2020/2", "9090/2"),
                ("localhost:3030/3", "9090/3"),
                ("localhost:4040/4", "localhost:9090/4"),
                ("0.0.0.0:4040/5", ":9090/5"),
                ("http://0.0.0.0:4040/6", "http://localhost:9090/6"),
                ("http://0.0.0.0/7", "http://localhost/7"),
            ],
            vec![
                ("/", url("http://localhost:9090/")),
                ("/2", url("http://localhost:9090/2")),
                ("/3", url("http://localhost:9090/3")),
                ("/4", url("http://localhost:9090/4")),
                ("/5", url("http://localhost:9090/5")),
                ("/6", url("http://localhost:9090/6")),
                ("/7", url("http://localhost/7")),
            ]
        )
    }

    #[test]
    fn url_src_query_params() {
        test_route_matches(
            vec![
                // Query params are currently ignored in sources,
                // but hopefully this will change:
                ("1010/1?foo=2", "9090/1"),
                ("1010/2?foo=2", "9090/2?lark=wibble"),
            ],
            vec![
                ("/1", url("http://localhost:9090/1")),
                ("/1/a/b", url("http://localhost:9090/1/a/b")),
                ("/2/a/b", url("http://localhost:9090/2/a/b?lark=wibble")),
                ("/2/a/b?foo=bar", url("http://localhost:9090/2/a/b?lark=wibble&foo=bar")),
            ]
        )
    }

    #[test]
    fn urls_query_params() {
        test_route_matches(
            vec![
                ("1010/1", "9090/1?foo=bar"),
                ("1010/2/(foo)/bar", "9090/2?foo=(foo)"),
                ("1010/3", "9090/3"),
            ],
            vec![
                ("/1", url("http://localhost:9090/1?foo=bar")),
                // Query params that are part of the destination route
                // currently override those that are provided:
                ("/1?foo=wibble", url("http://localhost:9090/1?foo=bar")),
                ("/1?foo=wibble&lark=2", url("http://localhost:9090/1?foo=bar&lark=2")),
                ("/1?foo=wibble&lark=2&boom", url("http://localhost:9090/1?foo=bar&lark=2&boom")),
                ("/1?foo=wibble&lark=2&boom&wobble", url("http://localhost:9090/1?foo=bar&lark=2&boom&wobble")),
                ("/1?lark=2&boom&wobble&foo", url("http://localhost:9090/1?foo=bar&lark=2&boom&wobble")),
                // Query params are expanded from patterns as well:
                ("/2/fooey/bar", url("http://localhost:9090/2?foo=fooey")),
                ("/2/wobbly/bar?lark=2", url("http://localhost:9090/2?foo=wobbly&lark=2")),
                // Query params provided to routes with none already should be ok:
                ("/3?foo", url("http://localhost:9090/3?foo")),
                ("/3?foo&bar=2", url("http://localhost:9090/3?foo&bar=2")),
                ("/3?foo=bar&wobble=wibble", url("http://localhost:9090/3?foo=bar&wobble=wibble")),
            ]
        )
    }

    #[test]
    fn exact_urls() {
        test_route_matches(
            vec![
                ("=8080/foo/bar", "9090/1"),
                // This path is longer, and so can accidentally be sorted
                // before the above if path length is taken into account
                // when it shouldn't be:
                ("=8080/favicon.ico", "9090/2"),
            ],
            vec![
                ("/foo/bar", url("http://localhost:9090/1")),
                ("/favicon.ico", url("http://localhost:9090/2")),
                // These aren't exact matches and so should fail to match:
                ("/foo/bar/", none()),
                ("/foo/bar/wibble", none()),
                ("/favicon.ico/", none()),
                ("/favicon.ico/wibble", none()),
            ]
        )
    }

    #[test]
    fn dont_add_trailing_slash_to_exact_match() {
        test_route_matches(
            vec![
                ("8080/hello/bar", "9090/wibble/bar"),
                ("8080/hello/bar.json", "9090/wibble/bar.json"),
                ("=8080/hello/wibble", "9090/hi/wibble"),
                ("=8080/hello/wibble.json", "9090/hi/wibble.json"),
            ],
            vec![
                ("/hello/bar", url("http://localhost:9090/wibble/bar")),
                ("/hello/bar.json", url("http://localhost:9090/wibble/bar.json")),
                ("/hello/wibble", url("http://localhost:9090/hi/wibble")),
                ("/hello/wibble.json", url("http://localhost:9090/hi/wibble.json")),
            ]
        )
    }

    #[test]
    fn match_first_available_regex_pattern() {
        test_route_matches(
            vec![
                ("8080/(foo)/bar", "9090/bar/(foo)/1"),
                // This is longer but should not be sorted first, as we
                // preserve the order of paths with patterns:
                ("8080/(foo)/(bar)", "9090/(bar)/(foo)/2"),
            ],
            vec![
                ("/hello/bar", url("http://localhost:9090/bar/hello/1")),
            ]
        )
    }

    #[test]
    fn match_exact_regex_over_prefix() {
        test_route_matches(
            vec![
                ("8080/hello/bar/", "9090/wibble/0/"),
                // This regex path should be picked, because exact regex routes
                // should always match over prefix routes:
                ("=8080/(hello)/(bar)/wibble", "9090/wibble/1/"),
            ],
            vec![
                ("/hello/bar/wibble", url("http://localhost:9090/wibble/1/")),
            ]
        )
    }

    #[test]
    fn match_exact_over_prefix() {
        test_route_matches(
            vec![
                ("8080/foo", "9090/1"),
                // This shorter but exact route should be picked:
                ("=8080/foo", "9090/2"),
            ],
            vec![
                ("/foo", url("http://localhost:9090/2")),
            ]
        )
    }

    #[test]
    fn mixed_urls() {
        test_route_matches(
            vec![
                // The first route is not regex based; this should be ignored
                // in favour of exact regex ones where applicable:
                ("8080/hello/bar/", "9090/wibble/0/"),
                // Regex based but *not* exact (no trailing '='), so should
                // be less specific than all of the below:
                ("8080/(foo)/bar", "9090/bar/(foo)/nonexact"),
                ("=8080/(foo)/bar", "9090/bar/(foo)/1"),
                // Multiple captures helps test that we have built up the
                // right regex to match on in the first place, since greediness
                // can lead to only the last capture being spotted:
                ("=8080/(foo)/(bar)", "9090/(bar)/(foo)/2"),
                ("=8080/(foo)/(bar)/wibble", "9090/wibble/(bar)/(foo).json3"),
                // This should capture anything with at least one '/' in the middle:
                ("=8080/(foo..)/(bar)/boom", "9090/boom/(bar)/(foo)/4"),
                // This should capture anything with 'BOOM' in the middle
                ("=8080/(foo..)/BOOM/(bar..)", "9090/(foo)/exploding/(bar)"),
            ],
            vec![
                ("/hello/bar", url("http://localhost:9090/bar/hello/1")),
                ("/hello/baz", url("http://localhost:9090/baz/hello/2")),
                ("/hello/bar/wibble", url("http://localhost:9090/wibble/bar/hello.json3")),
                // Should fall back to the non-regex route:
                ("/hello/bar/lark", url("http://localhost:9090/wibble/0/lark")),
                // Should fall back to the first route with '..'s:
                ("/foo/bar/lark/wibble/boom", url("http://localhost:9090/boom/wibble/foo/bar/lark/4")),
                // We can replace single parts of a path using multiple '..'s:
                ("/1/2/3/BOOM/4/5", url("http://localhost:9090/1/2/3/exploding/4/5")),
                ("/1/BOOM/2/3/4/5", url("http://localhost:9090/1/exploding/2/3/4/5")),
                // We can match the non-exact regex only if the route matches nothing else:
                ("/foo/bar/lark/wibble", url("http://localhost:9090/bar/foo/nonexact/lark/wibble")),
            ]
        )
    }

}