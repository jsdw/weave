use lazy_static::lazy_static;
use regex::Regex;
use url::Host;
use std::borrow::Cow;
use crate::errors::{ Error };

/// Take something that looks a little like a URL and
/// find each part of the URL.
pub struct SplitUrl<'a> {
    pub protocol: &'a str,
    pub host: Host<String>,
    pub port: u16,
    pub path: Cow<'a, str>,
    pub query: &'a str
}

impl SplitUrl<'_> {
    pub fn parse<'a>(input: &'a str) -> Result<SplitUrl<'a>, Error> {
        lazy_static!{
            // Are we matching on parts of the path? (.*?) is a non greedy match, to match as little
            // as possible, which is necessary to support multiple match patterns.
            static ref HOST_AND_PORT_RE: Regex = Regex::new(r"^(.*):([0-9]+)$").expect("host_and_port_re");
        }

        // Did we specify an input protocol? It should be nothing or http
        let (protocol, input) = if let Some(n) = input.find("://") {
            (&input[0..n], &input[n+3..])
        } else {
            ("http", input)
        };

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

        // Host default to localhost if not provided:
        let host = Host::parse(if host.is_empty() { "localhost" } else { host })?;

        // Split remaining input into path and query parts:
        let (raw_path, query) = split_path_and_query(&input);

        // Normalise path if needed by adding prefix /:
        let path = if input.starts_with("/") {
            Cow::from(raw_path)
        } else {
            Cow::from(format!("/{}", raw_path))
        };

        Ok(SplitUrl {
            protocol,
            host,
            port,
            path,
            query
        })
    }
}

/// Split path_and_query into separate path and query pieces, noting that a '?' can appear
/// inside a pattern for src locations and so ignoring those.
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
