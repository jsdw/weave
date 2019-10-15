use colored::*;
use lazy_static::lazy_static;
use regex::{ Regex, Captures};

/// This examples text is appended to the help output.
pub fn text() -> String {
    prettify_code(&format!("{EXAMPLES}

Serve static files from `./client/files` on `localhost:8080`, and redirect HTTP
requests starting with `localhost:8080/api` to `localhost:9090`:

{example1a}
{example1b}

Visit google by navigating to `localhost:8080`:

{example2a}
{example2b}

Visit google by navigating to `localhost:8080/foo`:

{example3a}
{example3b}

Serve files in your cwd by navigating to `0.0.0.0:8080` (makes them available to
anything that can see your machine):

{example4a}
{example4b}

Serve exactly `/favicon.ico` using a local file, but the rest of the site via
`localhost:9000`:

{example5a}
{example5b}

Match any API version provided and move it to the end of the destination path:

{example6a}
{example6b}

Serve JSON files in a local folder as exactly `api/(filename)/v1` to mock a
simple API:

{example7a}
{example7b}

Match paths ending in `/api/(filename)` and serve up JSON files from a
local folder:

{example8a}
{example8b}

`and` can be used to serve any number of routes simultaneously.

",
    EXAMPLES="EXAMPLES:".bold(),

    example1a="weave 8080 to ./client/files and 8080/api to 9090".cyan(),
    example1b="# Examples of routing given the above:
# http://localhost:8080/api/foo => http://localhost:9090/foo
# http://localhost:8080/api/bar/wibble => http://localhost:9090/bar/wibble
# http://localhost:8080/ => ./client/files/index.html
# http://localhost:8080/somefile => ./client/files/somefile
# http://localhost:8080/path/to/somefile => ./client/files/path/to/somefile".white(),

    example2a="weave 8080 to https://www.google.com".cyan(),
    example2b="# Examples of routing given the above:
# http://localhost:8080/ => https://www.google.com/
# http://localhost:8080/favicon.ico => https://www.google.com/favicon.ico
# http://localhost:8080/favicon.ico/bar => https://www.google.com/favicon.ico/bar".white(),

    example3a="weave 8080/foo to https://www.google.com".cyan(),
    example3b="# Examples of routing given the above:
# http://localhost:8080/ => No route matches this
# http://localhost:8080/foo => https://www.google.com
# http://localhost:8080/foo/favicon.ico => https://www.google.com/favicon.ico".white(),

    example4a="weave 0.0.0.0:8080 to ./".cyan(),
    example4b="# Examples of routing given the above:
# http://0.0.0.0:8080/ => ./index.html
# http://0.0.0.0:8080/somefile => ./somefile
# http://0.0.0.0:8080/path/to/somefile => ./path/to/somefile".white(),

    example5a="weave =8080/favicon.ico to ./favicon.ico and 8080 to 9090".cyan(),
    example5b="# Examples of routing given the above:
# http://localhost:8080/ => http://localhost:9090
# http://localhost:8080/favicon.ico => ./favicon.ico
# http://localhost:8080/favicon.ico/bar => http://localhost:9090/favicon.ico/bar".white(),

    example6a="weave '8080/(version)/api' to 'https://some.site/api/(version)'".cyan(),
    example6b="# Examples of routing given the above:
# http://localhost:8080/v1/api => https://some.site/api/v1
# http://localhost:8080/v1/api/foo => https://some.site/api/v1/foo
# http://localhost:8080/wibble/api/foo => https://some.site/api/wibble/foo".white(),

    example7a="weave '=8080/api/(filename)/v1' to './files/(filename).json'".cyan(),
    example7b="# Examples of routing given the above:
# http://localhost:8080/api/foo/v1 => ./files/foo.json
# http://localhost:8080/api/bar/v1 => ./files/bar.json
# http://localhost:8080/api/bar/v1/wibble => No route matches this".white(),

    example8a="weave '=8080/(base..)/api/(filename)' to './files/(filename).json'".cyan(),
    example8b="# Examples of routing given the above:
# http://localhost:8080/1/2/3/api/foo => ./files/foo.json
# http://localhost:8080/wibble/api/foo => ./files/foo.json
# http://localhost:8080/bar/api/foo => ./files/foo.json
# http://localhost:8080/api/foo => No route matches this".white(),

    ))
}

/// Find pairs of `'s and colourise the text contained within:
fn prettify_code(s: &str) -> String {
    lazy_static!(
        static ref CODE_BLOCK: Regex = Regex::new("`([^`]+)`").unwrap();
    );
    CODE_BLOCK.replace_all(s, |caps: &Captures| {
        let code = caps.get(1).unwrap().as_str();
        code.cyan().to_string()
    }).to_string()
}