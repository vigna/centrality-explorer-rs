# PageRank Explorer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A native Rust/egui teaching aid where an instructor edits a small
directed graph at a projector and sees each node's exact PageRank-versus-α
curve update instantly.

**Architecture:** A GUI-free library core (graph document model, exact
PageRank curves and α = 1 limit, Chung–Lu generator, layouts, session with
undo/redo and before/after curves, JSON files, editing gesture state machine)
is unit-tested in isolation. A thin egui layer (canvas, plot, dialogs) reads
the session every frame and sends edits back as new graph snapshots: the
session is the single source of truth, recomputing the ranking once per edit.

**Tech Stack:** Rust 1.99 (edition 2024), eframe/egui 0.37, egui_plot 0.38,
rfd 0.17, serde 1 + serde_json 1, rand 0.10 (`SmallRng`), anyhow 1;
dev: egui_kittest 0.37 (feature `eframe`), tempfile 3.

**Spec:** `docs/superpowers/specs/2026-10-29-pagerank-explorer-design.md`
(read it first; this plan argues from it).

## Global Constraints

- Crate `pagerank-explorer`, library `pagerank_explorer` plus binary
  `pagerank-explorer`; edition 2024.
- `.cargo/config.toml` must contain exactly:
  `[build]` / `rustflags = ["-C", "target-cpu=native"]`.
- Dependency versions: `eframe = "0.37"`, `egui = "0.37"`,
  `egui_plot = "0.38"`, `rfd = "0.17"`, `serde = { version = "1", features =
  ["derive"] }`, `serde_json = "1"`, `rand = "0.10"`, `anyhow = "1"`;
  dev: `egui_kittest = { version = "0.37", features = ["eframe"] }`,
  `tempfile = "3"`.
- APIs that differ from older egui/rand (verified in the 0.37/0.10 sources):
  `eframe::App` requires `fn ui(&mut self, ui: &mut egui::Ui, frame: &mut
  eframe::Frame)`; panels are `egui::Panel::{top,right,left,bottom}(id)` and
  `egui::CentralPanel::default()`, both shown with `.show(ui, |ui| …)`;
  `egui_plot::Line::new(name, PlotPoints)`; in rand 0.10 the extension trait
  with `random`/`random_range` is `rand::RngExt`, and seeding is
  `rand::SeedableRng::seed_from_u64` on `rand::rngs::SmallRng`.
- Follow the user's Rust guidelines (rust-guidelines skill): code is
  `cargo clippy --all-features --all-targets`-clean and `rustfmt`-formatted;
  directory modules use `mod.rs`; module names short; test functions named
  `test_*`; `assert_eq!(actual, expected)` order; tests that handle errors
  return `anyhow::Result<()>` and use `?`; prefer `impl Trait` parameters,
  `&[T]` over `&Vec<T>`, `impl AsRef<str>`/`impl AsRef<Path>`; struct fields
  immutable-first; `README.md` is the crate doc via
  `#![doc = include_str!("../README.md")]`; a `CHANGELOG.md` in
  keep-a-changelog format.
- Dangling nodes jump uniformly; the preference vector is uniform.
- α slider: range [0, 1], default 0.85.
- `PALETTE_SIZE = 12`.
- Random graphs: plain directed Chung–Lu, no resampling; defaults 10 nodes,
  average degree 2, exponent 2.5.
- Layout animations last about one second (use 1.0 s).
- `.gitignore`: `/target`, `.superpowers/`, `.playwright-mcp/`.
- Work on branch `pagerank-explorer` (the repository has no commits yet;
  create the branch before the first commit).

## Review Focus

- **Empty and one-node graphs** reach every component (ranking, plot scale,
  layouts, files): expected no panic, empty plot for n = 0, a flat curve at 1
  for n = 1. Tests: Task 3 `test_ranking_empty_graph`,
  `test_ranking_single_node`; Task 5 `test_layouts_empty_and_single`; Task 11
  `test_nice_ceiling`.
- **α exactly 1 on the slider**: scores must come from the exact limit, never
  from a singular solve. Tests: Task 3 `test_pagerank_at_one_is_limit`; Task 6
  `test_scores_at_alpha_one`.
- **Stale node ids** after undo/redo/delete (pending-edge source, hovered
  node, node being renamed, ghost lookup): silently dropped, no panic. Tests:
  Task 8 `test_prune_drops_stale_ids`; Task 6 `test_ghost_lookup_removed_node`.
- **Self-loops and undirected graphs** through ranking, limit, and toggling:
  self-loop-only node is a terminal singleton; undirected toggle affects both
  arcs. Tests: Task 1 `test_toggle_arc_undirected`; Task 3
  `test_terminal_self_loop`, `test_limit_matches_near_one` (includes an
  undirected path and a self-loop).
- **Hand-edited or malformed JSON files**: an error message, never a panic or
  a corrupt graph. Tests: Task 7 `test_load_rejects_*`.

---

### Task 1: Scaffold and graph document model

**Files:**
- Create: `Cargo.toml`, `.cargo/config.toml`, `.gitignore`, `README.md`
  (one paragraph for now), `CHANGELOG.md` (an `[Unreleased]` section),
  `src/lib.rs`, `src/main.rs` (temporary: `fn main() {}`),
  `src/graph/mod.rs`

**Interfaces:**
- Produces (`pagerank_explorer::graph`):
  - `pub const PALETTE_SIZE: usize = 12;`
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord,
    Serialize, Deserialize)] pub struct NodeId(pub u32);`
  - `#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)] pub struct
    Node { pub id: NodeId, pub label: String, pub pos: [f32; 2], pub color:
    usize }` — `pos` is in canvas-local logical points.
  - `#[derive(Clone, Debug, PartialEq)] pub struct Graph` with private fields
    `directed: bool, generation: u64, nodes: Vec<Node>, arcs:
    BTreeSet<(NodeId, NodeId)>, next_id: u32, seed: Option<u64>`.
  - `pub fn label_for(k: usize) -> String` (bijective base 26: a…z, aa…).
  - `impl Graph`: `new(directed: bool) -> Self` (fresh generation from a
    global `AtomicU64`); `is_directed`; `generation() -> u64`;
    `seed() -> Option<u64>`; `set_seed(&mut self, seed: impl
    Into<Option<u64>>)`; `len`; `is_empty`; `nodes() -> &[Node]`;
    `node(id) -> Option<&Node>`; `index_of(id) -> Option<usize>`;
    `add_node(&mut self, pos: [f32; 2]) -> NodeId`;
    `remove_node(&mut self, id) -> bool`;
    `set_label(&mut self, id, label: impl AsRef<str>) -> bool`;
    `set_pos(&mut self, id, pos: [f32; 2]) -> bool`;
    `set_positions(&mut self, pos: &[[f32; 2]])` (aligned with `nodes()`);
    `has_arc(from, to) -> bool`; `add_arc(&mut self, from, to) -> bool`;
    `remove_arc(&mut self, from, to) -> bool`;
    `toggle_arc(&mut self, from, to) -> bool` (presence after the toggle);
    `arcs(&self) -> impl Iterator<Item = (NodeId, NodeId)> + '_`;
    `successors(&self) -> Vec<Vec<usize>>` (by node index, each list sorted);
    `same_structure(&self, other: &Graph) -> bool`;
    `pub(crate) fn from_parts(directed: bool, nodes: Vec<Node>, arcs: impl
    IntoIterator<Item = (NodeId, NodeId)>, seed: Option<u64>) ->
    anyhow::Result<Graph>`.

Rules the tests pin: ids are never reused within a graph (`next_id` only
grows; `from_parts` sets it to max id + 1); a new node's label is the first
`label_for(k)` not used by any node; its color is the least-used palette
index, ties broken by the lowest index; in an undirected graph `add_arc`,
`remove_arc`, `toggle_arc` act on both directions (a self-loop is a single
arc); `add_arc` returns false for unknown ids or an existing arc;
`same_structure` compares directedness, the set of node ids, and the arc set
(ignores labels, positions, colors, seed, generation); `Clone` keeps the
generation; `from_parts` rejects duplicate node ids, arcs with unknown ids,
and colors ≥ `PALETTE_SIZE`, deduplicates arcs, and for undirected graphs
adds missing reverse arcs.

- [ ] **Step 1: Create the branch and scaffold**

```bash
git switch -c pagerank-explorer
```

Write `Cargo.toml` (dependencies from Global Constraints), `.cargo/config.toml`,
`.gitignore`, `README.md`, `CHANGELOG.md`, `src/lib.rs` (crate doc from
README, `pub mod graph;`), `src/main.rs`.

- [ ] **Step 2: Write the failing tests** (in `src/graph/mod.rs`, `mod tests`)

