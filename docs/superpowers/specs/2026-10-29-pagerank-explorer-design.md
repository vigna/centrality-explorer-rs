# PageRank Explorer (Rust) — Design

Date: 2026-10-29

## Intent

PageRank Explorer is a teaching aid for building intuition about PageRank
centrality. The instructor drives it **at a projector** in front of a class:
they build a small directed graph live, add and remove edges, and the class
watches the PageRank scores react in real time.

The central moment of every lecture is the plot of **each node's PageRank as a
function of the damping factor α ∈ [0, 1]**. When an edit closes or opens a
terminal strongly connected component, the curves change shape all of a
sudden, most visibly near α = 1, where the score concentrates on the terminal
components. Everything else in the application (editing, colors, slider,
layouts, random graphs) exists to set up that moment and make it readable from
the back of a room.

This replaces an old Java implementation (`~/pub/java/pagerankexpl`, Swing +
prefuse + JPlot + LAW). The principles stay the same; the user interface is
redesigned. The two concrete problems with the old version that motivate the
rewrite:

1. **Pairing nodes with curves is a nightmare.** All curves are drawn in the
   same blue; node colors encode the score, not identity.
2. **The curves are inaccurate exactly where the effect lives.** The old code
   draws a Maclaurin polynomial truncated at degree 99. Near α = 1 the
   truncation can be visibly wrong, especially on periodic structures (e.g., a
   cycle), where the coefficients oscillate instead of decaying.

## Goals

- Every edit (add/remove node or edge, undo/redo, random graph) updates the
  curves, node sizes, and terminal-component shading **immediately**, with no
  recompute step and no perceptible lag.
- Curves are **exact** (to floating-point precision) on all of [0, 1],
  including the limit value at α = 1.
- Each node is **visually paired** with its curve by a shared, stable color.
- Legible on a projector from the back of a classroom.
- Editing gestures are simple, visible, and need no hidden modifier keys.

## Non-goals

- Large graphs. Classroom graphs have up to a few dozen nodes; the app must
  stay real-time up to about 100 nodes, and need not scale further.
- Weighted edges, parallel edges, non-uniform preference vectors
  (personalized PageRank), other centralities.
- A browser/WebAssembly build (the chosen toolkit permits one later; it is not
  part of this work).
- Importing graphs saved by the old Java application.
- Students running the app on their own machines (distribution, installers).

## Anti-goals (failure even if it technically works)

- Visible lag between an edit and the updated curves.
- Curves that are wrong or jagged near α = 1.
- Existing nodes changing color after an edit (this destroys the pairing the
  whole redesign is about).
- Nodes moving on their own while the instructor is editing.
- Text, strokes, or arrowheads too thin or small to read when projected.

## Constraints

- **Language:** Rust (stable, edition 2024).
- **GUI:** `eframe`/`egui`, with `egui_plot` for the curve plot (or custom
  painting if `egui_plot` gets in the way). Native desktop application; the
  instructor's machine is a Mac, but nothing should be macOS-specific.
- **Build flags:** the crate must have `.cargo/config.toml` containing

  ```toml
  [build]
  rustflags = ["-C", "target-cpu=native"]
  ```

  (mandatory for all of the user's Rust crates). Consequence: a binary built on
  one machine may not run on an older CPU, so the app should be built on the
  machine that runs it.
- **Coding conventions:** follow the user's Rust development guidelines (the
  `rust-guidelines` skill, from `github.com/vigna/rust-dev-guidelines`).
- `.superpowers/` and `.playwright-mcp/` (brainstorming artifacts) go in
  `.gitignore`.

## The mathematics

### PageRank

