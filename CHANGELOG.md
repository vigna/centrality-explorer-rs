# Change Log

## [Unreleased]

### New

- View → ℓ1 normalization, checked by default, normalizes all centralities in
  ℓ1; unchecking it switches to the traditional normalization of each: none
  for the geometric centralities and Katz's index, ℓ2 for eigenvector
  centrality, and ℓ1 for Seeley's index and PageRank. The label of the y-axis
  states the normalization in effect, and node sizes always show ℓ1 shares.

- Without normalization, Katz's index diverges as βρ → 1: the y-axis then
  follows the scores at the current βρ, and the scores at βρ = 1 are infinite
  on the nodes reached by a dominant class.

- For eigenvector centrality and Seeley's index, Cesàro means, next to the
  slider, shows the means of the iterates of the power method (on A/ρ for
  eigenvector centrality), which converge to the centrality even when the
  iterates oscillate.

- The number K of iterations of the power method can be set from 1 to 1000
  (50 by default).

- Indegree (degree, on undirected graphs), the first centrality of the
  geometric family; self-loops count, so that it is 1A, the first iterate of
  the power method for eigenvector centrality.

- Graph → Rotate 90° Clockwise, Flip Horizontally, and Flip Vertically, about
  the center of the graph, animated and undoable.

- With ℓ1 normalization, a switch of centrality shows the previous centrality,
  on the current graph, as dashed curves, named by a note on the plot, until
  the next edit or switch: its curves, if the two centralities belong to the
  same family, and otherwise horizontal lines at its scores, at the current
  value of its parameter, if any.

### Changed

- The shortcuts of the centralities are Cmd-Alt-1 to Cmd-Alt-8, as indegree
  comes first.

- The x-axis of eigenvector centrality and Seeley's index is labeled “k
  (iterations of the power method)”.

- `rank::katz` returns Katz's index not normalized.

## [0.2.0] - 2026-10-29

### New

- Six new centralities besides PageRank, selectable from the Centrality menu
  or with Cmd-Alt-1 to Cmd-Alt-7, in three families: closeness, harmonic
  centrality, and betweenness (geometric); eigenvector centrality and
  Seeley's index (spectral, eigenvector); and Katz's index, with its exact
  limit as βρ → 1 (spectral, damped sum).

- For eigenvector centrality and Seeley's index, plots of the iterates of
  the power method computing them against the iteration *k*, showing
  oscillations on periodic structures, with a note when the iterates of
  eigenvector centrality vanish.

- Shading of the dominant classes for eigenvector centrality and Katz's
  index, and the name of the current centrality over the plot.

- Faded curves from before the last edit that survive a switch of
  centrality, so that the effect of an edit can be compared across
  centralities.

- Saved graphs record the current centrality and the parameter of Katz's
  index; files saved by 0.1.0 open with PageRank.

- Nodes whose score is exactly zero have a thicker gray outline.

- Dragging empty canvas moves the whole graph, keeping the nodes on the
  canvas; it is one undo step, like moving a node.

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
