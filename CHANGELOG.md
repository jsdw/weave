# 0.2.2

- Remove future compat things and migrate fully to `std::Future`s

# 0.2.1

- Update README and examples in binary

# 0.2

## Additions

- Add support for route patterns eg. `weave '8080/(foo)/bar' to '9090/bar/(foo)'` ([#1](https://github.com/jsdw/weave/pull/1)).
- Add support for exact route matches eg. `weave '=8080/favicon.ico' to './favicon.ico'` ([#1](https://github.com/jsdw/weave/pull/1)).

# 0.1

Initial release with basic routing