# Change Log

## [0.2.0] - 2026-10-29

### New

- Six new centralities besides PageRank, selectable from the Centrality menu
  or with Cmd-Alt-1 to Cmd-Alt-7, in three families: Katz's index, with its
  exact limit as βρ → 1 (damped spectral); Seeley's index and eigenvector
  centrality (power method); and closeness, harmonic centrality, and
  betweenness (geometric and path-based).

- Plots of the iterates of the power methods against the iteration *k*,
  showing oscillations on periodic structures, with a note when the
  iterates of eigenvector centrality vanish.

- Shading of the dominant classes for Katz's index and eigenvector
  centrality, and the name of the current centrality over the plot.

- Faded curves from before the last edit that survive a switch of
  centrality, so that the effect of an edit can be compared across
  centralities.

- Saved graphs record the current centrality and the parameter of Katz's
  index; files saved by 0.1.0 open with PageRank.

- Nodes whose score is exactly zero have a thicker gray outline.

- An Examples section in the README, with a commented screenshot of each
  centrality.

### Changed

- PageRank Explorer is now Centrality Explorer (crate and binary
  `centrality-explorer`).

- The arrow keys move the parameter of the current centrality.

## [0.1.0] - 2026-10-29

### New

- Interactive editing of directed and undirected graphs: nodes, edges,
  self-loops, labels, and positions, with unlimited undo and redo.

- Exact PageRank curves as a function of the damping factor, including the
  exact limit at α = 1.

- Node colors shared with their curves, node sizes showing the PageRank at
  the current α, and faded curves from before the last edit.

- Shading of sinks (terminal strongly connected components other than lone
  dangling nodes), and marking of dangling nodes.

- Directed Chung–Lu random graphs with power-law degrees.

- Force-directed, circle, and components layouts, with animated transitions.

- Saving and loading graphs as JSON.
