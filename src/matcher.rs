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
    use crate::location::{ DestLocation };

    use super::*;

    fn uri (s: &str) -> Uri { s.parse().unwrap() }
    fn resolved_url (u: &str) -> ResolvedLocation { ResolvedLocation::Url(u.to_owned()) }

    #[test]
    fn exact_prefix_means_exact() {
        let routes = vec![
            Route {
                src: "=8080/foo/bar".parse().unwrap(),
                dest: DestLocation::parse("9090/1").unwrap()
            },
            // This path is longer, and so can accidentally be sorted
            // before the above if path length is taken into account
            // when it shouldn't be:
            Route {
                src: "=8080/favicon.ico".parse().unwrap(),
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
                src: "8080/hello/bar".parse().unwrap(),
                dest: DestLocation::parse("9090/wibble/bar").unwrap()
            },
            Route {
                src: "8080/hello/bar.json".parse().unwrap(),
                dest: DestLocation::parse("9090/wibble/bar.json").unwrap()
            },
            Route {
                src: "=8080/hello/wibble".parse().unwrap(),
                dest: DestLocation::parse("9090/hi/wibble").unwrap()
            },
            Route {
                src: "=8080/hello/wibble.json".parse().unwrap(),
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
                src: "8080/(foo)/bar".parse().unwrap(),
                dest: DestLocation::parse("9090/bar/(foo)/1").unwrap()
            },
            // This path is longer, and so can accidentally be sorted
            // before the above if path length is taken into account
            // when it shouldn't be:
            Route {
                src: "8080/(foo)/(bar)".parse().unwrap(),
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
                src: "8080/hello/bar/".parse().unwrap(),
                dest: DestLocation::parse("9090/wibble/0/").unwrap()
            },
            // This regex path should be picked, because exact regex routes
            // should always match over prefix routes:
            Route {
                src: "=8080/(hello)/(bar)/wibble".parse().unwrap(),
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
                src: "8080/foo".parse().unwrap(),
                dest: DestLocation::parse("9090/1").unwrap()
            },
            // This shorter but exact route should be picked:
            Route {
                src: "=8080/foo".parse().unwrap(),
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
                src: "8080/hello/bar/".parse().unwrap(),
                dest: DestLocation::parse("9090/wibble/0/").unwrap()
            },
            // Regex based but *not* exact (no trailing '='), so should
            // be less specific than all of the below:
            Route {
                src: "8080/(foo)/bar".parse().unwrap(),
                dest: DestLocation::parse("9090/bar/(foo)/nonexact").unwrap()
            },
            Route {
                src: "=8080/(foo)/bar".parse().unwrap(),
                dest: DestLocation::parse("9090/bar/(foo)/1").unwrap()
            },
            // Multiple captures helps test that we have built up the
            // right regex to match on in the first place, since greediness
            // can lead to only the last capture being spotted:
            Route {
                src: "=8080/(foo)/(bar)".parse().unwrap(),
                dest: DestLocation::parse("9090/(bar)/(foo)/2").unwrap()
            },
            Route {
                src: "=8080/(foo)/(bar)/wibble".parse().unwrap(),
                dest: DestLocation::parse("9090/wibble/(bar)/(foo).json3").unwrap()
            },
            // This should capture anything with at least one '/' in the middle:
            Route {
                src: "=8080/(foo..)/(bar)/boom".parse().unwrap(),
                dest: DestLocation::parse("9090/boom/(bar)/(foo)/4").unwrap()
            },
            // This should capture anything with 'BOOM' in the middle
            Route {
                src: "=8080/(foo..)/BOOM/(bar..)".parse().unwrap(),
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