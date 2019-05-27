# Weave

A simple CLI router. Useful if you need to wire together a few things and expose them behind a single host/port.

Usage looks a bit like this:

```
weave 8080 to ./client/files and 8080/api to 9090
```

This command proxies requests to `localhost:8080/api/*` over to `localhost:9090/*`, and also serves files from `./client/files` on `localhost:8080`. For a given request, it picks the most specific match it can find fro mwhat's been given and uses that.

You can also specify exact routes and pattern match on parts of a route. See the examples below for more!

# Installation

Currently, to install `weave` you'll need to build it from source.

First, go to [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install) and install Rust.

Next, run these commands to download and use the correct nightly version of the language:

```
rustup toolchain install nightly-2019-05-21
rustup default nightly-2019-05-21
```

Finally, to install `weave` (v0.1), run the following:

```
cargo install --git https://github.com/jsdw/weave.git --tag v0.1 --force
```

This installs the latest version of `weave` into a local `.cargo/bin` folder that the rust installation will have prompted you to add to your `$PATH`. The `--force` command overwrites any existing `weave` binary in this folder; you can ditch it if you don't want this behaviour.

# Motivation

## Throwing together client code and some API you've been hacking on

One motivating example is that you have built some static HTML/JavaScript, and separately want to start hacking on an API that it can talk to. You might serve your static files on `localhost:8080` to view them using something like `php -S`, but where do you serve the API so that the static content can make requests to it when loaded into a browser (without hitting cross origin restrictions)?

Using `weave`, you'd just start your API server up on, say, port 9090, and then `weave 8080 to ./client/files and 8080/api to 9090` to merge the client and API under one hostname, `localhost:8080`. Now, when somebody navigates to `localhost:8080`, `index.html` is loaded,and that can talk to `/api/foo` to communicate with your API server.

## Merging several APIs under one unified interface

Taking the above example a step further, perhaps you have some client code that you want to be able to communicate with several small APIs that are under your control (perhaps you're building a dashboard, for instance). To get stuck in quickly, you may want to avoid writing your own server that can proxy requests to the various endpoints, and use `weave` to throw together a common interface that your client code can talk to:

```
weave \
    8080/api/stats to 10.10.0.21:9091/api/get_stats and \
    8080/api/build_failures to internal.system/get_build_failures and \
    8080 to ./client/file/dir/
```

Example usage to follow:

# Routing

## Prefix routes

Basic routes like `8080/foo` will match any incoming path whose _prefix_ is the same. Thus, `8080/foo` matches requests to `/foo`, but also `/foo/bar`, `/foo/bar/wibble` and so on.

## Exact routes

If you'd like to match an exact path only, prefix the source route with `=`. `=8080/foo` matches requests to exactly `/foo` and nothing else.

## Route patterns

To match on any path fragment provided, you can declare a variable using parentheses. `8080/(foo)/bar` matches `/lark/bar`, `/wibble/bar`, `/lark/bar/foo` and so on. To force exact matching only, as above we can prefix the route with `=`. `=8080/(foo)/bar` will match `/lark/bar` and `/wibble/bar` but not `/lark/bar/foo`. Variables must be basic alphanumeric strings beginning with an ascii letter (numbers, '-' and '_' are allowed in the rest of the string).

To capture as much of the route as possible, including separating `/`s, you can use a _dotdot_ variable in a path. `8080/(foo..)/bar` will match `/1/bar`, `/1/2/3/bar`, `/1/2/3/bar/4/5` and so on. Once again, prefix the route with `=` for exact matching only. `=8080/(foo..)/bar` will match `/1/bar` and `/1/2/3/bar` but not `/1/2/3/bar/4/5`.

The variables declared in parentheses in these source paths can be used in the destination paths too, as you might expect. See the examples for some uses of this.

You can combine uses of `(var1..)` and `(var2)`, and have multiple of each in a given route, but be aware that if there is ambiguity in which part of the route matches which variable, you cannot rely on the variabels containing what you expect.

# Examples

Visit google by navigating to `localhost:8080`:
```
weave 8080 to https://www.google.com
```

Visit google by navigating to `localhost:8080/foo`:
```
weave 8080/foo to https://www.google.com
```

Serve files in your cwd by navigating to `0.0.0.0:8080` (makes them available to anything that can see your machine):
```
weave 0.0.0.0:8080 to ./
```

Serve files in your cwd by navigating to `0.0.0.0:8080/files` and visit google by navigating to `0.0.0.0:8080/google`:
```
weave 0.0.0.0:8080/files to ./ and 0.0.0.0:8080/google to https://www.google.com
```

Serve exactly `/favicon.ico` using a local file, but the rest of the site via `localhost:9000`:
```
weave =8080/favicon.ico to ./favicon.ico and 8080 to 9090
```

Match any API version provided and move it to the end of the destination path:
```
weave '8080/(version)/api' to '8080/api/(version)'
```

Serve JSON files in a local folder as exactly `api/(filename)/v1` to mock a simple API:
```
weave '=8080/api/(filename)/v1' to './files/(filename).json'
```

Match paths ending in `/api/(filename)` and serve up JSON files from a local folder:
```
weave '=8080/(base..)/api/(filename)' to './files/(filename).json'
```
