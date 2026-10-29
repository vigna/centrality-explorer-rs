# PageRank Explorer

A teaching aid to build intuition about PageRank centrality. The instructor
edits a small directed graph at a projector, and the class watches, in real
time, how the PageRank of each node changes as a function of the damping
factor α ∈ [0 . . 1].

The central picture is the plot of the PageRank of each node against α. When
an edit closes or opens a terminal strongly connected component, the curves
change shape all of a sudden, most visibly near α = 1, where the score
concentrates on the terminal components.

## Building and Running

```sh
cargo run --release
```

The crate is compiled with `-C target-cpu=native` (see `.cargo/config.toml`),
so build it on the machine that runs it: a binary built on a recent CPU may
not run on an older one.

## Reading the Screen

- The graph is on the left; the plot of the PageRank curves is on the right,
  with the α slider underneath.
- Each node has its own color, shared by its curve; a node keeps its color
  for as long as it exists.
- The size of a node shows its PageRank at the current α.
- The vertical line in the plot marks the current α, with a dot on each
  curve; node labels at the right end of the plot name the curves.
- After each edit, the curves from before the edit remain visible as faded
  dashed lines, until the next edit.
- Shaded hulls mark the terminal strongly connected components: groups of
  nodes with no edge leaving them (a dangling node is one by itself).
- Nodes with a dashed outline are dangling: they have no outgoing edges, so
  the walk jumps from them to a node chosen uniformly at random.

Hovering a node (or its curve) highlights the pair and shows the node's
score.

## Editing

| Action | Gesture |
|---|---|
| Add a node | Double-click on empty canvas |
| Add or remove an edge | Click the source node, then the target node |
| Remove an edge | Click it |
| Cancel a pending edge | Esc, or click on empty canvas |
| Move a node | Drag it |
| Rename a node | Type over it; Enter confirms, Esc cancels |
| Delete a node | Backspace or Delete over it |
| Self-loops, and everything else | Right-click menu |

## Shortcuts

Cmd is Ctrl on Linux and Windows.

| Command | Shortcut |
|---|---|
| New graph (directed or undirected) | Cmd-N |
| Open, Save, Save As | Cmd-O, Cmd-S, Cmd-Shift-S |
| Undo, Redo | Cmd-Z, Cmd-Shift-Z |
| Random graph | Cmd-R |
| Force-directed, circle, components layout | Cmd-1, Cmd-2, Cmd-3 |
| Zoom in, out, reset | Cmd-+, Cmd-−, Cmd-0 |
| Full screen | Cmd-Shift-F |
| Change α by 0.01 (0.05) | ← and → (with Shift) |

Undo and redo cover every change to the graph, so flipping between them is a
quick way to show the same graph before and after an edit.

Random graphs follow the directed Chung–Lu model, whose in-degrees and
out-degrees follow a power law. Sparse random graphs often have no closed
component containing a cycle, in which case PageRank at α = 1 vanishes on
no node (see below); add the missing structure by hand.

## The Mathematics

Dangling nodes jump uniformly to every node, and the preference vector is
uniform. For α < 1 the curves are computed exactly, by solving
(*I* − α *P*ᵀ) *x* = (1 − α) *v* at each plotted α, rather than through a
truncated power series, which is inaccurate near α = 1 on periodic
structures.

The value at α = 1 is the exact limit of PageRank as α → 1. If the graph has
a terminal component other than a lone dangling node, the limit is supported
exactly on such components: each receives the probability that a walk from
the uniform distribution is absorbed into it, spread according to its
stationary distribution, and every other node, including dangling ones, gets
zero. If instead every terminal component is a dangling node, the walk
restarts from them, and the limit is the stationary distribution of the walk,
which is positive on every node.
