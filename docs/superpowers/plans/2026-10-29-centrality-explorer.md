# Centrality Explorer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn PageRank Explorer into Centrality Explorer, which shows seven centralities (PageRank, Katz, Seeley, eigenvector, closeness, harmonic, betweenness) with the same live editing, curves, and before/after comparison.

**Architecture:** The pure, GUI-free math lives in `src/rank/`, one file per family: `pagerank.rs` (moved), `spectral.rs` (Katz, spectral radius, dominant classes), `power.rs` (power-method iterates), and `geometric.rs` (closeness, harmonic, betweenness). `rank/mod.rs` defines `Centrality`, `Axis`, and a `Ranking` generalized over all seven. `Session` gains the current centrality, per-centrality slider values, and the graph from before the last edit, so the ghost survives a switch. The UI reads `Ranking::axis` and `Session::param()`, and has no per-centrality logic beyond labels.

**Tech Stack:** Rust 2024, eframe/egui 0.37, egui_plot 0.38, serde, and hand-rolled dense linear algebra (`rank/lu.rs`).

**Spec:** `docs/superpowers/specs/2026-10-29-centrality-explorer-design.md`

## Global Constraints

- Follow the user's Rust guidelines: load the `rust-guidelines` skill before writing code. Match the surrounding style: doc comments on every public item, and math in doc comments written with *italic* names and Unicode, as in `rank/mod.rs`.
- `.cargo/config.toml` with `rustflags = ["-C", "target-cpu=native"]` must stay.
- No new dependencies.
- Every score vector shown (curves and node sizes) is normalized to sum to 1. An identically zero vector stays zero.
- Constants: `SAMPLES = 256` (unchanged); `ITERATIONS = 50` (*K*); default Katz parameter βρ = 0.85; default α = 0.85; default centrality PageRank.
- Basic-class tolerance: |ρ_C − ρ| ≤ 1e-9 · ρ.
- Work on a branch, not on `main`. End every commit message with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- `cargo fmt --check` and `cargo clippy --all-targets` stay clean after every task.

## Review Focus

1. **Empty graph, a single node, a single node with a self-loop, and a graph with no arcs, under every centrality.** These must not panic, all values must be finite, and no-arc graphs give zero vectors where the math says so. Owner: Task 5.
2. **Eigenvector iterates that vanish at k = 1 (no arcs).** The ranking then has one sample, and hovering the plot or moving the slider past the vanishing point must not panic (`plot::value_at` clamps with `clamp(1, len − 1)`, which panics for length 1). Owner: Task 7.
3. **A 100-node directed cycle under Katz and eigenvector centrality.** Power iteration for ρ converges slowly there, so the ranking must still take under 50 ms in release mode. Owner: Task 5 (speed test).
4. **Ghost after deleting a node, then switching centrality.** The ghost is recomputed from the stored before-graph and must still contain the deleted node's curve. Owner: Task 6.
5. **Files saved by 0.1.0 (no `centrality` or `katz` fields).** They must load as PageRank with Katz at 0.85. Owner: Task 6.

---

### Task 1: Rename to Centrality Explorer

**Files:**
- Modify: `Cargo.toml` (name `centrality-explorer`; description "A teaching aid to build intuition about centrality measures"), `src/main.rs`, `tests/test_ui.rs` (crate paths), `src/ui/mod.rs:26` (`NAME`), `src/ui/dialogs.rs:13` (`FILTER` = `"Centrality Explorer graph"`) and its `test_window_title`, `README.md` (title only; content in Task 8)

**Interfaces:**
- Produces: crate path `centrality_explorer::…`, used by all later tasks' integration tests.

- [ ] **Step 1: Update `test_window_title` to expect the new name**

```rust
assert_eq!(window_title(None, true), "Centrality Explorer");
assert_eq!(window_title(Some(Path::new("/x/lecture.json")), false), "Centrality Explorer — lecture.json");
assert_eq!(window_title(Some(Path::new("/x/lecture.json")), true), "Centrality Explorer — lecture.json*");
```

- [ ] **Step 2: Run `cargo test test_window_title`.** Expected: FAIL.
- [ ] **Step 3: Rename everything listed under Files.** `README.md` gets the title `# Centrality Explorer`.
- [ ] **Step 4: Run `cargo test`.** Expected: all pass, with no remaining `pagerank_explorer` or `"PageRank Explorer"` (check with `grep -rn "pagerank_explorer\|PageRank Explorer" src tests Cargo.toml`).
- [ ] **Step 5: Commit** ("Rename to Centrality Explorer").

---

### Task 2: Closeness, harmonic, and betweenness

**Files:**
- Create: `src/rank/geometric.rs`
- Modify: `src/rank/mod.rs` (add `mod geometric; pub use geometric::{betweenness, closeness, harmonic};`)

