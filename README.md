# Weave

A simple CLI router. Useful if you need to wire together a few things and expose them behind a single hostname.

Currently, to install `weave` you'll need to go to [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install) first and install Rust. Once that's installed, you can install or upgrade the latest version of `weave` by running:

```
cargo install --git https://github.com/jsdw/weave.git --force
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

# Building from source

Given that `rust` has been installed, we'll need a recent nightly compiler:

```
rustup toolchain install nightly-2019-05-21
rustup default nightly-2019-05-03
```

Then, to build with release optimisations:

```
cargo build --release
```