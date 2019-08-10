use regex::Regex;
use lazy_static::lazy_static;
use crate::errors::{ Error };

pub fn parse_protocol_host_port (input: &str) -> Result<ProtocolHostPort,Error> {
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
    if protocol != "http" {
        return Err(err!("Incalid protocol: expected 'http'"))
    }

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

    Ok(ProtocolHostPort {
        protocol,
        host,
        port,
        input
    })
}

pub struct ProtocolHostPort<'a> {
    pub protocol: &'a str,
    pub host: &'a str,
    pub port: u16,
    pub input: &'a str
}