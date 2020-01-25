# 0.5.1

## Improvements

- Fix example colouring for newly added examples.

# 0.5.0

## Additions

- allow "nothing" to be substituted for a route on the CLI (eg `weave nothing and 8080 to 9090`),
  to aid in basic scripted usage.
- allow HTTP sources to be redirected to HTTP status codes (eg `weave 8080 to statuscode://403`).
- support "nothing" as an alias for a 404 status code (eg `weave 8080 to nothing`).

# 0.4.1

## Improvements

- Bump tokio and associated dependencies to latest.

# 0.4.0

## Additions

- Add support for TCP proxying.

## Improvements

- Support stable Rust and bump a few dependencies

# 0.3.1

## Improvements

- Bump compiler version to the latest Rust beta.
- Bump dependencies.

# 0.3.0

## Improvements

- Large refactoring of internals to make it easier to make future changes.
- A bunch more tests.
- Better support for query parameters in matching and route resolution.

# 0.2.2

## Improvements

- Remove future compat things and migrate fully to `std::Future`s

# 0.2.1

## Improvements

- More thorough examples are now provided on `-h`, including examples of the routing for each.

# 0.2.0

## Additions

- Add support for route patterns eg. `weave '8080/(foo)/bar' to '9090/bar/(foo)'` ([#1](https://github.com/jsdw/weave/pull/1)).
- Add support for exact route matches eg. `weave '=8080/favicon.ico' to './favicon.ico'` ([#1](https://github.com/jsdw/weave/pull/1)).

# 0.1.0

Initial release with basic routing