For a graph with *n* nodes, let *P* be the row-stochastic transition matrix:
a node with out-degree *d* > 0 gives probability 1/*d* to each successor;
a **dangling node** (out-degree 0) gives probability 1/*n* to every node
(uniform jump — the same behavior as the old version, which used LAW's
defaults). The preference vector *v* is uniform, *v* = (1/*n*, …, 1/*n*).

PageRank at damping α ∈ [0, 1) is the row vector

  r(α) = (1 − α) · v · (I − α P)⁻¹,

i.e., the solution of the linear system (I − α Pᵀ) xᵀ = (1 − α) vᵀ. It sums
to 1 for every α. For an undirected graph, each undirected edge is treated as
a pair of opposite arcs.

### The curves

Compute r(α) by direct dense linear solves (e.g., LU with partial pivoting) at
a grid of α values in [0, 1), plus the exact limit at α = 1 (below). The grid
must be fine enough that the curves look smooth at full-screen size (at least
a few hundred points; the builder may refine near α = 1). Each r(α) is a
rational function of α with no pole on [0, 1], so this sampling is accurate.
Conditioning of I − αP degrades like 1/(1 − α), which is harmless in `f64` for
the α values sampled.

Recompute all curves once per edit, never per frame. For *n* ≤ 100 a full
recompute must take well under one frame budget on the instructor's machine;
the builder may move it off the UI thread if needed, but a synchronous solve
is expected to suffice.

The α slider does **not** change the curves. Node sizes at the current α come
from one extra solve at exactly that α (cheap), not from interpolation.

### The limit at α = 1

The limit r* = lim_{α→1⁻} r(α) exists and equals v·P*, where P* is the Cesàro
limit of the powers of *P*. Compute it exactly from the structure of the chain
(with dangling rows already replaced by uniform rows):

1. Find the strongly connected components of the graph induced by the nonzero
   entries of *P*; the **terminal** components are those with no arc leaving
   them.
2. For each terminal component C, compute its stationary distribution π_C
   (the chain restricted to C is irreducible).
3. Compute the absorption probabilities a_C: the probability that a walk whose
   starting node is drawn from *v* is eventually absorbed into C (a linear
   system on the transient nodes).
4. r* = Σ_C a_C · π_C.

Note that r* is supported exactly on the nodes of terminal components — this
is the effect the lecture is about.

**Subtlety with dangling nodes.** Because dangling rows are uniform, a dangling
node links to every node. Terminal components must therefore be computed on
the chain actually walked (the patched one): a lone dangling node is never a
terminal component, and a graph whose only "sinks" are dangling nodes may have
a single, large terminal component. The terminal-component shading in the UI
(below) does **not** use the patched chain: it shows the terminal components
of the graph as drawn (user's decision, see below).

### Tests for the math

The math lives in a GUI-free module with unit tests, at minimum:

- r(α) sums to 1 for all sampled α and all test graphs.
- A directed cycle gives the uniform vector for every α, including α = 1.
- The computed limit r* matches r(1 − ε) for small ε on assorted graphs,
  including periodic ones and graphs with dangling nodes.
- Known small cases computed by hand (e.g., the five-node example below:
  after closing {d, e}, r* = (0, 0, 0, ½, ½)).
- Terminal-component detection on graphs with and without dangling nodes.

## The screen

Side-by-side layout sized for a 16:9 projector, readable at 1280×720:

- **Left: the graph canvas.**
- **Right: the curve plot**, with the **α slider** underneath (range [0, 1],
  default 0.85, current value shown large, e.g., "α = 0.85"; arrow keys nudge
  it). Dragging the vertical α line on the plot also sets α.
- A compact **toolbar or menu** for commands (new, open, save, random graph,
  layouts, undo/redo, zoom). Every command is also reachable from a right-click
  menu on the canvas, so nothing depends on memorizing shortcuts.

The visual design was chosen from mockups during brainstorming ("version B").

### Pairing nodes and curves

- Each node gets **its own color** from a palette of about 10–12 colors chosen
  to be distinguishable when projected. Its curve uses the same color.
- A node's color is assigned at creation and **never changes** while the node
  exists; deleting a node does not recolor others. New nodes take the least
  used palette color. Past the palette size, colors repeat; labels and hover
  highlighting disambiguate.
- **Node size** encodes the node's score at the current α: area proportional
  to the score, normalized so that a node with the uniform score 1/*n* has a
  standard size, clamped to a minimum (label always readable) and a maximum.
  Moving the slider makes nodes grow and shrink.
- The node **label** is drawn inside the node, in a contrasting color.
- The plot shows a **vertical line at the current α** with a dot where it
  crosses each curve.
- **Curve labels** at the right end (α = 1): node labels in the curve's color.
  Curves of nodes in the same terminal cycle often converge to the same value,
  so labels at the same height are grouped ("d e"), not overlapped.
- **Hover highlighting:** hovering a node thickens its curve and fades the
  others; hovering a curve highlights it and its node. (Clicking a node starts
  an edge, so click is not used for highlighting on the canvas; the builder may
  let a click on a curve pin its highlight.)

### Showing the change

After each edit, the curves from **before** the edit remain visible as faded,
dashed lines in the same colors until the next edit, so students see before
and after together. Details:

- "Edit" means anything that changes scores: adding/removing a node or edge,
  undo, redo. Moving nodes, layouts, renaming, and α changes do not touch the
  ghost curves.
- Ghost curves are drawn only for nodes that exist in both states.
- When the node set is replaced wholesale (new graph, open, random graph),
  there are no ghost curves.
- Undo after an edit therefore shows the edited state as the ghost — flipping
  undo/redo is the before/after demonstration.

The y-axis scales to the maximum of the current and ghost curves (rounded up
to a tick), so before and after share one scale; a toggle fixes it to [0, 1].
Curves do not morph or animate on edits — the ghost curves already show the
change, and the suddenness is the point.

### Terminal components

The canvas shades each **terminal strongly connected component of the graph
as drawn**: a component with no arc leaving it, so that a dangling node is a
terminal component by itself. It is recomputed after every edit and drawn as
a soft rounded hull behind the component's nodes, labeled "terminal". This
puts the cause (a component closing) next to the effect (curves splitting
near α = 1). Non-terminal components are not shaded.

(User's decision after trying the first version, which shaded the terminal
components of the patched chain: on a → b → c it shaded everything, whereas
only c is terminal.)

**Dangling nodes** are marked visibly (e.g., a dashed outline), since they
behave as if linked to every node.

### Projector readability

- Light, high-contrast theme.
- Large default fonts, thick strokes, large arrowheads.
- Global zoom with Cmd/Ctrl + and −, plus a full-screen toggle.

## Editing

All gestures use the platform command key where a modifier is needed (Cmd on
macOS, Ctrl elsewhere).

- **Add a node:** double-click on empty canvas. Nodes are labeled a, b, c, …
  (then continuing with two letters).
- **Rename a node:** typing a letter or digit while hovering a node starts
  renaming it, with an inline text field; Enter commits, Esc cancels. While
  renaming, Backspace edits the label; otherwise Backspace/Delete deletes the
  hovered node (see below). (Designer's call.)
- **Add or remove an edge:** click the source node — it gets a highlighted ring
  and an arrow follows the mouse — then click the target. If the arc is absent
  it is added; if present it is removed. The same two clicks toggle an edge, so
  the instructor can flip it on and off while talking. Clicking an edge
  directly also removes it. Esc, or a click on empty canvas, cancels a pending
  edge.
- **Self-loops:** added and removed from the right-click menu on a node.
- **Move a node:** drag it. (A drag never creates an edge.)
- **Delete a node:** hover it and press Delete/Backspace, or right-click menu.
- **Undo/redo:** Cmd-Z / Cmd-Shift-Z, unlimited, covering every change to the
  graph and node positions: edits, renames, moves (one step per drag), random
  graphs, layouts, new graph. α changes are not undoable. Snapshotting the
  whole document per step is fine at this size.
- **No confirmation dialogs** for commands that undo can reverse (new graph,
  random graph, layouts). Confirmation is asked only where undo cannot help:
  quitting, or opening a file over unsaved changes. (Designer's call.)
- No parallel edges.

**Drawing:** arrows with arrowheads clearly visible at projector scale; two
opposite arcs between the same pair are drawn side by side (offset or curved)
so both are visible and clickable; self-loops are drawn as small loops. Edge
hit-testing uses a generous tolerance.

### Undirected graphs

As in the old version, "New graph" offers directed (default) or undirected.
In an undirected graph, an edge toggle adds or removes both arcs, and edges
are drawn without arrowheads.

## Random graphs

A command generates a random directed graph replacing the current one
(undoable). Degrees must follow a **power law**. Model: directed **Chung–Lu**.

- Each node *i* gets an out-weight and an in-weight drawn from a power law
  with exponent γ; out- and in-weights are assigned independently, so in- and
  out-degree are not correlated.
- Weights are scaled so that the expected average degree equals the target
  average degree *d*.
- Arc u → v (u ≠ v) is present independently with probability
  min(1, w_out(u) · w_in(v) / W), where W is the total weight.
- Controls: number of nodes, average degree, exponent γ. Sensible defaults for
  a classroom (e.g., 10 nodes, average degree 2, γ around 2.5; the builder may
  tune these for good-looking results).
- A seeded RNG (`rand::rngs::SmallRng`, the user's choice); the seed is
  shown, and can be entered to regenerate a graph for a later lecture.
  Reproducibility is required only within the same build: stability of seeds
  across `rand` versions is explicitly not an issue.
- The generated graph is laid out with the force-directed layout immediately.

**Plain Chung–Lu, no resampling or patching** (the user's decision). Known
consequence, accepted: sparse random graphs usually have dangling nodes and
no closed component containing a cycle, and then the patched chain is a
single terminal component (no curve drops to zero at α = 1, and the shading is
one hull around all nodes). A simulation of the model above gave this outcome
in about 78% of graphs at 10 nodes and average degree 2, 60% at average degree
3, and 87% at 20 nodes. The instructor adds the missing structure by hand;
this is not a bug to work around.

Preferential attachment is deliberately **not** used: it produces acyclic
graphs with essentially no nontrivial terminal components.

## Layouts

Layouts run only on request (never continuously) and **animate** nodes to
their new positions over about a second, so students can follow each node.
All are undoable.

- **Force-directed** (e.g., Fruchterman–Reingold), the "tidy up" command.
- **Circle.**
- **Components:** the layout agrees with the terminal-component shading. The
  terminal components of the graph (the shaded ones, including dangling
  nodes) go in the rightmost column. All other components are arranged left
  to right by the condensation DAG of the graph (by longest path from
  sources). Nodes within a component are arranged compactly, on a small
  circle.

  Useful fact for the implementation: if the drawn graph has at least one
  closed component containing a cycle (or a self-loop), every dangling node is
  transient in the patched chain, and the terminal components of the patched
  chain are exactly those closed components. Otherwise the patched chain has a
  single terminal component.

## Files

Save/open graphs as JSON (via `serde`): directedness, nodes (stable id, label,
position, color index), arcs, current α, and the random seed if any. Native
file dialogs (e.g., the `rfd` crate). The window title shows the file name and
an unsaved-changes marker.

## Architecture (guidance)

Keep the mathematics and the graph model independent of the GUI:

- **Document model:** nodes with stable ids (colors and ghost curves are keyed
  by id, never by index), arcs, positions, labels, directedness.
- **Ranking:** from a document, compute the curves, the limit at α = 1, the
  terminal components of the patched chain, and the score vector at a given α.
  Pure functions, unit-tested.
- **Generation and layout:** Chung–Lu generator; layout algorithms producing
  target positions; GUI-free.
- **History:** undo/redo stack of document snapshots.
- **UI:** egui app with the canvas, the plot, the controls, and the animation
  of layout transitions. Recompute ranking once after each edit, keeping the
  previous result for the ghost curves.

## Left to the builder

- Exact palette, sizes, fonts, stroke widths, and spacing (subject to the
  projector-readability goals).
- Dense linear algebra: hand-rolled LU or a crate such as `nalgebra`.
- Sampling grid for the curves, as long as curves are smooth and exact at α = 1.
- Hull shape for terminal-component shading.
- Force-directed layout parameters; random-graph default parameters.
- Whether the α marker is also draggable on the plot (preferred) and whether a
  curve can be pinned by clicking.
- Showing the numeric score of the hovered node (a tooltip is welcome).
