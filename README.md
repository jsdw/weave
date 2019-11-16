[![Build Status](https://travis-ci.org/jsdw/weave.svg?branch=master)](https://travis-ci.org/jsdw/weave)

# Weave

A simple CLI based HTTP/TCP router/proxy. Useful if you need to wire together a few things and expose them behind a single host/port, or just as a fast, single-binary alternative to `php -s` or `static-server`. Also useful if you need to proxy TCP traffic to another location.

# Examples

Forward TCP connections from `localhost:2222` to `1.2.3.4:22`:
```
weave tcp://localhost:2222 to 1.2.3.4:22
```

Serve static files from the current directory on `localhost:8080`:
```
weave 8080 to .
```

Serve static files from `./client/files` on `localhost:8080`, and redirect HTTP requests starting with `localhost:8080/api` to `localhost:9090`:
```
weave 8080 to ./client/files and 8080/api to 9090
# Examples of routing given the above:
# http://localhost:8080/api/foo => http://localhost:9090/foo
# http://localhost:8080/api/bar/wibble => http://localhost:9090/bar/wibble
# http://localhost:8080/ => ./client/files/index.html
# http://localhost:8080/somefile => ./client/files/somefile
# http://localhost:8080/path/to/somefile => ./client/files/path/to/somefile
```

Visit google by navigating to `localhost:8080`:
```
weave 8080 to https://www.google.com
# Examples of routing given the above:
# http://localhost:8080/ => https://www.google.com/
# http://localhost:8080/favicon.ico => https://www.google.com/favicon.ico
# http://localhost:8080/favicon.ico/bar => https://www.google.com/favicon.ico/bar
```

Visit google by navigating to `localhost:8080/foo`:
```
weave 8080/foo to https://www.google.com
# Examples of routing given the above:
# http://localhost:8080/ => No route matches this
# http://localhost:8080/foo => https://www.google.com
# http://localhost:8080/foo/favicon.ico => https://www.google.com/favicon.ico
```

Serve files in your cwd by navigating to `0.0.0.0:8080` (makes them available to anything that can see your machine):
```
weave 0.0.0.0:8080 to ./
# Examples of routing given the above:
# http://0.0.0.0:8080/ => ./index.html
# http://0.0.0.0:8080/somefile => ./somefile
# http://0.0.0.0:8080/path/to/somefile => ./path/to/somefile
```

Serve exactly `/favicon.ico` using a local file, but the rest of the site via `localhost:9000`:
```
weave =8080/favicon.ico to ./favicon.ico and 8080 to 9090
# Examples of routing given the above:
# http://localhost:8080/ => http://localhost:9090
# http://localhost:8080/favicon.ico => ./favicon.ico
# http://localhost:8080/favicon.ico/bar => http://localhost:9090/favicon.ico/bar
```

Match any API version provided and move it to the end of the destination path:
```
weave '8080/(version)/api' to 'https://some.site/api/(version)'
# Examples of routing given the above:
# http://localhost:8080/v1/api => https://some.site/api/v1
# http://localhost:8080/v1/api/foo => https://some.site/api/v1/foo
# http://localhost:8080/wibble/api/foo => https://some.site/api/wibble/foo
```

Serve JSON files in a local folder as exactly `api/(filename)/v1` to mock a simple API:
```
weave '=8080/api/(filename)/v1' to './files/(filename).json'
# Examples of routing given the above:
# http://localhost:8080/api/foo/v1 => ./files/foo.json
# http://localhost:8080/api/bar/v1 => ./files/bar.json
# http://localhost:8080/api/bar/v1/wibble => No route matches this
```

Match paths ending in `/api/(filename)` and serve up JSON files from a local folder:
```
weave '=8080/(base..)/api/(filename)' to './files/(filename).json'
# Examples of routing given the above:
# http://localhost:8080/1/2/3/api/foo => ./files/foo.json
# http://localhost:8080/wibble/api/foo => ./files/foo.json
# http://localhost:8080/bar/api/foo => ./files/foo.json
# http://localhost:8080/api/foo => No route matches this
```

`and` can be used to serve any number of routes simultaneously. Keep reading for more information on the different types of routes, and how they are prioritised.

# Installation

## From pre-built binaries

Prebuilt compressed binaries are available [here](https://github.com/jsdw/weave/releases/latest). Download the compressed `.tar.gz` file for your OS/architecture and decompress it (on MacOS, this is automatic if you double-click the downloaded file).

If you like, you can download and decompress the latest release on the commandline. On **MacOS**, run:

```
curl -L https://github.com/jsdw/weave/releases/download/v0.4.0/weave-v0.4.0-x86_64-apple-darwin.tar.gz | tar -xz
```

For **Linux**, run:

```
curl -L https://github.com/jsdw/weave/releases/download/v0.4.0/weave-v0.4.0-x86_64-unknown-linux-musl.tar.gz | tar -xz
```

In either case, you'll end up with a `weave` binary in your current folder. The examples assume that you have placed this into your `$PATH` so that it can be called from anywhere.

## From source

Alternately, you can compile `weave` from source.

First, go to [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install) and install Rust.

Then to install a release of `weave` (here, v0.4.0), run the following:

```
cargo install --git https://github.com/jsdw/weave.git --tag v0.4.0 --force
```

This installs the latest version of `weave` into a local `.cargo/bin` folder that the rust installation will have prompted you to add to your `$PATH`. The `--force` command overwrites any existing `weave` binary in this folder; you can ditch it if you don't want this behaviour.

# More Information on routing

## Prefix routes

Basic routes like `8080/foo` will match any incoming path whose _prefix_ is the same. Thus, `8080/foo` matches requests to `/foo`, but also `/foo/bar`, `/foo/bar/wibble` and so on.

## Exact routes

If you'd like to match an exact path only, prefix the source route with `=`. `=8080/foo` matches requests to exactly `/foo` and nothing else.

## Route patterns

To match on any path fragment provided, you can declare a variable using parentheses. `8080/(foo)/bar` matches `/lark/bar`, `/wibble/bar`, `/lark/bar/foo` and so on. To force exact matching only, as above we can prefix the route with `=`. `=8080/(foo)/bar` will match `/lark/bar` and `/wibble/bar` but not `/lark/bar/foo`. Variables must be basic alphanumeric strings beginning with an ascii letter (numbers, '-' and '_' are allowed in the rest of the string).

To capture as much of the route as possible, including separating `/`s, you can use a _dotdot_ variable in a path. `8080/(foo..)/bar` will match `/1/bar`, `/1/2/3/bar`, `/1/2/3/bar/4/5` and so on. Once again, prefix the route with `=` for exact matching only. `=8080/(foo..)/bar` will match `/1/bar` and `/1/2/3/bar` but not `/1/2/3/bar/4/5`.

The variables declared in parentheses in these source paths can be used in the destination paths too, as you might expect. See the examples for some uses of this.

You can combine uses of `(var1..)` and `(var2)`, and have multiple of each in a given route, but be aware that if there is ambiguity in which part of the route matches which variable, you cannot rely on the variabels containing what you expect.

## Route ordering

If you combine multiple routes using `and`, they will be sorted in this order:

1. Exact match routes
2. Exact match routes with route patterns
3. Prefix routes
4. Prefix routes with route patterns

Within these groups, exact match routes and prefix routes are then sorted longest (most specific) first. routes with route patterns are sorted by the order in which they were declared.

When matching an incoming request, the first route that matches wins, and the request is redirected to the destination given with that route. This should generally lead to requests being redirected as you would expect; more specific matches will tend to win over less specific matches.

# Known Issues

- Untested on windows, so (at the very least) serving from file paths may not work as expected.
