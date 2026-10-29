# Centrality Explorer — Design

Date: 2026-10-29

## Intent

PageRank Explorer (version 0.1.0, this repository) is a teaching aid: the
instructor edits a small graph at a projector, and the class watches each
node's PageRank, plotted as a function of α, react to every edit. This work
generalizes it to seven centralities and renames it **Centrality Explorer**.
The principle stays the same: one curve per node, in the node's color; node
size showing the score; the curves from before the last edit kept as faded
dashed lines; everything updated instantly on every edit.

The seven centralities come in three families, and the families are the
point of the lecture:

| Family | Centrality | x-axis of the plot |
|---|---|---|
| Damped spectral | PageRank | α ∈ [0, 1] (as today) |
| | Katz | βρ ∈ [0, 1] |
| Power method | Seeley's index | iteration *k* = 0, …, *K* |
| | Eigenvector centrality | iteration *k* = 0, …, *K* |
| Geometric/path | Closeness, harmonic, betweenness | none: horizontal lines |

The two damped indices tend, at the right end of their plots, to the limits
of the two power methods when these converge: PageRank as α → 1 to Seeley's
index, Katz as βρ → 1 to eigenvector centrality. Plotting the power-method
iterates (the user's choice, from a proposal during brainstorming) shows
what happens when the method does *not* converge: on periodic structures the
curves zigzag forever, which the damped limits hide.

## Goals

- All seven centralities, selectable at any time, with the same editing,
  colors, node sizes, hover pairing, before/after curves, and undo/redo as
  PageRank today.
- Exact curves (to floating-point precision) everywhere, including the right
  end of the Katz plot, which is the exact limit as βρ → 1, not a value
  computed at some β close to 1/ρ.
- Every edit updates curves, node sizes, and shading immediately, as today,
  up to about 100 nodes.
- The name of the current centrality is readable from the back of the room.

## Non-goals

- Centralities other than these seven (e.g., in-degree, HITS, Lin's index).
- Weighted graphs; non-uniform preference vectors; Katz with a vector other
  than **1**.
- Animating the power method in time (the slider over *k* covers this).
- Renaming the repository directory (the user's business).

## Anti-goals (failure even if it technically works)

- Power-method curves that are averaged, smoothed, or otherwise made to
  converge: the zigzag on periodic graphs is the lesson.
- A Katz curve that is wrong or jagged near βρ = 1, or whose right end is not
  the exact limit.
- Losing anything that works today (gestures, colors, undo, before/after,
  projector readability) when switching centrality.
- Switching centrality changing node colors, positions, or the graph.

## The mathematics

Notation: the graph has *n* nodes and adjacency matrix *A* (*a*ᵢⱼ = 1 iff
there is an arc *i* → *j*; a self-loop sets *a*ᵢᵢ = 1; an undirected edge is
a pair of opposite arcs, as today). Scores are row vectors, and every
centrality is **oriented by incoming paths** (a node is central if it is
reached), so spectral scores multiply on the right by *A* or *P*. *P* is the
row-stochastic matrix used by PageRank today, with dangling rows replaced by
uniform rows. ρ is the spectral radius of *A*.

**Normalization (decided):** every score vector shown, in the plot and in
node sizes, is normalized to sum to 1. A vector that is identically zero is
shown as zero (all nodes at minimum size, all curves at 0). Node sizes keep
today's rule: a node with score 1/*n* has the standard size.

### PageRank

Unchanged.

### Katz

*k*(β) = **1** Σ_{t ≥ 0} β^t *A*^t = **1** (*I* − β*A*)⁻¹, for 0 ≤ β < 1/ρ,
shown normalized: κ(β) = *k*(β) / ‖*k*(β)‖₁. The term *t* = 0 is included, so
κ(0) is uniform, like PageRank at α = 0.

- **ρ > 0:** the x-axis is *t* = βρ ∈ [0, 1], so the axis does not change
  when an edit changes ρ, and before/after curves line up. Sample like
  PageRank (solve at each *t* < 1) and add the exact limit at *t* = 1.
- **ρ = 0** (no cycles and no self-loops; *A* is nilpotent): there is no
  natural range, and the **user decided β ∈ [0, 1]**. The x-axis is then β
  itself; *I* − *A* is invertible, so β = 1 is an ordinary solve and no limit
  is needed.

In both cases, the x-axis is the fraction of the range of β, with ρ taken as
1 when ρ = 0. A ghost from before an edit that changed ρ between 0 and a
positive value is therefore drawn as usual (my call). With 0/1 matrices,
ρ > 0 implies ρ ≥ 1, and closing a single simple cycle gives ρ = 1, so in
the common case the two axes coincide exactly.

**The limit at βρ = 1.** It always exists. Let the *basic* classes be the
strongly connected components *C* whose spectral radius ρ_C equals ρ, and
let ν (the index of ρ) be the maximum number of basic classes on a path of
the condensation. Then (1 − βρ)^ν **1**(*I* − β*A*)⁻¹ tends to a nonzero
nonnegative vector, and κ tends to its normalization. The limit is supported
on the basic classes that end a chain of ν basic classes, and on the nodes
they reach. When ν > 1, a basic class upstream of another one gets zero in
the limit.

One exact method (the builder may choose another): process classes in
topological order of the condensation, writing *s* = 1 − βρ and expanding
*k*_C, the restriction of *k* to class *C*, as *c*_C *s*^(−*h*(C)) + …, where
*h*(C) is the maximum number of basic classes on a path ending in *C*. From
*k*_C (*I* − β*A*_CC) = **1**_C + β Σ_{D→C} *k*_D *A*_DC, and writing
*u*_H(C) = Σ_{D→C, h(D) = H} *c*_D *A*_DC / ρ, plus **1**_C when *H* = 0:

- non-basic *C*: *c*_C = *u*_{h(C)}(C) (*I* − *A*_CC/ρ)⁻¹;
- basic *C*: *c*_C = (*u*_{h(C)−1}(C) *r*_C / (*l*_C *r*_C)) *l*_C, with *l*_C
  and *r*_C the left and right Perron vectors of *A*_CC;
- the limit is (*c*_C on the classes with *h*(C) = ν, 0 elsewhere),
  normalized.

This needs ρ to near machine precision, and a tolerance to decide
ρ_C = ρ. With 0/1 matrices, exact ties (e.g., two cycles) are the common
case, and near-ties are not.

### Seeley's index (power method on *P*)

Iterates *x*₀ = (1/*n*, …, 1/*n*), *x*_{k+1} = *x*_k *P*, for *k* = 0, …, *K*.
*P* is stochastic, so every iterate sums to 1. When the iterates converge,
their limit is *v P*\*, the PageRank limit at α = 1. On periodic chains they
oscillate (e.g., a ⇄ b, c → a alternates between (⅓, ⅔, 0) and
(⅔, ⅓, 0)).

### Eigenvector centrality (power method on *A*)

Iterates *x*₀ = (1/*n*, …, 1/*n*) (the normalized vector of ones),
*x*_{k+1} = *x*_k *A* / ‖*x*_k *A*‖₁, for *k* = 0, …, *K*. If *x*_k *A* = 0
(exactly: everything is nonnegative, so there is no cancellation), the
iterates **vanish**. This happens on every graph with ρ = 0, at some
*k* ≤ *n*. When they converge, the limit equals the Katz limit at βρ = 1.
When ν > 1, convergence is only like 1/*k*, and the curves are visibly still
drifting at *K*, which is accepted and honest. On bipartite graphs they
oscillate: on an undirected star they alternate between the uniform vector
and (½ on the center, the rest split among the leaves).

**K (my call):** about 50, the same for both power methods. The builder may
tune it.

### Closeness, harmonic, betweenness

Following Boldi and Vigna, *Axioms for Centrality*, with distances *d*(*y*, *x*)
from *y* to *x* (incoming):

- **Closeness:** 1 / Σ_{y : 0 < d(y,x) < ∞} *d*(*y*, *x*), and 0 if no other
  node reaches *x* (my call).
- **Harmonic:** Σ_{y ≠ x} 1 / *d*(*y*, *x*), with 1/∞ = 0.
- **Betweenness:** Σ σ_yz(*x*) / σ_yz over ordered pairs of distinct *y*, *z*
  different from *x* with σ_yz > 0 (Brandes's algorithm). In undirected
  graphs ordered pairs count each path twice, which normalization removes.

Self-loops do not affect any of the three.

## The screen

The layout is unchanged: canvas on the left, plot and slider on the right.

**Selecting a centrality (my call):** a **Centrality** menu with the seven
entries grouped by family, the current one checked (PageRank, Katz | Seeley,
Eigenvector | Closeness, Harmonic, Betweenness), each with a shortcut. The
current centrality's name appears large on screen (e.g., as a heading over
the plot or as the y-axis label), and the window title starts with
"Centrality Explorer".

**The plot, per family:**

- **PageRank:** as today.
- **Katz:** x-axis labeled βρ (or β when ρ = 0), ticks as for α; slider and
  vertical marker over the same range, with default 0.85. The value of ρ is
  shown next to the slider (my call).
- **Power methods:** x-axis is *k*, with integer ticks. The curves join the
  iterates with straight segments, so a period-2 zigzag is obvious. The
  slider and marker take integer values 0…*K* (default *K*; arrow keys step
  by 1, by 5 with Shift). Node sizes show the iterate at the slider's *k*.
  If the iterates vanish, the curves stop at the last nonzero iterate, a
  short note on the plot says so (e.g., "**1**·*A*ᵏ = 0 for *k* ≥ 4"), and at
  those *k* all nodes have minimum size.
- **Closeness, harmonic, betweenness:** horizontal lines at each node's
  score, across the plot. There is no slider, no marker, and no x ticks.
  Labels at the right end, hover, and before/after lines work as for the
  other curves.

Each parametric centrality keeps its own slider value for the session (α
for PageRank, βρ for Katz, one *k* shared by the two power methods).

**Before/after (user's decision):** the faded dashed curves always show the
current centrality on the graph from before the last edit, so they survive a
switch. The instructor can make an edit under PageRank, then switch to Katz
and see the same edit's effect there; the user also expects people to use
this to compare centralities. The session therefore keeps the graph from
before the last edit, not just its curves. The ghost follows today's rules
otherwise (no ghost after new, open, or random graph; undo/redo flips).

**Shading and markers, per centrality (agreed):**

- PageRank and Seeley's index: sink hulls and dashed outlines for dangling
  nodes, as today.
- Katz and eigenvector centrality: hulls around the basic classes
  (ρ_C = ρ) when ρ > 0, labeled e.g. "dominant". There are no dangling
  markers, since dangling nodes are not special for *A*.
- Closeness, harmonic, and betweenness: no hulls, no markers.

The **components layout** keeps its current behavior (sinks in the rightmost
column) for all centralities.

## Files

Saved graphs gain the current centrality and the Katz parameter, as
optional fields, so files saved by 0.1.0 still load (as PageRank, Katz at
0.85). As with α today, changing these is neither undoable nor an unsaved
change. The iteration *k* is not saved; it resets to *K*. (My call.)

## The rename

Crate and binary `centrality-explorer`, application name, window title,
file-dialog filter, README, Cargo description, doc comments, and tests that
mention the name. The README gains a section per centrality in "The
Mathematics" and the per-centrality reading of the screen. The CHANGELOG
gets a new version entry.

## Tests for the math

GUI-free and unit-tested, as today. At minimum:

- Katz: κ(0) is uniform; every κ sums to 1; the computed limit matches κ at
  βρ = 1 − ε on assorted graphs: strongly connected, periodic, two basic
  classes in a chain (ν = 2, e.g., a ⇄ b → c ⇄ d, where the limit is zero on
  a and b), two basic classes in parallel, and basic plus non-basic classes
  downstream. For ν > 1 the convergence is O(ε), so the tolerance must allow
  for it.
- Katz with ρ = 0: β ranges over [0, 1], and the value at β = 1 matches a
  hand computation on a small DAG.
- Seeley iterates: they sum to 1; on aperiodic graphs they approach the
  PageRank limit; the a ⇄ b, c → a example oscillates with period 2.
- Eigenvector iterates: on aperiodic graphs they approach the Katz limit;
  on a DAG they vanish at the predicted *k*; the undirected star oscillates.
- Closeness, harmonic, betweenness: hand-computed values on a directed path,
  a directed cycle, a star, and a graph that is not strongly connected.
- The speed test at 100 nodes, extended to Katz.

## Architecture (guidance)

Today's `Ranking` holds PageRank curves, sinks, and dangling nodes. It
generalizes to a ranking per centrality: the sampled x values, the curves,
the x-axis kind (α, βρ, β, *k*, none), the hulls to shade with their label,
the nodes to mark as dangling, the scores at a slider value, and, for
eigenvector centrality, the vanishing iteration. The session recomputes the
current centrality on each edit, and recomputes the current one and its
ghost on a switch. The UI reads all of this, with no
centrality-specific logic beyond labels.

## Left to the builder

- Shortcuts for the centralities. They must not collide with existing
  shortcuts, or with letters and digits typed over a node to rename it.
- *K*, the Katz sampling grid, the algorithm for ρ and the Perron vectors,
  and the method for the exact Katz limit.
- Wording and placement of the centrality name, the hull label, the vanishing
  note, and the ρ display.
- Module names (e.g., whether `rank` becomes `centrality`).
- The new version number.
