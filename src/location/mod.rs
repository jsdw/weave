mod src_location;
mod dest_location;

pub use src_location::*;
pub use dest_location::*;

#[cfg(test)]
mod test {

    use url::Url;
    use std::str::FromStr;
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