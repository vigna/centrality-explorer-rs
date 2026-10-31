# Change Log

## [Unreleased]

### New

- Web build: the application compiles to WebAssembly (`web/build.sh`) as a
  single self-contained page, published to GitHub Pages by a workflow.

- Examples menu with 21 built-in graphs (17 directed, 4 undirected), loaded
  undoably and laid out for the current canvas; the web build starts on the
  first one.

### Improved

- `rand` is used without its default features, which removes the dependency on
  `getrandom`.

- `rfd` is a dependency of non-web targets only, and `-C target-cpu=native`
  is no longer applied to wasm32 builds.

## [0.1.0] - 2026-10-30

First release.