```rust
#[test]
fn test_label_for() {
    assert_eq!(label_for(0), "a");
    assert_eq!(label_for(25), "z");
    assert_eq!(label_for(26), "aa");
    assert_eq!(label_for(27), "ab");
    assert_eq!(label_for(52), "ba");
    assert_eq!(label_for(701), "zz");
    assert_eq!(label_for(702), "aaa");
}

#[test]
fn test_add_node_labels_and_colors() {
    let mut g = Graph::new(true);
    let a = g.add_node([0.0, 0.0]);
    let b = g.add_node([1.0, 0.0]);
    let _c = g.add_node([2.0, 0.0]);
    assert!(g.remove_node(b));
    let d = g.add_node([3.0, 0.0]);
    let labels: Vec<_> = g.nodes().iter().map(|n| n.label.as_str()).collect();
    assert_eq!(labels, ["a", "c", "b"]);
    assert_eq!(g.node(d).unwrap().color, 1);
    assert_eq!(g.node(a).unwrap().color, 0);
    assert_eq!(d, NodeId(3)); // ids are never reused
}

#[test]
fn test_colors_wrap() {
    let mut g = Graph::new(true);
    let ids: Vec<_> = (0..=PALETTE_SIZE).map(|i| g.add_node([i as f32, 0.0])).collect();
    assert_eq!(g.node(ids[PALETTE_SIZE]).unwrap().color, 0);
}

#[test]
fn test_toggle_arc_directed() {
    let mut g = Graph::new(true);
    let (a, b) = (g.add_node([0.0, 0.0]), g.add_node([1.0, 0.0]));
    assert!(g.toggle_arc(a, b));
    assert!(g.has_arc(a, b));
    assert!(!g.has_arc(b, a));
    assert!(!g.toggle_arc(a, b));
    assert!(!g.has_arc(a, b));
}

#[test]
fn test_toggle_arc_undirected() {
    let mut g = Graph::new(false);
    let (a, b) = (g.add_node([0.0, 0.0]), g.add_node([1.0, 0.0]));
    assert!(g.toggle_arc(a, b));
    assert!(g.has_arc(a, b) && g.has_arc(b, a));
    assert!(!g.toggle_arc(b, a));
    assert_eq!(g.arcs().count(), 0);
    assert!(g.toggle_arc(a, a));
    assert_eq!(g.arcs().count(), 1);
}

#[test]
fn test_remove_node_removes_arcs() {
    let mut g = Graph::new(true);
    let (a, b, c) = (g.add_node([0.0; 2]), g.add_node([1.0, 0.0]), g.add_node([2.0, 0.0]));
    g.add_arc(a, b);
    g.add_arc(b, c);
    g.add_arc(c, a);
    g.remove_node(b);
    assert_eq!(g.arcs().collect::<Vec<_>>(), [(c, a)]);
    assert!(!g.add_arc(a, b));
}

#[test]
fn test_successors() {
    let mut g = Graph::new(true);
    let ids: Vec<_> = (0..3).map(|i| g.add_node([i as f32, 0.0])).collect();
    g.add_arc(ids[0], ids[2]);
    g.add_arc(ids[0], ids[1]);
    g.add_arc(ids[2], ids[0]);
    assert_eq!(g.successors(), vec![vec![1, 2], vec![], vec![0]]);
}

#[test]
fn test_same_structure() {
    let mut g = Graph::new(true);
    let (a, b) = (g.add_node([0.0; 2]), g.add_node([1.0, 0.0]));
    let mut h = g.clone();
    h.set_pos(a, [5.0, 5.0]);
    h.set_label(b, "x");
    assert!(g.same_structure(&h));
    h.add_arc(a, b);
    assert!(!g.same_structure(&h));
    let mut k = g.clone();
    k.add_node([2.0, 0.0]);
    assert!(!g.same_structure(&k));
}

#[test]
fn test_generation() {
    let g = Graph::new(true);
    assert_ne!(Graph::new(true).generation(), g.generation());
    assert_eq!(g.clone().generation(), g.generation());
}

#[test]
fn test_from_parts_validation() {
    let node = |id, color| Node { id: NodeId(id), label: "n".into(), pos: [0.0; 2], color };
    assert!(Graph::from_parts(true, vec![node(0, 0), node(0, 1)], [], None).is_err());
    assert!(Graph::from_parts(true, vec![node(0, 0)], [(NodeId(0), NodeId(7))], None).is_err());
    assert!(Graph::from_parts(true, vec![node(0, PALETTE_SIZE)], [], None).is_err());
    let mut g = Graph::from_parts(false, vec![node(0, 0), node(5, 1)], [(NodeId(0), NodeId(5))], None).unwrap();
    assert!(g.has_arc(NodeId(5), NodeId(0)));
    assert_eq!(g.add_node([0.0; 2]), NodeId(6));
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --lib graph`
Expected: compilation errors (items not defined).

- [ ] **Step 4: Implement `src/graph/mod.rs`** per the Interfaces block. Add
  doc comments to every public item.

- [ ] **Step 5: Run tests and clippy**

Run: `cargo test --lib graph && cargo clippy --all-features --all-targets -- -D warnings`
Expected: all tests pass, no warnings.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock .cargo .gitignore README.md CHANGELOG.md src docs
git commit -m "Add crate scaffold and graph document model"
```

---

### Task 2: Exact PageRank at α < 1

**Files:**
- Create: `src/rank/mod.rs`, `src/rank/lu.rs`
- Modify: `src/lib.rs` (`pub mod rank;`)

**Interfaces:**
- Consumes: `Graph::successors()` (Task 1).
- Produces (`pagerank_explorer::rank`):
  - `#[derive(Clone, Debug, PartialEq)] pub struct Chain { succ:
    Vec<Vec<usize>> }` — an empty successor list means dangling (uniform
    jump).
  - `impl Chain`: `pub fn new(succ: Vec<Vec<usize>>) -> Self`;
    `pub fn from_graph(graph: &Graph) -> Self`; `len`; `is_empty`;
    `pub fn is_dangling(&self, i: usize) -> bool`;
    `pub fn successors(&self) -> &[Vec<usize>]`;
    `pub(crate) fn transition(&self) -> Vec<f64>` (dense row-major n×n P,
    dangling rows uniform).
  - `pub fn pagerank(chain: &Chain, alpha: f64) -> Vec<f64>` — solves
    (I − α Pᵀ) x = (1 − α) v with v uniform. In this task it requires
    `0.0 <= alpha < 1.0` (`debug_assert!`); Task 3 extends it to α = 1.
  - `pub(crate) fn lu::solve(a: &mut [f64], b: &mut [f64])` — `a` row-major
    n×n with n = `b.len()`; Gaussian elimination with partial pivoting;
    overwrites `b` with the solution.

- [ ] **Step 1: Write the failing tests** (in `src/rank/mod.rs`; the
  five-node example is a=0, b=1, c=2, d=3, e=4; reference values computed
  independently with NumPy)

```rust
pub(crate) fn example_after() -> Chain {
    Chain::new(vec![vec![1], vec![2], vec![0, 3], vec![4], vec![3]])
}
pub(crate) fn example_before() -> Chain {
    Chain::new(vec![vec![1], vec![2], vec![0, 3], vec![4], vec![]])
}
fn assert_close(actual: &[f64], expected: &[f64], tol: f64) {
    assert_eq!(actual.len(), expected.len());
    for (a, e) in actual.iter().zip(expected) {
        assert!((a - e).abs() <= tol, "{actual:?} != {expected:?}");
    }
}

#[test]
fn test_pagerank_known_values() {
    assert_close(&pagerank(&example_after(), 0.85),
        &[0.07733381437719854, 0.09573374222061874, 0.11137368088752596,
          0.37057230406197667, 0.34498645845268017], 1e-10);
    assert_close(&pagerank(&example_before(), 0.85),
        &[0.16903241069316305, 0.20925005914360068, 0.24343506032647272,
          0.16903241069316302, 0.20925005914360073], 1e-10);
}

#[test]
fn test_pagerank_sums_to_one() {
    for chain in [example_after(), example_before(), Chain::new(vec![vec![1, 2, 3], vec![], vec![], vec![0]])] {
        for alpha in [0.0, 0.3, 0.85, 0.99, 0.999] {
            let sum: f64 = pagerank(&chain, alpha).iter().sum();
            assert!((sum - 1.0).abs() < 1e-12, "alpha {alpha}: {sum}");
        }
    }
}

#[test]
fn test_pagerank_alpha_zero_uniform() {
    assert_close(&pagerank(&example_after(), 0.0), &[0.2; 5], 1e-15);
}

#[test]
fn test_pagerank_cycle_uniform() {
    let cycle = Chain::new(vec![vec![1], vec![2], vec![3], vec![0]]);
    for alpha in [0.5, 0.99] {
        assert_close(&pagerank(&cycle, alpha), &[0.25; 4], 1e-12);
    }
}

