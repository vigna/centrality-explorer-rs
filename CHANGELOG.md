# Change Log

## [0.1.2] - 2026-10-31

### Improved

- Buckets are no longer marked in undirected graphs, where they are just the
  connected components with an edge.

## [0.1.1] - 2026-10-31

### New

- Web build: the application compiles to WebAssembly (`web/build.sh`) as a
  single self-contained page, published to GitHub Pages by a workflow.

- Examples menu with 21 built-in graphs (17 directed, 4 undirected), loaded
  undoably and laid out for the current canvas; the web build starts on the
  first one. Hovering over an entry describes the dataset and what an arc
  means.

### Improved

- The random-graph dialog starts with 15 nodes instead of 10.

- `rand` is used without its default features, which removes the dependency on
  `getrandom`.

- `rfd` is a dependency of non-web targets only, and `-C target-cpu=native`
  is no longer applied to wasm32 builds.

## [0.1.0] - 2026-10-30

First release.
