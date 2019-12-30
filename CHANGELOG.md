# 0.3.2

## Improvements

- Bump dependencies off alpha versions.
- Use the stable Rust compiler now that `async`/`await` is stable.

# 0.3.1

## Improvements

- Bump compiler version to the latest Rust beta.
- Bump dependencies.
- Improve example output in `--help`.

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

- More thorough examples are now provided on `-h`, including exampels of the routing for each.

# 0.2.0

## Additions

- Add support for route patterns eg. `weave '8080/(foo)/bar' to '9090/bar/(foo)'` ([#1](https://github.com/jsdw/weave/pull/1)).
- Add support for exact route matches eg. `weave '=8080/favicon.ico' to './favicon.ico'` ([#1](https://github.com/jsdw/weave/pull/1)).

# 0.1.0

Initial release with basic routing