#[test]
fn test_pagerank_tiny() {
    assert_eq!(pagerank(&Chain::new(vec![]), 0.5), Vec::<f64>::new());
    assert_close(&pagerank(&Chain::new(vec![vec![]]), 0.5), &[1.0], 1e-15);
    assert_close(&pagerank(&Chain::new(vec![vec![0]]), 0.5), &[1.0], 1e-15);
}
```

and in `src/rank/lu.rs`:

```rust
#[test]
fn test_solve() {
    // 2x + y - z = 8; -3x - y + 2z = -11; -2x + y + 2z = -3 -> (2, 3, -1)
    let mut a = vec![2.0, 1.0, -1.0, -3.0, -1.0, 2.0, -2.0, 1.0, 2.0];
    let mut b = vec![8.0, -11.0, -3.0];
    solve(&mut a, &mut b);
    for (x, e) in b.iter().zip([2.0, 3.0, -1.0]) {
        assert!((x - e).abs() < 1e-12);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib rank`
Expected: compilation errors.

- [ ] **Step 3: Implement `lu::solve`, `Chain`, and `pagerank`.**

- [ ] **Step 4: Run tests and clippy**

Run: `cargo test --lib rank && cargo clippy --all-features --all-targets -- -D warnings`
Expected: PASS, no warnings.

- [ ] **Step 5: Commit**

```bash
git add src
git commit -m "Add exact PageRank computation for alpha < 1"
```

---

### Task 3: Terminal components, the α = 1 limit, and `Ranking`

**Files:**
- Create: `src/rank/scc.rs`
- Modify: `src/rank/mod.rs`

**Interfaces:**
- Consumes: `Chain`, `pagerank`, `lu::solve` (Task 2); `Graph`, `NodeId`
  (Task 1).
- Produces (`pagerank_explorer::rank`, `scc` items re-exported):
  - `pub fn sccs(succ: &[Vec<usize>]) -> Vec<Vec<usize>>` — Tarjan
    (iterative), each component sorted ascending, components in reverse
    topological order (components with no outgoing arcs first).
  - `pub fn terminal_components(chain: &Chain) -> Vec<Vec<usize>>` — terminal
    SCCs of the **patched** chain (dangling rows link to every node), each
    sorted, list sorted by smallest member.
  - `pub fn limit(chain: &Chain) -> Vec<f64>` — r* = Σ_C a_C π_C as in the
    spec ("The limit at α = 1"): π_C by solving the stationary equations on C
    with one equation replaced by Σπ = 1; a_C = vᵀh where h solves
    (I − P_TT) h_T = P_{T,C} 1 on the transient nodes T.
  - `pagerank(chain, 1.0)` now returns `limit(chain)`.
  - `pub const SAMPLES: usize = 256;`
  - `#[derive(Clone, Debug)] pub struct Ranking { pub ids: Vec<NodeId>, pub
    alphas: Vec<f64>, pub curves: Vec<Vec<f64>>, pub terminal:
    Vec<Vec<NodeId>>, pub dangling: Vec<NodeId>, chain: Chain }` —
    `alphas[k] = k / SAMPLES` for `k < SAMPLES`, plus a final `1.0`
    (length `SAMPLES + 1`); `curves[i][k]` is the score of `ids[i]` at
    `alphas[k]`, the last entry from `limit`.
  - `impl Ranking`: `pub fn new(graph: &Graph) -> Self`;
    `pub fn scores(&self, alpha: f64) -> Vec<f64>` (exact at `alpha`, aligned
    with `ids`; `alpha >= 1.0` uses the limit);
    `pub fn curve(&self, id: NodeId) -> Option<&[f64]>`;
    `pub fn max_value(&self) -> f64` (0.0 when empty).

- [ ] **Step 1: Write the failing tests** (reuse `example_after`,
  `example_before`, `assert_close` from Task 2)

```rust
#[test]
fn test_sccs() {
    assert_eq!(sccs(example_after().successors()), vec![vec![3, 4], vec![0, 1, 2]]);
}

#[test]
fn test_terminal_components() {
    assert_eq!(terminal_components(&example_after()), vec![vec![3, 4]]);
    // e is dangling, hence linked to everybody: one big terminal component
    assert_eq!(terminal_components(&example_before()), vec![vec![0, 1, 2, 3, 4]]);
}

#[test]
fn test_terminal_self_loop() {
    assert_eq!(terminal_components(&Chain::new(vec![vec![0], vec![0]])), vec![vec![0]]);
}

#[test]
fn test_limit_example() {
    assert_close(&limit(&example_after()), &[0.0, 0.0, 0.0, 0.5, 0.5], 1e-12);
}

#[test]
fn test_limit_matches_near_one() {
    let graphs = [
        example_after(),
        example_before(),
        Chain::new(vec![vec![1], vec![0], vec![0, 3], vec![]]), // periodic + dangling
        Chain::new(vec![vec![1], vec![0, 2], vec![1, 3], vec![2]]), // undirected path
        Chain::new(vec![vec![1, 2], vec![1], vec![0]]), // self-loop terminal
        Chain::new(vec![vec![1], vec![2], vec![0], vec![4], vec![5], vec![3], vec![0, 3]]), // two closed cycles
    ];
    for chain in &graphs {
        let l = limit(chain);
        assert!((l.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert_close(&l, &pagerank(chain, 1.0 - 1e-7), 1e-5);
    }
}

#[test]
fn test_pagerank_at_one_is_limit() {
    assert_eq!(pagerank(&example_after(), 1.0), limit(&example_after()));
}

#[test]
fn test_ranking_new() {
    let mut g = Graph::new(true);
    let ids: Vec<_> = (0..5).map(|i| g.add_node([i as f32, 0.0])).collect();
    for (s, t) in [(0, 1), (1, 2), (2, 0), (2, 3), (3, 4), (4, 3)] {
        g.add_arc(ids[s], ids[t]);
    }
    let r = Ranking::new(&g);
    assert_eq!(r.ids, ids);
    assert_eq!(r.alphas.len(), SAMPLES + 1);
    assert_eq!(r.alphas[0], 0.0);
    assert_eq!(*r.alphas.last().unwrap(), 1.0);
    assert!((r.curve(ids[3]).unwrap().last().unwrap() - 0.5).abs() < 1e-12);
    assert_eq!(r.terminal, vec![vec![ids[3], ids[4]]]);
    assert!(r.dangling.is_empty());
    assert_close(&r.scores(0.85), &pagerank(&example_after(), 0.85), 1e-12);
}

#[test]
fn test_ranking_empty_graph() {
    let r = Ranking::new(&Graph::new(true));
    assert!(r.curves.is_empty());
    assert_eq!(r.max_value(), 0.0);
    assert!(r.scores(0.5).is_empty());
}

#[test]
fn test_ranking_single_node() {
    let mut g = Graph::new(true);
    let a = g.add_node([0.0; 2]);
    let r = Ranking::new(&g);
    assert!(r.curve(a).unwrap().iter().all(|&x| (x - 1.0).abs() < 1e-12));
    assert_eq!(r.dangling, vec![a]);
}

/// Run with `cargo test --release -- --ignored`.
#[test]
#[ignore]
fn test_ranking_speed_100() {
    let mut g = Graph::new(true);
    let ids: Vec<_> = (0..100).map(|i| g.add_node([i as f32, 0.0])).collect();
    for i in 0..100 {
        g.add_arc(ids[i], ids[(i * 7 + 1) % 100]);
        g.add_arc(ids[i], ids[(i * 13 + 5) % 100]);
    }
    let start = std::time::Instant::now();
    let _ = Ranking::new(&g);
    assert!(start.elapsed().as_millis() < 50, "{:?}", start.elapsed());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib rank`
Expected: compilation errors.

- [ ] **Step 3: Implement `sccs`, `terminal_components`, `limit`, the
  α = 1 case of `pagerank`, and `Ranking`.** If `test_ranking_speed_100`
  fails in release, reduce `pagerank` cost by reducing Pᵀ to upper Hessenberg
  form once per `Ranking::new` (then each shifted solve is O(n²)); otherwise
  keep plain LU.

- [ ] **Step 4: Run tests and clippy**

Run: `cargo test --lib rank && cargo test --release --lib rank -- --ignored && cargo clippy --all-features --all-targets -- -D warnings`
Expected: PASS (including the ignored speed test in release), no warnings.

- [ ] **Step 5: Commit**

```bash
git add src
git commit -m "Add terminal components, exact alpha = 1 limit, and Ranking"
```

---

### Task 4: Chung–Lu random graphs

**Files:**
- Create: `src/random/mod.rs`
- Modify: `src/lib.rs` (`pub mod random;`)

**Interfaces:**
- Consumes: `Graph` (Task 1).
- Produces (`pagerank_explorer::random`):
  - `#[derive(Clone, Copy, Debug, PartialEq)] pub struct ChungLu { pub nodes:
    usize, pub avg_degree: f64, pub exponent: f64 }` with
    `Default` = `{ nodes: 10, avg_degree: 2.0, exponent: 2.5 }`.
  - `impl ChungLu { pub fn generate(&self, seed: u64) -> Graph }` — directed,
    labels a, b, …, all positions `[0.0, 0.0]` (the UI lays them out),
    `graph.seed() == Some(seed)`.

Algorithm (fixes draw order, so a seed determines the graph):
`SmallRng::seed_from_u64(seed)`; draw all n out-weights, then all n
in-weights, each as `w = u.powf(-1.0 / (exponent - 1.0))` with
`u = 1.0 - rng.random::<f64>()`; scale each weight vector so it sums to
`W = nodes as f64 * avg_degree`; then for `u` in `0..n`, for `v` in `0..n`,
`u != v`: add arc u→v if `rng.random::<f64>() < (w_out[u] * w_in[v] /
W).min(1.0)`. No resampling.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn test_same_seed_same_graph() {
    let p = ChungLu::default();
    let (g, h) = (p.generate(42), p.generate(42));
    assert_eq!(g.arcs().collect::<Vec<_>>(), h.arcs().collect::<Vec<_>>());
    assert_eq!(g.seed(), Some(42));
}

#[test]
fn test_different_seeds_differ() {
    let p = ChungLu { nodes: 30, ..ChungLu::default() };
    assert_ne!(p.generate(1).arcs().collect::<Vec<_>>(), p.generate(2).arcs().collect::<Vec<_>>());
}

#[test]
fn test_shape() {
    let g = ChungLu { nodes: 50, ..ChungLu::default() }.generate(7);
    assert_eq!(g.len(), 50);
    assert!(g.is_directed());
    assert!(g.arcs().all(|(u, v)| u != v));
    assert_eq!(g.nodes()[0].label, "a");
}

#[test]
fn test_mean_degree() {
    let p = ChungLu { nodes: 300, avg_degree: 3.0, exponent: 3.0 };
    let mean = (0..5).map(|s| p.generate(s).arcs().count() as f64 / 300.0).sum::<f64>() / 5.0;
    assert!((2.7..=3.3).contains(&mean), "{mean}");
}

#[test]
fn test_heavy_tail() {
    let p = ChungLu { nodes: 300, avg_degree: 2.0, exponent: 2.1 };
    let mean_max = (0..5).map(|s| {
        let g = p.generate(s);
        let mut indeg = vec![0usize; g.len()];
        for (_, v) in g.arcs() { indeg[g.index_of(v).unwrap()] += 1; }
        *indeg.iter().max().unwrap() as f64
    }).sum::<f64>() / 5.0;
    assert!(mean_max >= 8.0, "{mean_max}");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib random`
Expected: compilation errors.

- [ ] **Step 3: Implement `ChungLu::generate`.**

- [ ] **Step 4: Run tests and clippy**

Run: `cargo test --lib random && cargo clippy --all-features --all-targets -- -D warnings`
Expected: PASS, no warnings.

- [ ] **Step 5: Commit**

```bash
git add src
git commit -m "Add directed Chung-Lu random graph generator"
```

---

### Task 5: Layouts

**Files:**
- Create: `src/layouts/mod.rs`
- Modify: `src/lib.rs` (`pub mod layouts;`)

**Interfaces:**
- Consumes: `Graph` (Task 1); `sccs`, `terminal_components`, `Chain`
  (Tasks 2–3).
- Produces (`pagerank_explorer::layouts`), each returning positions aligned
  with `graph.nodes()`, all inside `[MARGIN, size - MARGIN]` on both axes,
  deterministic:
  - `pub const MARGIN: f32 = 60.0;`
  - `pub fn circle(graph: &Graph, size: [f32; 2]) -> Vec<[f32; 2]>` — center
    of the canvas, radius `(0.5 * min(width, height) - MARGIN).max(0.0)` (so
    it stays inside the margins at any zoom), first node at the top,
    clockwise.
  - `pub fn force(graph: &Graph, size: [f32; 2]) -> Vec<[f32; 2]>` —
    Fruchterman–Reingold on the underlying undirected graph with a weak pull
    toward the center (keeps disconnected parts on screen), starting from the
    current positions, or from `circle` if any two current positions
    coincide; fixed iteration count with linear cooling; result scaled and
    translated to fit the margins.
  - `pub fn components(graph: &Graph, size: [f32; 2]) -> Vec<[f32; 2]>` —
    the spec's "Components" rule, as an algorithm:
    1. drawn SCCs (`sccs(&graph.successors())`) and their condensation DAG;
       `col(S)` = longest path from a source of the DAG;
    2. if `terminal_components` of the patched chain is a single component
       containing every node, use `col` as is;
    3. otherwise let `m` = max `col` over components that are neither
       patched-terminal nor dangling singletons (−1 if none); dangling
       singletons go to column `m + 1`; patched-terminal components go to
       the last column (`m + 2` if there are dangling nodes, else `m + 1`);
    4. columns evenly spaced across the width; components in a column
       stacked vertically, ordered by smallest node index; nodes of a
       component on a small circle (radius ∝ √size), a singleton at the
       component's center.

- [ ] **Step 1: Write the failing tests**

```rust
fn graph(n: usize, arcs: &[(usize, usize)]) -> (Graph, Vec<NodeId>) {
    let mut g = Graph::new(true);
    let ids: Vec<_> = (0..n).map(|i| g.add_node([10.0 * i as f32, 5.0 * (i % 3) as f32])).collect();
    for &(s, t) in arcs { g.add_arc(ids[s], ids[t]); }
    (g, ids)
}
const SIZE: [f32; 2] = [800.0, 600.0];
fn inside(p: &[[f32; 2]]) -> bool {
    p.iter().all(|&[x, y]| (MARGIN..=SIZE[0] - MARGIN).contains(&x) && (MARGIN..=SIZE[1] - MARGIN).contains(&y))
}

#[test]
fn test_layouts_inside_and_deterministic() {
    let (g, _) = graph(5, &[(0, 1), (1, 2), (2, 0), (2, 3), (3, 4), (4, 3)]);
    for layout in [circle, force, components] {
        let p = layout(&g, SIZE);
        assert_eq!(p.len(), 5);
        assert!(inside(&p));
        assert_eq!(p, layout(&g, SIZE));
    }
}

#[test]
fn test_layouts_empty_and_single() {
    let (g0, _) = graph(0, &[]);
    let (g1, _) = graph(1, &[]);
    for layout in [circle, force, components] {
        assert!(layout(&g0, SIZE).is_empty());
        assert!(inside(&layout(&g1, SIZE)));
    }
}

#[test]
fn test_force_separates_components() {
    let (g, _) = graph(6, &[(0, 1), (1, 2), (2, 0), (3, 4), (4, 5), (5, 3)]);
    let p = force(&g, SIZE);
    let d = |i: usize, j: usize| ((p[i][0] - p[j][0]).powi(2) + (p[i][1] - p[j][1]).powi(2)).sqrt();
    assert!(d(0, 1) < d(0, 3) && d(3, 4) < d(2, 5));
}

#[test]
fn test_components_terminal_right() {
    let (g, _) = graph(5, &[(0, 1), (1, 2), (2, 0), (2, 3), (3, 4), (4, 3)]);
    let p = components(&g, SIZE);
    assert!((0..3).all(|i| p[i][0] < p[3][0] && p[i][0] < p[4][0]));
}

#[test]
fn test_components_dangling_left_of_terminal() {
    // a→b→c (c dangling), a→d, d↔e closed
    let (g, _) = graph(5, &[(0, 1), (1, 2), (0, 3), (3, 4), (4, 3)]);
    let p = components(&g, SIZE);
    assert!(p[1][0] < p[2][0] && p[2][0] < p[3][0]);
}

#[test]
fn test_components_single_terminal_follows_dag() {
    let (g, _) = graph(2, &[(0, 1)]); // b dangling: one patched terminal component
    let p = components(&g, SIZE);
    assert!(p[0][0] < p[1][0]);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib layouts`
Expected: compilation errors.

- [ ] **Step 3: Implement `circle`, `force`, `components`.**

- [ ] **Step 4: Run tests and clippy**

Run: `cargo test --lib layouts && cargo clippy --all-features --all-targets -- -D warnings`
Expected: PASS, no warnings.

- [ ] **Step 5: Commit**

```bash
git add src
git commit -m "Add circle, force-directed, and components layouts"
```

---

### Task 6: Session (undo/redo, ranking, before/after curves)

**Files:**
- Create: `src/session/mod.rs` (private module; `pub use session::Session;`
  in `src/lib.rs`)

**Interfaces:**
- Consumes: `Graph` (Task 1), `Ranking` (Task 3).
- Produces (`pagerank_explorer::Session`):
  - fields (private): `graph: Graph, ranking: Ranking, ghost:
    Option<Ranking>, scores: Vec<f64>, alpha: f64, undo: Vec<Graph>, redo:
    Vec<Graph>, drag_origin: Option<Graph>, path: Option<PathBuf>, dirty:
    bool`.
  - `pub const DEFAULT_ALPHA: f64 = 0.85;`
  - `pub fn new(directed: bool) -> Self`; `pub fn with_graph(graph: Graph,
    alpha: f64, path: impl Into<Option<PathBuf>>) -> Self` (fresh history,
    not dirty);
  - accessors: `graph() -> &Graph`, `ranking() -> &Ranking`,
    `ghost() -> Option<&Ranking>`, `scores() -> &[f64]` (cached
    `ranking.scores(alpha)`, aligned with `ranking.ids`), `alpha() -> f64`,
    `path() -> Option<&Path>`, `is_dirty() -> bool`, `can_undo`, `can_redo`;
  - `pub fn set_alpha(&mut self, alpha: f64)` — clamps to [0, 1], refreshes
    `scores`; not undoable, does not set `dirty`;
  - `pub fn commit(&mut self, new: Graph)` — no-op if `new == graph`;
    otherwise push the current graph on `undo`, clear `redo`, install `new`,
    set `dirty`;
  - `pub fn undo(&mut self) -> bool`, `pub fn redo(&mut self) -> bool` —
    move graphs between the stacks and install, set `dirty`;
  - `pub fn begin_drag(&mut self)`, `pub fn drag_to(&mut self, id: NodeId,
    pos: [f32; 2])` (moves in place, no history), `pub fn end_drag(&mut
    self)` (one undo step if any position changed);
  - `pub fn mark_saved(&mut self, path: PathBuf)` — sets `path`, clears
    `dirty`.

Install rule (private `fn install(&mut self, new: Graph)`), the spec's
"Showing the change": if `new.generation() != graph.generation()` → new
ranking, `ghost = None`; else if `!new.same_structure(&graph)` → `ghost =
Some(old ranking)`, new ranking; else keep ranking and ghost (labels and
positions only). Refresh `scores` whenever the ranking changes.

- [ ] **Step 1: Write the failing tests**

```rust
fn two_nodes() -> (Session, NodeId, NodeId) {
    let mut s = Session::new(true);
    let mut g = s.graph().clone();
    let (a, b) = (g.add_node([0.0; 2]), g.add_node([100.0, 0.0]));
    s.commit(g);
    (s, a, b)
}
fn with_arc(s: &Session, a: NodeId, b: NodeId) -> Graph {
    let mut g = s.graph().clone();
    g.toggle_arc(a, b);
    g
}

#[test]
fn test_commit_undo_redo() {
    let (mut s, a, b) = two_nodes();
    s.commit(with_arc(&s, a, b));
    assert!(s.graph().has_arc(a, b));
    assert!(s.undo());
    assert!(!s.graph().has_arc(a, b));
    assert!(s.redo());
    assert!(s.graph().has_arc(a, b));
}

#[test]
fn test_commit_identical_is_noop() {
    let mut s = Session::new(true);
    s.commit(s.graph().clone());
    assert!(!s.can_undo());
    assert!(!s.is_dirty());
}

#[test]
fn test_redo_cleared_on_commit() {
    let (mut s, a, b) = two_nodes();
    s.commit(with_arc(&s, a, b));
    s.undo();
    s.commit(with_arc(&s, b, a));
    assert!(!s.can_redo());
}

#[test]
fn test_ghost_after_structural_edit() {
    let (mut s, a, b) = two_nodes();
    let before = s.ranking().curve(a).unwrap().to_vec();
    s.commit(with_arc(&s, a, b));
    assert_eq!(s.ghost().unwrap().curve(a).unwrap(), &before[..]);
}

#[test]
fn test_ghost_kept_on_move() {
    let (mut s, a, b) = two_nodes();
    s.commit(with_arc(&s, a, b));
    let ghost = s.ghost().unwrap().curve(a).unwrap().to_vec();
    let mut g = s.graph().clone();
    g.set_pos(a, [50.0, 50.0]);
    s.commit(g);
    assert_eq!(s.ghost().unwrap().curve(a).unwrap(), &ghost[..]);
}

#[test]
fn test_ghost_cleared_on_new_generation() {
    let (mut s, a, b) = two_nodes();
    s.commit(with_arc(&s, a, b));
    s.commit(Graph::new(true));
    assert!(s.ghost().is_none());
}

#[test]
fn test_undo_shows_edited_state_as_ghost() {
    let (mut s, a, b) = two_nodes();
    s.commit(with_arc(&s, a, b));
    let edited = s.ranking().curve(a).unwrap().to_vec();
    s.undo();
    assert_eq!(s.ghost().unwrap().curve(a).unwrap(), &edited[..]);
}

#[test]
fn test_ghost_lookup_removed_node() {
    let (mut s, a, _) = two_nodes();
    let mut g = s.graph().clone();
    g.remove_node(a);
    s.commit(g);
    assert!(s.ranking().curve(a).is_none());
    assert!(s.ghost().unwrap().curve(a).is_some());
}

#[test]
fn test_drag_is_one_step() {
    let (mut s, a, _) = two_nodes();
    s.begin_drag();
    for x in [10.0, 20.0, 30.0] { s.drag_to(a, [x, 0.0]); }
    s.end_drag();
    assert_eq!(s.graph().node(a).unwrap().pos, [30.0, 0.0]);
    assert!(s.undo());
    assert_eq!(s.graph().node(a).unwrap().pos, [0.0, 0.0]);
}

#[test]
fn test_scores_at_alpha_one() {
    let (mut s, a, b) = two_nodes();
    s.commit(with_arc(&s, a, b)); // b dangling
    s.set_alpha(1.0);
    assert!(s.scores().iter().all(|x| x.is_finite()));
    assert!((s.scores().iter().sum::<f64>() - 1.0).abs() < 1e-12);
    s.set_alpha(7.0);
    assert_eq!(s.alpha(), 1.0);
}

#[test]
fn test_dirty() {
    let (mut s, _, _) = two_nodes();
    assert!(s.is_dirty());
    s.mark_saved("x.json".into());
    assert!(!s.is_dirty());
    s.set_alpha(0.5);
    assert!(!s.is_dirty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib session`
Expected: compilation errors.

- [ ] **Step 3: Implement `Session`.**

- [ ] **Step 4: Run tests and clippy**

Run: `cargo test --lib session && cargo clippy --all-features --all-targets -- -D warnings`
Expected: PASS, no warnings.

- [ ] **Step 5: Commit**

```bash
git add src
git commit -m "Add session with undo/redo and before/after rankings"
```

---

### Task 7: JSON files

**Files:**
- Create: `src/files/mod.rs`
- Modify: `src/lib.rs` (`pub mod files;`), `src/session/mod.rs`
- Modify: `Cargo.toml` (dev-dependency `tempfile = "3"`)

**Interfaces:**
- Consumes: `Graph::from_parts`, `Node`, `NodeId` (Task 1); `Session`
  (Task 6).
- Produces (`pagerank_explorer::files`):
  - `pub fn to_json(graph: &Graph, alpha: f64) -> String` (pretty-printed);
  - `pub fn from_json(json: impl AsRef<str>) -> anyhow::Result<(Graph, f64)>`;
  - `pub fn save(graph: &Graph, alpha: f64, path: impl AsRef<Path>) ->
    anyhow::Result<()>`;
  - `pub fn load(path: impl AsRef<Path>) -> anyhow::Result<(Graph, f64)>`;
  - `Session::open(path: impl AsRef<Path>) -> anyhow::Result<Session>`
    (via `with_graph`, history reset — opening is not undoable);
    `Session::save(&mut self) -> anyhow::Result<()>` (error if no path);
    `Session::save_as(&mut self, path: impl AsRef<Path>) ->
    anyhow::Result<()>`; both call `mark_saved`.
- File format (version 1), private serde DTO:
  `{ "version": 1, "directed": bool, "alpha": f64, "seed": u64 | null,
  "nodes": [{ "id", "label", "pos", "color" }], "arcs": [[from, to], …] }`.
  `from_json` rejects `version != 1` and anything `Graph::from_parts`
  rejects; clamps `alpha` to [0, 1]; the loaded graph has a fresh generation.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn test_roundtrip() -> anyhow::Result<()> {
    let mut g = Graph::new(false);
    let (a, b) = (g.add_node([1.0, 2.0]), g.add_node([3.0, 4.0]));
    g.toggle_arc(a, b);
    g.set_label(a, "home");
    g.set_seed(Some(9));
    let (h, alpha) = from_json(to_json(&g, 0.7))?;
    assert_eq!(alpha, 0.7);
    assert!(h.same_structure(&g));
    assert_eq!(h.nodes(), g.nodes());
    assert_eq!(h.seed(), Some(9));
    assert!(!h.is_directed());
    assert_ne!(h.generation(), g.generation());
    Ok(())
}

#[test]
fn test_load_rejects_bad_version() {
    assert!(from_json(r#"{"version":2,"directed":true,"alpha":0.85,"seed":null,"nodes":[],"arcs":[]}"#).is_err());
}

#[test]
fn test_load_rejects_unknown_arc_id() {
    assert!(from_json(r#"{"version":1,"directed":true,"alpha":0.85,"seed":null,
        "nodes":[{"id":0,"label":"a","pos":[0,0],"color":0}],"arcs":[[0,3]]}"#).is_err());
}

#[test]
fn test_load_rejects_duplicate_node_id() {
    assert!(from_json(r#"{"version":1,"directed":true,"alpha":0.85,"seed":null,
        "nodes":[{"id":0,"label":"a","pos":[0,0],"color":0},{"id":0,"label":"b","pos":[0,0],"color":1}],"arcs":[]}"#).is_err());
}

#[test]
fn test_load_rejects_malformed_json() {
    assert!(from_json("{ not json").is_err());
    assert!(from_json(r#"{"version":1}"#).is_err());
}

#[test]
fn test_session_save_open() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("lecture.json");
    let mut s = Session::new(true);
    let mut g = s.graph().clone();
    g.add_node([5.0, 5.0]);
    s.commit(g);
    s.save_as(&path)?;
    assert!(!s.is_dirty());
    let t = Session::open(&path)?;
    assert_eq!(t.graph().len(), 1);
    assert_eq!(t.path(), Some(path.as_path()));
    assert!(!t.can_undo());
    Ok(())
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib files`
Expected: compilation errors.

- [ ] **Step 3: Implement the DTO, `to_json`/`from_json`/`save`/`load`, and
  `Session::{open, save, save_as}`.**

- [ ] **Step 4: Run tests and clippy**

Run: `cargo test --lib && cargo clippy --all-features --all-targets -- -D warnings`
Expected: PASS, no warnings.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src
git commit -m "Add JSON save and load"
```

---

### Task 8: Editing gestures (GUI-free state machine)

**Files:**
- Create: `src/edit/mod.rs`
- Modify: `src/lib.rs` (`pub mod edit;`)

**Interfaces:**
- Consumes: `Graph`, `NodeId` (Task 1).
- Produces (`pagerank_explorer::edit`):
  - `#[derive(Clone, Debug, PartialEq)] pub enum Gesture { Idle,
    PendingEdge { from: NodeId }, Renaming { node: NodeId, text: String } }`
  - `#[derive(Clone, Copy, Debug, PartialEq)] pub enum Target { Node(NodeId),
    Arc(NodeId, NodeId), Empty }`
  - `#[derive(Clone, Copy, Debug, PartialEq)] pub enum EditKey { Char(char),
    Backspace, Delete, Enter, Escape }`
  - `#[derive(Debug, Default)] pub struct Editor { gesture: Gesture }`
    (`Gesture::Idle` is the default)
  - `impl Editor`: `pub fn gesture(&self) -> &Gesture`;
    `pub fn click(&mut self, graph: &Graph, target: Target) -> Option<Graph>`;
    `pub fn double_click_empty(&mut self, graph: &Graph, pos: [f32; 2]) ->
    Graph`; `pub fn key(&mut self, graph: &Graph, hovered: Option<NodeId>,
    key: EditKey) -> Option<Graph>`; `pub fn start_rename(&mut self, graph:
    &Graph, node: NodeId)` (text = current label); `pub fn cancel(&mut
    self)`; `pub fn prune(&mut self, graph: &Graph)` (back to `Idle` if the
    gesture refers to a node not in `graph`);
  - free functions for the context menu: `pub fn toggle_self_loop(graph:
    &Graph, node: NodeId) -> Graph`, `pub fn delete_node(graph: &Graph, node:
    NodeId) -> Graph`, `pub fn delete_arc(graph: &Graph, from: NodeId, to:
    NodeId) -> Graph`.

Returned graphs are modified copies for `Session::commit`. Transition table
(the spec's "Editing", plus calls marked "plan's call"):

| State | Input | Result |
|---|---|---|
| Idle | click Node(x) | PendingEdge{x} |
| PendingEdge{x} | click Node(y), y ≠ x | toggle x→y, Idle |
| PendingEdge{x} | click Node(x) | Idle (no self-loop; plan's call) |
| Idle | click Arc(u, v) | remove u→v (both directions if undirected), Idle |
| PendingEdge | click Arc | Idle, no change (plan's call) |
| any but Renaming | click Empty | Idle |
| Renaming | any click | commit rename if text non-empty, Idle (plan's call) |
| Idle/PendingEdge | Char(c) alphanumeric, hovered n | Renaming{n, c} |
| Renaming | Char(c) / Backspace | push / pop |
| Renaming | Enter | set label if non-empty, Idle |
| Renaming | Escape | Idle, no change |
| Idle/PendingEdge | Backspace/Delete, hovered n | delete n, Idle |
| any | Escape | Idle |

- [ ] **Step 1: Write the failing tests**

```rust
fn setup() -> (Graph, NodeId, NodeId) {
    let mut g = Graph::new(true);
    let (a, b) = (g.add_node([0.0; 2]), g.add_node([100.0, 0.0]));
    (g, a, b)
}

#[test]
fn test_click_click_toggles() {
    let (g, a, b) = setup();
    let mut e = Editor::default();
    assert_eq!(e.click(&g, Target::Node(a)), None);
    assert_eq!(e.gesture(), &Gesture::PendingEdge { from: a });
    let g = e.click(&g, Target::Node(b)).unwrap();
    assert!(g.has_arc(a, b));
    e.click(&g, Target::Node(a));
    let g = e.click(&g, Target::Node(b)).unwrap();
    assert!(!g.has_arc(a, b));
    assert_eq!(e.gesture(), &Gesture::Idle);
}

#[test]
fn test_click_same_node_cancels() {
    let (g, a, _) = setup();
    let mut e = Editor::default();
    e.click(&g, Target::Node(a));
    assert_eq!(e.click(&g, Target::Node(a)), None);
    assert_eq!(e.gesture(), &Gesture::Idle);
}

#[test]
fn test_click_arc_removes() {
    let (mut g, a, b) = setup();
    g.add_arc(a, b);
    let mut e = Editor::default();
    assert!(!e.click(&g, Target::Arc(a, b)).unwrap().has_arc(a, b));
}

#[test]
fn test_escape_and_empty_cancel() {
    let (g, a, _) = setup();
    let mut e = Editor::default();
    e.click(&g, Target::Node(a));
    e.click(&g, Target::Empty);
    assert_eq!(e.gesture(), &Gesture::Idle);
    e.click(&g, Target::Node(a));
    assert_eq!(e.key(&g, None, EditKey::Escape), None);
    assert_eq!(e.gesture(), &Gesture::Idle);
}

#[test]
fn test_rename() {
    let (g, a, _) = setup();
    let mut e = Editor::default();
    assert_eq!(e.key(&g, Some(a), EditKey::Char('h')), None);
    e.key(&g, Some(a), EditKey::Char('x'));
    e.key(&g, Some(a), EditKey::Backspace);
    e.key(&g, Some(a), EditKey::Char('i'));
    let g = e.key(&g, Some(a), EditKey::Enter).unwrap();
    assert_eq!(g.node(a).unwrap().label, "hi");
}

#[test]
fn test_rename_escape_keeps_label() {
    let (g, a, _) = setup();
    let mut e = Editor::default();
    e.key(&g, Some(a), EditKey::Char('z'));
    assert_eq!(e.key(&g, Some(a), EditKey::Escape), None);
    assert_eq!(g.node(a).unwrap().label, "a");
}

#[test]
fn test_delete_hovered() {
    let (g, a, _) = setup();
    let mut e = Editor::default();
    let g = e.key(&g, Some(a), EditKey::Backspace).unwrap();
    assert!(g.node(a).is_none());
    assert_eq!(e.key(&g, None, EditKey::Delete), None);
}

#[test]
fn test_double_click_adds_node() {
    let (g, _, _) = setup();
    let g = Editor::default().double_click_empty(&g, [42.0, 7.0]);
    assert_eq!(g.len(), 3);
    assert_eq!(g.nodes()[2].pos, [42.0, 7.0]);
}

#[test]
fn test_prune_drops_stale_ids() {
    let (g, a, _) = setup();
    let mut e = Editor::default();
    e.click(&g, Target::Node(a));
    let mut h = g.clone();
    h.remove_node(a);
    e.prune(&h);
    assert_eq!(e.gesture(), &Gesture::Idle);
}

#[test]
fn test_toggle_self_loop() {
    let (g, a, _) = setup();
    let g = toggle_self_loop(&g, a);
    assert!(g.has_arc(a, a));
    assert!(!toggle_self_loop(&g, a).has_arc(a, a));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib edit`
Expected: compilation errors.

- [ ] **Step 3: Implement `Editor` and the free functions** per the table.

- [ ] **Step 4: Run tests and clippy**

Run: `cargo test --lib edit && cargo clippy --all-features --all-targets -- -D warnings`
Expected: PASS, no warnings.

- [ ] **Step 5: Commit**

```bash
git add src
git commit -m "Add editing gesture state machine"
```

---

### Task 9: Application shell (window, panels, menus, shortcuts, α slider)

**Files:**
- Create: `src/ui/mod.rs`, `src/ui/palette.rs`
- Modify: `src/lib.rs` (`pub mod ui;`), `src/main.rs`
- Modify: `Cargo.toml` (dev-dependency `egui_kittest`)
- Test: `tests/test_ui.rs`

**Interfaces:**
- Consumes: `Session` (Tasks 6–7), `Editor` (Task 8), `layouts` (Task 5).
- Produces (`pagerank_explorer::ui`):
  - `pub fn run() -> eframe::Result` (window title "PageRank Explorer",
    initial size 1280×720); `src/main.rs` calls it.
  - `pub struct App` with fields `session: Session, editor: Editor, hovered:
    Option<NodeId>, fixed_y: bool, canvas_rect: egui::Rect` (Task 10 adds
    `animation`, Task 12 adds dialog state).
  - `impl App`: `pub fn new(cc: &eframe::CreationContext<'_>) -> Self`
    (light visuals, text styles Body/Button 20, Heading 26, Monospace 18,
    Small 15); `pub fn session(&self) -> &Session`; `pub fn
    session_mut(&mut self) -> &mut Session`; `pub fn canvas_rect(&self) ->
    egui::Rect` (last frame's canvas rectangle); `pub fn apply_layout(&mut
    self, layout: fn(&Graph, [f32; 2]) -> Vec<[f32; 2]>)` (commits the new
    positions computed for the canvas size; Task 10 makes it also start an
    animation from the old positions).
  - `impl eframe::App for App` (`fn ui`): top menu bar, right panel with the
    plot (Task 11) above the α slider (exactly half the window width, not
    resizable), central panel with the canvas (Task 10). Until Tasks 10–11
    land, the panels may show placeholders.
  - `palette.rs`: `pub const PALETTE: [egui::Color32; PALETTE_SIZE]` and
    `pub fn color(index: usize) -> egui::Color32` (index modulo size).
    Palette constraints: saturated, mutually distinct when projected, no
    grays (gray reads as "ghost"), no light yellows, white labels legible on
    every color.
- α slider: `egui::Slider` over [0, 1] writing `session.set_alpha`, with the
  value shown large ("α = 0.85", Heading size). ←/→ change α by 0.01,
  Shift-←/→ by 0.05, only when no text field has focus and the editor is not
  renaming.
- Shortcuts (Cmd on macOS, Ctrl elsewhere: `egui::Modifiers::COMMAND`):
  Cmd-N new graph dialog, Cmd-O open, Cmd-S save, Cmd-Shift-S save as,
  Cmd-Z undo, Cmd-Shift-Z redo, Cmd-R random graph dialog, Cmd-1 force
  layout, Cmd-2 circle layout, Cmd-3 components layout, Cmd-Shift-F
  full screen. Zoom uses egui's built-in Cmd +/−/0. Consume shifted
  variants before unshifted ones (`consume_shortcut` matches logically).
  The menus (File, Edit, Graph, View) list the same commands; View also has
  "Fixed y-axis [0, 1]" (`fixed_y`). Dialog commands are stubs until
  Task 12.
- After every undo/redo, call `editor.prune(session.graph())` and clear
  `hovered` if the node is gone.

- [ ] **Step 1: Write the failing tests** (`tests/test_ui.rs`)

```rust
use egui_kittest::Harness;
use pagerank_explorer::ui::App;

fn harness() -> Harness<'static, App> {
    Harness::builder().with_size([1280.0, 720.0]).build_eframe(|cc| App::new(cc))
}

#[test]
fn test_app_starts() {
    let mut h = harness();
    h.run();
    assert!(h.state().session().graph().is_empty());
    assert_eq!(h.state().session().alpha(), 0.85);
}

#[test]
fn test_undo_shortcut() {
    let mut h = harness();
    let mut g = h.state().session().graph().clone();
    g.add_node([100.0, 100.0]);
    h.state_mut().session_mut().commit(g);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    h.run();
    assert!(h.state().session().graph().is_empty());
    h.key_press_modifiers(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Z);
    h.run();
    assert_eq!(h.state().session().graph().len(), 1);
}

#[test]
fn test_alpha_arrows() {
    let mut h = harness();
    h.run();
    h.key_press(egui::Key::ArrowRight);
    h.run();
    assert!((h.state().session().alpha() - 0.86).abs() < 1e-9);
}
```

(Add `egui` as a dev-dependency too if the test crate needs it by name, or
use `egui_kittest::kittest`/re-exports.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test test_ui`
Expected: compilation errors.

- [ ] **Step 3: Implement `run`, `App` (shell only), `palette.rs`,
  `main.rs`.**

- [ ] **Step 4: Run tests, clippy, and the app**

Run: `cargo test && cargo clippy --all-features --all-targets -- -D warnings && cargo run`
Expected: tests pass, no warnings; a 1280×720 light window with menus, an
empty right panel with the α slider, and an empty canvas. Close it.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src tests
git commit -m "Add application shell with menus, shortcuts, and alpha slider"
```

---

### Task 10: Graph canvas

**Files:**
- Create: `src/ui/canvas.rs`, `src/ui/geometry.rs`
- Modify: `src/ui/mod.rs`
- Test: `tests/test_ui.rs`

**Interfaces:**
- Consumes: `App` fields (Task 9); `Editor`, `Target`, `EditKey`,
  free functions (Task 8); `Session` (Task 6); `palette::color` (Task 9).
- Produces:
  - `geometry.rs` (pure, unit-tested): `pub fn node_radius(score: f64, n:
    usize) -> f32` = `(R0 * (score * n as f64).sqrt() as f32).clamp(R_MIN,
    R_MAX)` with `R0 = 24.0`, `R_MIN = 14.0`, `R_MAX = 64.0`;
    `pub fn dist_to_segment(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32`;
    `pub fn hull(circles: &[([f32; 2], f32)], pad: f32) -> Vec<[f32; 2]>`
    (convex hull, counter-clockwise, of 16 points sampled on each circle of
    radius r + pad; Andrew's monotone chain); `pub fn ease(t: f32) -> f32`
    (cubic ease-in-out, clamps t to [0, 1]).
  - `pub(crate) struct Animation { from: HashMap<NodeId, [f32; 2]>, start:
    f64 }` stored in a new `App` field `animation: Option<Animation>`, set by
    `apply_layout`; `DURATION = 1.0` s; drawn position = lerp(from, current,
    ease(elapsed / DURATION)); nodes absent from `from` start at the canvas
    center; any pointer press on the canvas ends the animation; request
    repaints while it runs.
  - `pub(crate) fn canvas(app: &mut App, ui: &mut egui::Ui)`.

Drawing (back to front): terminal-component hulls (`ranking.terminal`, fill
= the light tint of the color of the component's first node at ~15% opacity,
dashed outline, small italic label "terminal" under the hull); arcs (stroke
2.5 pt, dark gray; arrowheads 14 pt long and 7 pt half-width ending at the
target's rim; the two arcs of an opposite pair offset ±6 pt perpendicular;
self-loops as a 14 pt loop above the node; undirected graphs draw one line
per pair without arrowheads); nodes (filled `palette::color(node.color)`,
radius `node_radius(scores[i], n)`, 2 pt dark outline, dashed outline instead
if dangling, white label inside at 20 pt; the pending-edge source gets a
3 pt accent ring and a rubber-band arrow to the pointer; the hovered node
gets a thicker outline).

Interaction: hit-test nodes (distance ≤ radius + 4) before arcs (distance ≤
8 pt to the drawn segment); map egui events to the `Editor`: primary click →
`click(Target)`, double-click on empty → `double_click_empty` at the
canvas-local position, typed text and Backspace/Delete/Enter/Escape →
`key(…, hovered, …)` (whenever no other widget has keyboard focus, e.g., a
dialog text field or the slider; hovering alone is enough, no click needed);
commit any returned graph. Hovering a node shows a tooltip with its label
and score at the current α (four decimals). Primary drag on a node → `begin_drag`,
`drag_to` with canvas-local positions, `end_drag` on release (a drag never
creates an edge). Right-click menus: on a node — "Toggle self-loop",
"Rename", "Delete node"; on an arc — "Delete edge"; on empty canvas —
"Add node here", "Random graph…", "Layout ▸" (Force-directed, Circle,
Components), "Undo", "Redo". While renaming, draw an inline text field over
the node showing the editor's text. Set `app.hovered` from the canvas each
frame (Task 11 may also set it from the plot). Node positions are
canvas-local: screen = `canvas_rect.min + pos`.

- [ ] **Step 1: Write the failing tests**

Unit tests in `src/ui/geometry.rs`:

```rust
#[test]
fn test_node_radius() {
    assert_eq!(node_radius(0.2, 5), 24.0);
    assert_eq!(node_radius(0.0, 5), 14.0);
    assert_eq!(node_radius(1.0, 50), 64.0);
}

#[test]
fn test_dist_to_segment() {
    assert_eq!(dist_to_segment([5.0, 3.0], [0.0, 0.0], [10.0, 0.0]), 3.0);
    assert_eq!(dist_to_segment([-4.0, 3.0], [0.0, 0.0], [10.0, 0.0]), 5.0);
}

#[test]
fn test_hull() {
    let h = hull(&[([0.0, 0.0], 10.0)], 5.0);
    assert!(h.iter().all(|&[x, y]| ((x * x + y * y).sqrt() - 15.0).abs() < 1e-3));
    let h = hull(&[([0.0, 0.0], 10.0), ([100.0, 0.0], 10.0)], 5.0);
    let xs: Vec<f32> = h.iter().map(|p| p[0]).collect();
    assert!(xs.iter().cloned().fold(f32::MAX, f32::min) <= -14.9);
    assert!(xs.iter().cloned().fold(f32::MIN, f32::max) >= 114.9);
}

#[test]
fn test_ease() {
    assert_eq!(ease(0.0), 0.0);
    assert_eq!(ease(1.0), 1.0);
    assert_eq!(ease(2.0), 1.0);
    assert!((ease(0.5) - 0.5).abs() < 1e-6);
}
```

Integration tests appended to `tests/test_ui.rs` (helper `click_at(h, pos)`
= `hover_at`, `drag_at`, `run`, `drop_at`, `run`; screen position =
`h.state().canvas_rect().min + egui::vec2(x, y)`):

```rust
fn two_node_app() -> (Harness<'static, App>, NodeId, NodeId) {
    let mut h = harness();
    h.run();
    let mut g = h.state().session().graph().clone();
    let (a, b) = (g.add_node([150.0, 300.0]), g.add_node([400.0, 300.0]));
    h.state_mut().session_mut().commit(g);
    h.run();
    (h, a, b)
}

#[test]
fn test_canvas_click_click_toggles_arc() {
    let (mut h, a, b) = two_node_app();
    click_at(&mut h, [150.0, 300.0]);
    click_at(&mut h, [400.0, 300.0]);
    assert!(h.state().session().graph().has_arc(a, b));
    click_at(&mut h, [150.0, 300.0]);
    click_at(&mut h, [400.0, 300.0]);
    assert!(!h.state().session().graph().has_arc(a, b));
}

#[test]
fn test_canvas_drag_moves_node() {
    let (mut h, a, _) = two_node_app();
    // drag_at at the node, hover along the way, drop 80 pt to the right
    drag(&mut h, [150.0, 300.0], [230.0, 300.0]);
    let pos = h.state().session().graph().node(a).unwrap().pos;
    assert!((pos[0] - 230.0).abs() < 2.0 && (pos[1] - 300.0).abs() < 2.0);
    assert!(!h.state().session().graph().has_arc(a, a));
    h.state_mut().session_mut().undo();
    assert_eq!(h.state().session().graph().node(a).unwrap().pos, [150.0, 300.0]);
}

#[test]
fn test_canvas_backspace_deletes_hovered() {
    let (mut h, a, _) = two_node_app();
    hover(&mut h, [150.0, 300.0]);
    h.key_press(egui::Key::Backspace);
    h.run();
    assert!(h.state().session().graph().node(a).is_none());
}
```

(`drag` and `hover` are helpers like `click_at`. Double-click is covered by
`Editor::double_click_empty` in Task 8; add a harness test for it only if
kittest can simulate two clicks within egui's double-click interval.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test`
Expected: compilation errors / failing canvas tests.

- [ ] **Step 3: Implement `geometry.rs`, `Animation`, and `canvas`**; wire
  `canvas` into the central panel and `apply_layout` into the menus,
  shortcuts, and context menu.

- [ ] **Step 4: Run tests, clippy, and the app**

Run: `cargo test && cargo clippy --all-features --all-targets -- -D warnings && cargo run`
Expected: tests pass, no warnings. Manually: double-click adds nodes a, b, c;
click–click adds and removes arcs; opposite arcs are both visible; dragging
moves nodes; Cmd-1/2/3 animate layouts over about a second; a closed pair
gets a "terminal" hull; a dangling node has a dashed outline.

- [ ] **Step 5: Commit**

```bash
git add src tests
git commit -m "Add graph canvas with drawing, editing, and layout animation"
```

---

### Task 11: Curve plot

**Files:**
- Create: `src/ui/plot.rs`
- Modify: `src/ui/mod.rs`

**Interfaces:**
- Consumes: `Session::{ranking, ghost, scores, alpha, set_alpha}` (Task 6),
  `App::{hovered, fixed_y}` (Task 9), `palette::color` (Task 9).
- Produces:
  - pure helpers (unit-tested): `pub fn nice_ceiling(x: f64) -> f64` — the
    smallest multiple of 0.1 that is ≥ x, at least 0.1, at most 1.0;
    `pub fn group_end_labels(values: &[(usize, f64)], min_gap: f64) ->
    Vec<(f64, Vec<usize>)>` — sorts by value and merges entries closer than
    `min_gap` into one group placed at the group's mean;
    `pub fn nearest_curve(ranking: &Ranking, alpha: f64, y: f64, tol: f64)
    -> Option<usize>` — index of the curve whose (linearly interpolated)
    value at `alpha` is closest to `y`, if within `tol`.
  - `pub(crate) fn plot(app: &mut App, ui: &mut egui::Ui)`.

Behavior (the spec's "Pairing nodes and curves" and "Showing the change"):
`egui_plot::Plot` with pan, zoom, scroll, and boxed zoom disabled; x from 0
to 1 with room on the right for end labels (show x up to 1.1, ticks only on
[0, 1]); y from 0 to `if fixed_y { 1.0 } else { nice_ceiling(max of ranking
and ghost) }`; axis label "α". Ghost curves first (same color, ~45% opacity,
2 pt, dashed), only for ids present in the current ranking; current curves
3 pt; when `hovered` is set its curve is 5.5 pt and the others drop to ~25%
opacity; a vertical line at α with a dot (node color, white outline) where
it crosses each curve at `session.scores()`; end labels at x ≈ 1.02 from
`group_end_labels` with `min_gap` equal to about 18 px in plot units, each
label's node names in their own colors (e.g., "d e"). Pointer hover over a
curve (via `nearest_curve`, tolerance about 10 px in plot units) sets
`hovered`; primary click or drag inside the plot with x in [0, 1] sets α.

- [ ] **Step 1: Write the failing tests** (in `src/ui/plot.rs`)

```rust
#[test]
fn test_nice_ceiling() {
    assert!((nice_ceiling(0.37) - 0.4).abs() < 1e-12);
    assert!((nice_ceiling(0.5) - 0.5).abs() < 1e-12);
    assert!((nice_ceiling(0.0) - 0.1).abs() < 1e-12);
    assert!((nice_ceiling(1.0) - 1.0).abs() < 1e-12);
}

#[test]
fn test_group_end_labels() {
    let groups = group_end_labels(&[(0, 0.0), (1, 0.5), (2, 0.001), (3, 0.5)], 0.02);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].1, vec![0, 2]);
    assert_eq!(groups[1].1, vec![1, 3]);
    assert!((groups[1].0 - 0.5).abs() < 1e-12);
}

#[test]
fn test_nearest_curve() {
    let mut g = Graph::new(true);
    let ids: Vec<_> = (0..5).map(|i| g.add_node([i as f32, 0.0])).collect();
    for (s, t) in [(0, 1), (1, 2), (2, 0), (2, 3), (3, 4), (4, 3)] { g.add_arc(ids[s], ids[t]); }
    let r = Ranking::new(&g);
    assert_eq!(nearest_curve(&r, 1.0, 0.0, 0.01).map(|i| i < 3), Some(true));
    assert_eq!(nearest_curve(&r, 0.85, 0.9, 0.05), None);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib plot`
Expected: compilation errors.

- [ ] **Step 3: Implement the helpers and `plot`**; wire it into the right
  panel above the slider.

- [ ] **Step 4: Run tests, clippy, and the app**

Run: `cargo test && cargo clippy --all-features --all-targets -- -D warnings && cargo run`
Expected: tests pass, no warnings. Manually build the spec's five-node
example (a→b→c→a, c→d, d→e) and add e→d: d and e rise to ½ at α = 1 while a,
b, c drop to 0; the previous curves stay dashed; hovering a node or curve
highlights the pair; dragging on the plot moves α and resizes nodes.

- [ ] **Step 5: Commit**

```bash
git add src
git commit -m "Add PageRank curve plot with before/after curves"
```

---

### Task 12: Dialogs, files in the UI, window title

**Files:**
- Create: `src/ui/dialogs.rs`
- Modify: `src/ui/mod.rs`
- Test: `tests/test_ui.rs`

**Interfaces:**
- Consumes: `Session::{open, save, save_as, commit, path, is_dirty}`
  (Tasks 6–7), `ChungLu` (Task 4), `layouts::force` (Task 5),
  `App::apply_layout`/`Animation` (Tasks 9–10).
- Produces:
  - `pub fn window_title(path: Option<&Path>, dirty: bool) -> String` —
    `"PageRank Explorer"` without a path; `"PageRank Explorer — name.json"`
    with one; a trailing `*` when dirty and a path exists.
  - `impl App { pub fn generate_random(&mut self, params: ChungLu, seed: u64) }`
    — generates, applies `layouts::force` with the current canvas size,
    commits (undoable), animates nodes from the canvas center.
  - Dialog state in `App`: new-graph dialog (Directed / Undirected radio,
    OK/Cancel → `commit(Graph::new(directed))`); random-graph dialog
    (`DragValue`s: nodes 2..=100, average degree 0.5..=10.0 step 0.1,
    exponent 2.0..=4.0 step 0.1, seed as u64 with a "New seed" button that
    draws from the system clock; "Generate" calls `generate_random`; the
    dialog stays open so the instructor can regenerate); error message
    window for failed open/save; unsaved-changes confirmation (Save /
    Discard / Cancel) before Open and on window close
    (`close_requested` → `ViewportCommand::CancelClose` until resolved).
  - File dialogs with `rfd::FileDialog` (filter "PageRank Explorer graph",
    extension `json`); Save without a path behaves as Save As.
  - Every frame, send `ViewportCommand::Title(window_title(…))` when it
    changes.

- [ ] **Step 1: Write the failing tests**

Unit test in `src/ui/dialogs.rs` (or `mod.rs`, wherever `window_title`
lives):

```rust
#[test]
fn test_window_title() {
    assert_eq!(window_title(None, true), "PageRank Explorer");
    assert_eq!(window_title(Some(Path::new("/x/lecture.json")), false), "PageRank Explorer — lecture.json");
    assert_eq!(window_title(Some(Path::new("/x/lecture.json")), true), "PageRank Explorer — lecture.json*");
}
```

Integration test in `tests/test_ui.rs`:

```rust
#[test]
fn test_generate_random() {
    let mut h = harness();
    h.run();
    h.state_mut().generate_random(pagerank_explorer::random::ChungLu::default(), 3);
    h.run();
    let app = h.state();
    let g = app.session().graph();
    assert_eq!(g.len(), 10);
    assert_eq!(g.seed(), Some(3));
    let size = app.canvas_rect().size();
    assert!(g.nodes().iter().all(|n| n.pos[0] <= size.x && n.pos[1] <= size.y));
    assert!(app.session().ghost().is_none());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test`
Expected: compilation errors.

- [ ] **Step 3: Implement `dialogs.rs`, `generate_random`, file commands,
  the close confirmation, and the title update**; replace the Task 9 stubs.

- [ ] **Step 4: Run tests, clippy, and the app**

Run: `cargo test && cargo clippy --all-features --all-targets -- -D warnings && cargo run`
Expected: tests pass, no warnings. Manually: Cmd-R opens the random dialog
and generates a 10-node graph that animates out from the center; Cmd-Z
restores the previous graph; save, quit with no prompt, reopen the file and
get the same graph; edit, then try to quit and get the unsaved-changes
prompt; Cmd-N offers directed/undirected and an undirected graph draws
edges without arrowheads.

- [ ] **Step 5: Commit**

```bash
git add src tests
git commit -m "Add dialogs, file commands, and window title"
```

---

### Task 13: Documentation and final verification

**Files:**
- Modify: `README.md`, `CHANGELOG.md`

- [ ] **Step 1: Write `README.md`**: what the tool is for (from the spec's
  Intent), how to build and run (`cargo run --release`; note that
  `target-cpu=native` means building on the machine that runs it), the
  editing gestures and shortcuts, the meaning of the visual encodings (color
  = identity, size = score at α, dashed = before the last edit, hull =
  terminal component, dashed outline = dangling node), and the mathematics
  in two short paragraphs (exact curves; limit at α = 1 supported on the
  terminal components). Links in reference style.

- [ ] **Step 2: Update `CHANGELOG.md`** with a `[0.1.0] - <date>` entry
  listing the features under "New".

- [ ] **Step 3: Full verification**

Run: `cargo fmt --check && cargo clippy --all-features --all-targets -- -D warnings && cargo test && cargo test --release -- --ignored && cargo doc --no-deps`
Expected: all clean and passing. Check that the binary was built with the
native CPU flag: the `"rustflags"` field in
`target/release/.fingerprint/pagerank-explorer-*/bin-pagerank-explorer.json`
contains `target-cpu=native`.

- [ ] **Step 4: Lecture rehearsal** (`cargo run --release`, at 1280×720 and
  full screen): build a→b→c→a, c→d, d→e; toggle e→d on and off with two
  clicks each time and confirm the curves near α = 1 jump each time with the
  previous curves dashed; Cmd-Z/Cmd-Shift-Z flips before/after; Cmd-+ zooms
  everything; generate a random graph, tidy it with Cmd-1, order it with
  Cmd-3; save and reopen.

- [ ] **Step 5: Commit**

```bash
git add README.md CHANGELOG.md
git commit -m "Document PageRank Explorer"
```