**Interfaces:**
- Produces, all taking successor lists by node index (as `Graph::successors()` returns them) and returning raw, unnormalized values:
  - `pub fn closeness(succ: &[Vec<usize>]) -> Vec<f64>`: 1 / Σ_{y : 0 < d(y,x) < ∞} *d*(*y*, *x*), and 0 if no other node reaches *x*.
  - `pub fn harmonic(succ: &[Vec<usize>]) -> Vec<f64>`: Σ_{y ≠ x} 1/*d*(*y*, *x*).
  - `pub fn betweenness(succ: &[Vec<usize>]) -> Vec<f64>`: Σ σ_yz(*x*)/σ_yz over ordered pairs of distinct *y*, *z* ≠ *x* (Brandes).

- [ ] **Step 1: Write failing tests** in `geometric.rs` (use `crate::rank::tests::assert_close`, tolerance 1e-12):

```rust
#[test]
fn test_directed_path() { // a → b → c
    let succ = vec![vec![1], vec![2], vec![]];
    assert_close(&closeness(&succ), &[0.0, 1.0, 1.0 / 3.0], 1e-12);
    assert_close(&harmonic(&succ), &[0.0, 1.0, 1.5], 1e-12);
    assert_close(&betweenness(&succ), &[0.0, 1.0, 0.0], 1e-12);
}
#[test]
fn test_directed_cycle() { // 4-cycle
    let succ = vec![vec![1], vec![2], vec![3], vec![0]];
    assert_close(&closeness(&succ), &[1.0 / 6.0; 4], 1e-12);
    assert_close(&harmonic(&succ), &[11.0 / 6.0; 4], 1e-12);
    assert_close(&betweenness(&succ), &[3.0; 4], 1e-12);
}
#[test]
fn test_undirected_star() { // center 0, leaves 1, 2, 3
    let succ = vec![vec![1, 2, 3], vec![0], vec![0], vec![0]];
    assert_close(&closeness(&succ), &[1.0 / 3.0, 0.2, 0.2, 0.2], 1e-12);
    assert_close(&harmonic(&succ), &[3.0, 2.0, 2.0, 2.0], 1e-12);
    assert_close(&betweenness(&succ), &[6.0, 0.0, 0.0, 0.0], 1e-12);
}
#[test]
fn test_not_strongly_connected() { // a → b, c → b, b → d
    let succ = vec![vec![1], vec![3], vec![1], vec![]];
    assert_close(&closeness(&succ), &[0.0, 0.5, 0.0, 0.2], 1e-12);
    assert_close(&harmonic(&succ), &[0.0, 2.0, 0.0, 2.0], 1e-12);
    assert_close(&betweenness(&succ), &[0.0, 2.0, 0.0, 0.0], 1e-12);
}
#[test]
fn test_betweenness_splits_shortest_paths() { // diamond a → b, a → c, b → d, c → d
    let succ = vec![vec![1, 2], vec![3], vec![3], vec![]];
    assert_close(&betweenness(&succ), &[0.0, 0.5, 0.5, 0.0], 1e-12);
}
#[test]
fn test_self_loops_ignored() {
    assert_close(&harmonic(&[vec![0, 1], vec![1]]), &harmonic(&[vec![1], vec![]]), 1e-12);
    assert!(closeness(&[]).is_empty());
}
```

- [ ] **Step 2: Run `cargo test geometric`.** Expected: FAIL (functions missing).
- [ ] **Step 3: Implement the three functions.** Each runs one BFS per source *y* over `succ`, adding into the target *x*. Betweenness is Brandes's algorithm (BFS, then dependency accumulation in reverse BFS order).
- [ ] **Step 4: Run `cargo test geometric`.** Expected: PASS.
- [ ] **Step 5: Commit** ("Add closeness, harmonic, and betweenness centrality").

---

### Task 3: Katz's index and the spectral structure

**Files:**
- Create: `src/rank/spectral.rs`
- Modify: `src/rank/mod.rs` (add `mod spectral; pub use spectral::{Spectrum, katz};`)

**Interfaces:**
- Consumes: `rank::sccs` (reverse topological order: components with no outgoing arcs first), `lu::solve`.
- Produces:
  - `pub fn katz(succ: &[Vec<usize>], beta: f64) -> Vec<f64>`: κ(β), meaning **1**(*I* − β*A*)⁻¹ normalized to sum to 1. It solves (*I* − β*A*ᵀ) *k*ᵀ = **1** densely. Requires β < 1/ρ, or any β when ρ = 0.
  - `pub struct Spectrum`, with `pub fn new(succ: &[Vec<usize>]) -> Self`, `pub fn rho(&self) -> f64`, `pub fn dominant_classes(&self) -> Vec<Vec<usize>>`, and `pub fn katz_limit(&self) -> Vec<f64>`.
  - `dominant_classes` returns the basic classes (ρ_C = ρ within the Global Constraints tolerance) when ρ > 0, and is empty otherwise. Each class is sorted, and classes are sorted by smallest node.
  - `katz_limit` returns the normalized limit of κ as β → 1/ρ, computed exactly by the recursion in the spec ("The limit at βρ = 1"). It must only be called when ρ > 0 (`debug_assert!`).

- [ ] **Step 1: Write failing tests** in `spectral.rs` (helper: build `succ` from an arc list):

```rust
#[test]
fn test_rho() {
    assert_eq!(Spectrum::new(&[vec![1], vec![2], vec![]]).rho(), 0.0); // DAG
    assert!((Spectrum::new(&[vec![1], vec![2], vec![0]]).rho() - 1.0).abs() < 1e-12);
    // a ⇄ b plus b → b: ρ is the golden ratio
    assert!((Spectrum::new(&[vec![1], vec![0, 1]]).rho() - 1.618033988749895).abs() < 1e-12);
    assert!((Spectrum::new(&[vec![0]]).rho() - 1.0).abs() < 1e-12);
}
#[test]
fn test_katz_zero_is_uniform() { assert_close(&katz(&[vec![1], vec![0, 1]], 0.0), &[0.5, 0.5], 1e-15); }
#[test]
fn test_katz_dag_beta_one() { // a → b → c, β = 1: (1, 2, 3) / 6
    assert_close(&katz(&[vec![1], vec![2], vec![]], 1.0), &[1.0 / 6.0, 1.0 / 3.0, 0.5], 1e-12);
}
#[test]
fn test_katz_limit_values() {
    // a ⇄ b → c ⇄ d (ν = 2): zero upstream
    assert_close(&Spectrum::new(&[vec![1], vec![0, 2], vec![3], vec![2]]).katz_limit(), &[0.0, 0.0, 0.5, 0.5], 1e-12);
    // two 2-cycles in parallel, a source into both, both into a dangling node
    let par = succ_of(6, &[(0,1),(1,0),(2,3),(3,2),(4,0),(4,2),(1,5),(3,5)]);
    assert_close(&Spectrum::new(&par).katz_limit(), &[1.0/6.0, 1.0/6.0, 1.0/6.0, 1.0/6.0, 0.0, 1.0/3.0], 1e-12);
    // periodic a ⇄ b, c → a
    assert_close(&Spectrum::new(&[vec![1], vec![0], vec![0]]).katz_limit(), &[0.5, 0.5, 0.0], 1e-12);
    // chain of three self-loops into a plain node (ν = 3)
    let chain = succ_of(4, &[(0,0),(0,1),(1,1),(1,2),(2,2),(2,3)]);
    assert_close(&Spectrum::new(&chain).katz_limit(), &[0.0, 0.0, 0.5, 0.5], 1e-12);
}
#[test]
fn test_katz_limit_matches_near_limit() {
    // the four graphs above, plus a triangle with a self-loop on a, feeding a 2-cycle (non-basic downstream)
    for succ in [/* … */] {
        let s = Spectrum::new(&succ);
        assert_close(&s.katz_limit(), &katz(&succ, (1.0 - 1e-6) / s.rho()), 1e-5);
    }
}
#[test]
fn test_dominant_classes() {
    // a 3-cycle and a 5-cycle (both ρ = 1) are both dominant; a lone node is not
    let succ = succ_of(9, &[(0,1),(1,2),(2,0),(3,4),(4,5),(5,6),(6,7),(7,3),(8,0)]);
    assert_eq!(Spectrum::new(&succ).dominant_classes(), vec![vec![0,1,2], vec![3,4,5,6,7]]);
    assert!(Spectrum::new(&[vec![1], vec![]]).dominant_classes().is_empty());
    // triangle with self-loop beats a 2-cycle downstream
    let succ = succ_of(5, &[(0,0),(0,1),(1,2),(2,0),(2,3),(3,4),(4,3)]);
    assert_eq!(Spectrum::new(&succ).dominant_classes(), vec![vec![0,1,2]]);
}
#[test]
fn test_katz_sums_to_one() {
    // the golden-ratio graph, the parallel graph, and the triangle-with-self-loop graph
    // at β = 0.5/ρ and 0.99/ρ: |Σ − 1| < 1e-12
}
```

The expected limits were checked against brute-force solves at βρ = 1 − ε during design. The error is O(ε), and about 1.25 ε for ν = 2.

- [ ] **Step 2: Run `cargo test spectral`.** Expected: FAIL.
- [ ] **Step 3: Implement `katz`, `Spectrum::new`, and the getters.** `Spectrum::new` stores `succ`, the classes upstream-first (reverse of `sccs`), each class's spectral radius, and, for basic classes, left and right Perron vectors. Per class, with *m* nodes and local indices:
  - *m* = 1: the radius is 1 if the node has a self-loop and 0 otherwise; the Perron vectors are `[1.0]`.
  - *m* > 1: run power iteration on the primitive matrix *M* = *A*_CC + *I*, starting from uniform and normalizing with ℓ1 at each step: `x ← x M` for the left vector and `y ← M y` for the right one. Stop when the Collatz–Wielandt bounds lo = minᵢ (*xM*)ᵢ/*x*ᵢ and hi = maxᵢ (*xM*)ᵢ/*x*ᵢ satisfy hi − lo ≤ 1e-13 · hi, or after 1 000 000 iterations. The radius is (lo + hi)/2 − 1.
  - ρ is the maximum class radius.
- [ ] **Step 4: Implement `dominant_classes` and `katz_limit`.** `katz_limit` follows the spec's recursion on *u*_H(C). The leading coefficients are computed in upstream-first order. For non-basic classes, `lu::solve` on (*I* − *A*_CCᵀ/ρ). The result keeps the classes with *h*(C) = ν, normalized.
- [ ] **Step 5: Run `cargo test spectral`.** Expected: PASS.
- [ ] **Step 6: Commit** ("Add Katz's index with its exact limit").

---

### Task 4: Power-method iterates

**Files:**
- Create: `src/rank/power.rs`
- Modify: `src/rank/mod.rs` (add `mod power; pub use power::{eigenvector_iterates, seeley_iterates};`)

**Interfaces:**
- Consumes: `Chain` and `Chain::transition()` (currently in `rank/mod.rs`; Task 5 moves them to `rank/pagerank.rs` and re-exports them, so `super::Chain` keeps working).
- Produces:
  - `pub fn seeley_iterates(chain: &Chain, iterations: usize) -> Vec<Vec<f64>>`: *x*₀ uniform, *x*_{k+1} = *x*_k *P*. Returns `iterations + 1` vectors, and a single empty vector when *n* = 0.
  - `pub fn eigenvector_iterates(succ: &[Vec<usize>], iterations: usize) -> Vec<Vec<f64>>`: *x*₀ uniform, *x*_{k+1} = *x*_k *A* / ‖*x*_k *A*‖₁. It stops before the first iterate with *x*_k *A* = 0 (sum exactly 0.0), so a result shorter than `iterations + 1` means the iterates vanished at *k* = `len()`. Returns a single empty vector when *n* = 0.

- [ ] **Step 1: Write failing tests** in `power.rs`:

```rust
#[test]
fn test_seeley_oscillates() { // a ⇄ b, c → a
    let it = seeley_iterates(&Chain::new(vec![vec![1], vec![0], vec![0]]), 4);
    assert_eq!(it.len(), 5);
    assert_close(&it[1], &[2.0 / 3.0, 1.0 / 3.0, 0.0], 1e-15);
    assert_close(&it[2], &[1.0 / 3.0, 2.0 / 3.0, 0.0], 1e-15);
    assert_close(&it[3], &it[1], 1e-15);
}
#[test]
fn test_seeley_converges_to_pagerank_limit() {
    // aperiodic: example_after with a self-loop on e, and the path with a dangling end
    for chain in [Chain::new(vec![vec![1], vec![2], vec![0, 3], vec![4], vec![3, 4]]),
                  Chain::new(vec![vec![1], vec![2], vec![]])] {
        let it = seeley_iterates(&chain, 2000);
        assert_close(it.last().unwrap(), &limit(&chain), 1e-9);
        assert!(it.iter().all(|x| (x.iter().sum::<f64>() - 1.0).abs() < 1e-12));
    }
}
#[test]
fn test_eigenvector_vanishes_on_dag() { // a → b → c
    let it = eigenvector_iterates(&[vec![1], vec![2], vec![]], 50);
    assert_eq!(it.len(), 3); // vanishes at k = 3
    assert_close(&it[1], &[0.0, 0.5, 0.5], 1e-15);
    assert_close(&it[2], &[0.0, 0.0, 1.0], 1e-15);
    assert_eq!(eigenvector_iterates(&[vec![], vec![]], 50).len(), 1); // no arcs
}
#[test]
fn test_eigenvector_star_oscillates() { // undirected star, center 0
    let it = eigenvector_iterates(&[vec![1, 2, 3], vec![0], vec![0], vec![0]], 4);
    assert_close(&it[1], &[0.5, 1.0 / 6.0, 1.0 / 6.0, 1.0 / 6.0], 1e-15);
    assert_close(&it[2], &[0.25; 4], 1e-15);
}
#[test]
fn test_eigenvector_slow_convergence() { // a (loop) → b (loop): 1·Aᵏ = (1, k + 1)
    let it = eigenvector_iterates(&[vec![0, 1], vec![1]], 50);
    assert!((it[50][0] - 1.0 / 52.0).abs() < 1e-15);
}
#[test]
fn test_eigenvector_converges_to_katz_limit() {
    // ν = 1, aperiodic: triangle with a self-loop feeding a 2-cycle; a ⇄ b with b → b
    for succ in [/* … */] {
        let it = eigenvector_iterates(&succ, 2000);
        assert_close(it.last().unwrap(), &Spectrum::new(&succ).katz_limit(), 1e-9);
    }
}
```

- [ ] **Step 2: Run `cargo test power`.** Expected: FAIL.
- [ ] **Step 3: Implement both functions** with dense or sparse products. Use `chain.transition()` for *P*, and `succ` directly for *A*.
- [ ] **Step 4: Run `cargo test power`.** Expected: PASS.
- [ ] **Step 5: Commit** ("Add power-method iterates for Seeley's index and eigenvector centrality").

---

### Task 5: `Centrality`, `Axis`, and the general `Ranking`

**Files:**
- Create: `src/rank/pagerank.rs`, which receives `Chain`, `pagerank`, `limit`, `terminal_components`, and their tests from `rank/mod.rs`, unchanged. `rank/mod.rs` re-exports them, so `crate::rank::{Chain, pagerank, limit, terminal_components}` stay valid.
- Modify: `src/rank/mod.rs`, plus the field renames in `src/ui/canvas.rs:279` and `src/ui/plot.rs` (`alphas` → `xs`, `sinks` → `hulls`), and `src/session/mod.rs` (`Ranking::new(&g, Centrality::PageRank)` until Task 6).

**Interfaces:**
- Consumes: Tasks 2–4.
- Produces, in `rank/mod.rs`:

```rust
pub const ITERATIONS: usize = 50;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Centrality { #[default] PageRank, Katz, Seeley, Eigenvector, Closeness, Harmonic, Betweenness }
impl Centrality {
    /// Menu order, by family.
    pub const ALL: [Self; 7] = [PageRank, Katz, Seeley, Eigenvector, Closeness, Harmonic, Betweenness];
    /// "PageRank", "Katz's Index", "Seeley's Index", "Eigenvector Centrality",
    /// "Closeness Centrality", "Harmonic Centrality", "Betweenness Centrality".
    pub fn name(self) -> &'static str;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis { Alpha, BetaRho, Beta, Iteration, Flat }
impl Axis {
    /// "α", "βρ", "β", "k", "".
    pub fn label(self) -> &'static str;
    /// ITERATIONS as f64 for Iteration, 1 otherwise.
    pub fn x_max(self) -> f64;
}

pub struct Ranking {
    pub centrality: Centrality,
    pub axis: Axis,
    pub ids: Vec<NodeId>,
    pub xs: Vec<f64>,                 // was `alphas`
    pub curves: Vec<Vec<f64>>,        // curves[i][k] at xs[k]
    pub hulls: Vec<Vec<NodeId>>,      // was `sinks`
    pub hull_label: &'static str,     // "sinks", "dominant", or ""
    pub dangling: Vec<NodeId>,        // empty except PageRank and Seeley
    pub rho: Option<f64>,             // Some for Katz and Eigenvector
    pub vanished: Option<usize>,      // Eigenvector: first k with 1·Aᵏ = 0, if ≤ ITERATIONS
    /* private state for `scores` */
}
impl Ranking {
    pub fn new(graph: &Graph, centrality: Centrality) -> Self;
    pub fn scores(&self, x: f64) -> Vec<f64>;
    pub fn curve(&self, id: NodeId) -> Option<&[f64]>; // unchanged
    pub fn max_value(&self) -> f64;                   // unchanged
}
```

Per centrality, `Ranking::new` builds:

| Centrality | axis | xs | curves | hulls / label | dangling |
|---|---|---|---|---|---|
| PageRank | Alpha | k/SAMPLES, then 1 (as today) | `pagerank` | sinks (today's rule) / "sinks" | yes |
| Katz, ρ > 0 | BetaRho | same grid | `katz(succ, t/ρ)` for t < 1; `katz_limit` at 1 | `dominant_classes` / "dominant" | no |
| Katz, ρ = 0 | Beta | same grid | `katz(succ, t)`, including t = 1 | none / "" | no |
| Seeley | Iteration | 0, 1, …, ITERATIONS | `seeley_iterates` | sinks / "sinks" | yes |
| Eigenvector | Iteration | 0, …, len − 1 | `eigenvector_iterates`; `vanished = Some(len)` if len ≤ ITERATIONS | `dominant_classes` / "dominant" | no |
| Closeness, Harmonic, Betweenness | Flat | [0, 1] | the normalized vector, twice | none / "" | no |

`scores(x)`:
- Alpha: `pagerank(chain, x.clamp(0, 1))`.
- BetaRho: the stored limit if x ≥ 1, else `katz(succ, x/ρ)`.
- Beta: `katz(succ, x.clamp(0, 1))`.
- Iteration: column `x.round()` if it is less than `xs.len()`, else all zeros.
- Flat: column 0.

All vectors pass through a private `fn normalized(v: Vec<f64>) -> Vec<f64>` (divide by the sum if positive, else unchanged).

- [ ] **Step 1: Move the PageRank code to `pagerank.rs`, with re-exports.** Rename the fields and update callers. Run `cargo test`. Expected: PASS once the existing tests are updated for the renames (`r.sinks` → `r.hulls`, `r.alphas` → `r.xs`, `Ranking::new(&g)` → `Ranking::new(&g, Centrality::PageRank)`).
- [ ] **Step 2: Write failing tests** in `rank/mod.rs`:

```rust
#[test]
fn test_every_centrality_on_small_graphs() { // Review Focus 1
    // empty; one node; one node with a self-loop; three nodes, no arcs; the 5-node lecture example
    for g in small_graphs() {
        for c in Centrality::ALL {
            let r = Ranking::new(&g, c);
            assert_eq!(r.centrality, c);
            assert!(r.curves.iter().flatten().all(|x| x.is_finite()));
            for curve in &r.curves { assert_eq!(curve.len(), r.xs.len()); }
            for k in 0..r.xs.len() {
                let sum: f64 = r.curves.iter().map(|c| c[k]).sum();
                assert!(sum.abs() < 1e-12 || (sum - 1.0).abs() < 1e-12, "{c:?} {sum}");
            }
            let max = r.axis.x_max();
            for x in [0.0, 0.5 * max, max] {
                assert!(r.scores(x).iter().all(|x| x.is_finite()));
            }
        }
    }
}
#[test]
fn test_ranking_axes() {
    let lecture = lecture_graph(); // a→b→c→a, c→d, d⇄e
    assert_eq!(Ranking::new(&lecture, Centrality::PageRank).axis, Axis::Alpha);
    let katz = Ranking::new(&lecture, Centrality::Katz);
    assert_eq!(katz.axis, Axis::BetaRho);
    assert_eq!(katz.hulls.len(), 2); // a b c and d e both have ρ = 1
    assert_eq!(katz.hull_label, "dominant");
    assert_eq!(Ranking::new(&path_graph(), Centrality::Katz).axis, Axis::Beta);
    let seeley = Ranking::new(&lecture, Centrality::Seeley);
    assert_eq!((seeley.axis, seeley.xs.len()), (Axis::Iteration, ITERATIONS + 1));
    let eig = Ranking::new(&path_graph(), Centrality::Eigenvector); // a → b → c
    assert_eq!((eig.xs.len(), eig.vanished), (3, Some(3)));
    assert!(eig.scores(10.0).iter().all(|&x| x == 0.0));
    let flat = Ranking::new(&lecture, Centrality::Harmonic);
    assert_eq!((flat.axis, flat.xs.clone()), (Axis::Flat, vec![0.0, 1.0]));
    assert!(flat.hulls.is_empty() && flat.dangling.is_empty());
}
#[test]
fn test_katz_ranking_endpoints() {
    let r = Ranking::new(&lecture_graph(), Centrality::Katz);
    assert_close(&r.scores(0.0), &[0.2; 5], 1e-15);
    let s = Spectrum::new(&lecture_graph().successors());
    assert_close(&r.scores(1.0), &s.katz_limit(), 1e-15);
    assert_close(&r.scores(0.85), &katz(&lecture_graph().successors(), 0.85 / s.rho()), 1e-12);
}
#[test]
fn test_flat_rankings_are_normalized() {
    let r = Ranking::new(&star_graph(), Centrality::Betweenness); // undirected star, 3 leaves
    assert_close(&r.scores(0.0), &[1.0, 0.0, 0.0, 0.0], 1e-12);
}
/// Run with `cargo test --release -- --ignored`.
#[test]
#[ignore]
fn test_ranking_speed_100_all() { // Review Focus 3
    // the existing pseudo-random 100-node graph, and a directed 100-cycle with one chord
    for g in [/* … */] {
        for c in Centrality::ALL {
            let start = std::time::Instant::now();
            let _ = Ranking::new(&g, c);
            assert!(start.elapsed().as_millis() < 50, "{c:?} {:?}", start.elapsed());
        }
    }
}
```

- [ ] **Step 3: Run `cargo test rank`.** Expected: the new tests FAIL.
- [ ] **Step 4: Implement `Centrality`, `Axis`, and the general `Ranking`.**
- [ ] **Step 5: Run `cargo test` and `cargo test --release -- --ignored test_ranking_speed`.** Expected: PASS.
- [ ] **Step 6: Commit** ("Generalize rankings to seven centralities").

---

### Task 6: Session and files

**Files:**
- Modify: `src/files/mod.rs`, `src/session/mod.rs`

**Interfaces:**
- Consumes: `Centrality`, `Axis`, `Ranking`, `ITERATIONS`.
- Produces:
  - In `files`: `#[derive(Clone, Copy, Debug, PartialEq)] pub struct Settings { pub centrality: Centrality, pub alpha: f64, pub katz: f64 }`, where `Default` is (PageRank, 0.85, 0.85). The following functions take or return it instead of `alpha: f64`:
    - `pub fn to_json(graph: &Graph, settings: &Settings) -> String`
    - `pub fn from_json(json: impl AsRef<str>) -> anyhow::Result<(Graph, Settings)>`
    - `pub fn save(graph: &Graph, settings: &Settings, path: impl AsRef<Path>) -> anyhow::Result<()>`
    - `pub fn load(path: impl AsRef<Path>) -> anyhow::Result<(Graph, Settings)>`
  - `GraphFile` gains `#[serde(default)] centrality: Centrality` and `#[serde(default = "default_katz")] katz: f64`. The format stays version 1. `alpha` and `katz` are clamped to [0, 1] on load. Update the module doc example.
  - In `Session`:
    - `pub fn with_graph(graph: Graph, settings: Settings, path: impl Into<Option<PathBuf>>) -> Self` (replaces the `alpha` argument).
    - `pub fn settings(&self) -> Settings`.
    - `pub fn centrality(&self) -> Centrality` and `pub fn set_centrality(&mut self, c: Centrality)`. A switch recomputes `ranking`, recomputes `ghost` from the before-graph, and updates `scores`. It is neither undoable nor dirty.
    - `pub fn katz(&self) -> f64` and `pub fn iteration(&self) -> usize`. `alpha()` and `set_alpha()` stay.
    - `pub fn param(&self) -> Option<f64>`: α for PageRank, βρ (or β) for Katz, *k* for Seeley and Eigenvector, `None` for the rest.
    - `pub fn set_param(&mut self, x: f64)`: sets the current centrality's value. α and Katz are clamped to [0, 1]; *k* is `x.round()` clamped to 0..=ITERATIONS. It does nothing for Flat. It updates `scores` when the value changes.
    - `pub fn step_param(&mut self, steps: i32)`: continuous values move to `((v·100).round() + steps)/100`; *k* moves to `k + steps`, clamped.
    - Remove `DEFAULT_ALPHA`. `iteration` starts at ITERATIONS and is not saved.
  - The session stores `before: Option<Graph>`. `install` sets it to the old graph on a structural change, clears it on a new generation, and otherwise leaves it alone, so the existing ghost tests keep their meaning.

- [ ] **Step 1: Write failing tests.**

In `files`:
```rust
#[test]
fn test_roundtrip_settings() -> anyhow::Result<()> {
    let s = Settings { centrality: Centrality::Katz, alpha: 0.7, katz: 0.3 };
    let (_, t) = from_json(to_json(&Graph::new(true), &s))?;
    assert_eq!(t, s);
    Ok(())
}
#[test]
fn test_load_0_1_0_file() -> anyhow::Result<()> { // Review Focus 5
    let (_, s) = from_json(r#"{"version":1,"directed":true,"alpha":0.6,"seed":null,"nodes":[],"arcs":[]}"#)?;
    assert_eq!(s, Settings { centrality: Centrality::PageRank, alpha: 0.6, katz: 0.85 });
    Ok(())
}
```

In `session`:
```rust
#[test]
fn test_ghost_survives_switch() {
    let (mut s, a, b) = two_nodes();
    s.commit(with_arc(&s, a, b));
    s.set_centrality(Centrality::Katz);
    let expected = Ranking::new(&{ let mut g = s.graph().clone(); g.toggle_arc(a, b); g }, Centrality::Katz);
    assert_eq!(s.ghost().unwrap().curve(a).unwrap(), expected.curve(a).unwrap());
    assert_eq!(s.ranking().centrality, Centrality::Katz);
    assert!(!s.can_redo());
}
#[test]
fn test_ghost_with_deleted_node_survives_switch() { // Review Focus 4
    let (mut s, a, _) = two_nodes();
    let mut g = s.graph().clone();
    g.remove_node(a);
    s.commit(g);
    s.set_centrality(Centrality::Harmonic);
    assert!(s.ranking().curve(a).is_none());
    assert!(s.ghost().unwrap().curve(a).is_some());
}
#[test]
fn test_no_ghost_after_switch_on_new_graph() {
    let mut s = Session::new(true);
    s.set_centrality(Centrality::Seeley);
    assert!(s.ghost().is_none());
}
#[test]
fn test_params_per_centrality() {
    let (mut s, _, _) = two_nodes();
    s.mark_saved("x.json".into());
    s.set_param(0.5);
    assert_eq!(s.alpha(), 0.5);
    s.set_centrality(Centrality::Katz);
    assert_eq!(s.param(), Some(0.85));
    s.step_param(-5);
    assert!((s.katz() - 0.8).abs() < 1e-12);
    s.set_centrality(Centrality::Eigenvector);
    assert_eq!(s.param(), Some(ITERATIONS as f64));
    s.step_param(-1);
    assert_eq!(s.iteration(), ITERATIONS - 1);
    s.set_param(1e9);
    assert_eq!(s.iteration(), ITERATIONS);
    s.set_centrality(Centrality::Closeness);
    assert_eq!(s.param(), None);
    s.step_param(3); // no effect
    s.set_centrality(Centrality::PageRank);
    assert_eq!(s.alpha(), 0.5);
    assert!(!s.is_dirty()); // neither switching nor parameters dirty the session
}
#[test]
fn test_save_open_keeps_settings() -> anyhow::Result<()> { /* Katz, katz 0.4 → save_as → open → same Settings; iteration back to ITERATIONS */ }
```
Update existing callers of `with_graph`, `files::save`, `files::load`, and `from_json`/`to_json` in tests.

- [ ] **Step 2: Run `cargo test session files`.** Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run `cargo test`.** Expected: PASS.
- [ ] **Step 5: Commit** ("Add the current centrality and its parameters to sessions and files").

---

### Task 7: User interface

**Files:**
- Modify: `src/ui/mod.rs`, `src/ui/plot.rs`, `src/ui/canvas.rs`, `tests/test_ui.rs`

**Interfaces:**
- Consumes: `Session::{centrality, set_centrality, param, set_param, step_param}`, `Ranking::{axis, xs, hulls, hull_label, dangling, rho, vanished}`, `Axis::{label, x_max}`, `Centrality::{ALL, name}`.
- Produces: `Command::Centrality(Centrality)`.

Decisions:
- **Shortcuts:** Cmd-Alt-1 … Cmd-Alt-7 select `Centrality::ALL[0..7]`. Shortcuts match modifiers logically, so they must come **before** Cmd-1…3 in `SHORTCUTS`, the same way shifted variants come first today.
- **Menu:** a **Centrality** menu between Graph and View, with one selectable item per centrality (checked when current, showing its shortcut). Separators go after Katz and after Eigenvector. The empty-canvas context menu gets a "Centrality" submenu with the same items.
- **Right panel, top to bottom:**
  - A heading with `centrality.name()` at Heading size or larger.
  - The plot.
  - The slider panel, shown only when `param()` is `Some`. Its label is `"{axis.label()} = {:.2}"`, or `"k = {k}"` for Iteration. For Katz, `"ρ = {:.3}"` follows the label. The slider is continuous on [0, 1] or an integer on 0..=ITERATIONS.
- **Plot:**
  - x bounds are `0..=axis.x_max() * 1.12`, and end labels sit at `axis.x_max() * 1.015`.
  - x grid and ticks: [0.5, 0.1, 0.05] with one-decimal labels for continuous axes; [10, 5, 1] with integer labels at multiples of 10 for Iteration; none for Flat.
  - `x_axis_label(axis.label())` and `y_axis_label(centrality.name())`.
  - The vertical line and the dots only when `param()` is `Some`. Click and drag call `set_param`. Hover and click clamp the pointer's x to `[0, axis.x_max()]`, not today's `[0, 1]`.
  - When `vanished` is `Some(v)`, the text "1·Aᵏ = 0 for k ≥ {v}" sits at the bottom right inside the plot.
  - `value_at` must handle a single sample (return `curve[0]`). This is Review Focus 2.
- **Canvas:** shade `ranking().hulls`, labeled `hull_label`. The dangling markers already come from the ranking.
- **Arrow keys:** `step_param(±1)`, or `step_param(±5)` with Shift.

- [ ] **Step 1: Write failing tests** in `tests/test_ui.rs` and `plot.rs`:

```rust
#[test]
fn test_centrality_shortcuts() {
    let (mut h, _, _) = two_node_app();
    h.key_press_modifiers(egui::Modifiers::COMMAND | egui::Modifiers::ALT, egui::Key::Num2);
    h.run();
    assert_eq!(h.state().session().centrality(), Centrality::Katz);
    let before = h.state().session().graph().clone();
    h.key_press_modifiers(egui::Modifiers::COMMAND | egui::Modifiers::ALT, egui::Key::Num1);
    h.run();
    assert_eq!(h.state().session().centrality(), Centrality::PageRank);
    assert_eq!(h.state().session().graph(), &before); // no force layout
}
#[test]
fn test_iteration_arrows() {
    let mut h = harness();
    h.run();
    h.state_mut().session_mut().set_centrality(Centrality::Seeley);
    h.key_press(egui::Key::ArrowLeft);
    h.run();
    assert_eq!(h.state().session().iteration(), ITERATIONS - 1);
}
#[test]
fn test_vanished_eigenvector_hover() { // Review Focus 2
    let (mut h, _, _) = two_node_app(); // no arcs: vanishes at k = 1
    h.state_mut().session_mut().set_centrality(Centrality::Eigenvector);
    h.run();
    h.hover_at(egui::pos2(900.0, 300.0));
    h.run();
    h.key_press(egui::Key::ArrowLeft);
    h.run();
}
#[test]
fn test_every_centrality_renders() {
    let mut h = harness();
    h.run();
    // commit the five-node lecture example as `lecture_harness` does (no wgpu needed)
    for c in Centrality::ALL {
        h.state_mut().session_mut().set_centrality(c);
        h.run();
    }
}
```
In `plot.rs`: `test_value_at_single_sample` asserts `value_at(&[0.0], &[0.3], 5.0) == 0.3`.

Add an ignored, GPU-only `test_render_centralities`. It renders the lecture example after the edit that closes {d, e}, under each centrality, to `target/screenshots/<lowercase name>.png`, following the existing render tests.

- [ ] **Step 2: Run `cargo test`.** Expected: the new tests FAIL.
- [ ] **Step 3: Implement the decisions above.**
- [ ] **Step 4: Run `cargo test`.** Expected: PASS.
- [ ] **Step 5: Run `cargo test --test test_ui -- --ignored test_render_centralities` and inspect every PNG.** Check four things: the centrality name is readable; the Seeley/eigenvector curves zigzag on the lecture example (the d ⇄ e sink has period 2); the Katz hulls are labeled "dominant"; and the flat centralities show horizontal lines with no slider. Fix anything that is off.
- [ ] **Step 6: Commit** ("Add centrality selection, per-centrality axes, and sliders").

---

### Task 8: Documentation and release

**Files:**
- Modify: `README.md`, `CHANGELOG.md`, `Cargo.toml` (version `0.2.0`)

- [ ] **Step 1: Rewrite the README.**
  - **Intro:** a teaching aid about centrality, with the three families.
  - **Reading the Screen:** per family, what the plot, slider, hulls, and dashed outlines mean, and that the before-the-edit curves survive a switch.
  - **Shortcuts:** add Cmd-Alt-1…7.
  - **The Mathematics:** one subsection per centrality, from the spec, including incoming orientation, normalization, the Katz axis (βρ, or β when ρ = 0), the exact limit and its support, power-method convergence and oscillation, and the *Axioms for Centrality* definitions.
  - Keep the existing PageRank text.
- [ ] **Step 2: Add a `## [0.2.0] - <date>` CHANGELOG entry** listing the rename, the six new centralities, the power-method plots, ghost curves that survive a switch, and the new file fields.
- [ ] **Step 3: Run `cargo test`, `cargo test --release -- --ignored`, `cargo clippy --all-targets`, `cargo fmt --check`, and `cargo doc --no-deps`.** Expected: all clean. `lib.rs` includes the README as crate docs, so broken intra-doc links show up here.
- [ ] **Step 4: Commit** ("Document Centrality Explorer 0.2.0").
