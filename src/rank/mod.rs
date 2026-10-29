//! Exact centralities of small graphs.

mod geometric;
mod lu;
mod pagerank;
mod power;
mod scc;
mod spectral;

pub use geometric::{betweenness, closeness, harmonic};
pub use pagerank::{Chain, limit, pagerank, terminal_components};
pub use power::{eigenvector_iterates, seeley_iterates};
pub use scc::{sccs, terminal_sccs};
pub use spectral::{Spectrum, katz};

use serde::{Deserialize, Serialize};

use crate::graph::{Graph, NodeId};

/// The number of values of the parameter sampled in [0 . . 1) by [`Ranking`]
/// for Katz's index and PageRank, which adds the right end, 1.
pub const SAMPLES: usize = 256;

/// The number *K* of iterations of the power method computing Seeley's index
/// and eigenvector centrality shown by [`Ranking`].
pub const ITERATIONS: usize = 50;

/// A centrality.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Centrality {
    /// Closeness centrality.
    Closeness,
    /// Harmonic centrality.
    Harmonic,
    /// Betweenness centrality.
    Betweenness,
    /// Eigenvector centrality.
    Eigenvector,
    /// Seeley's index.
    Seeley,
    /// Katz's index.
    Katz,
    /// PageRank.
    #[default]
    PageRank,
}

impl Centrality {
    /// All centralities, in menu order, that is, by family: geometric
    /// (closeness, harmonic, betweenness), spectral by eigenvector
    /// (eigenvector centrality, Seeley's index), and spectral by damped sum
    /// (Katz's index, PageRank); in each spectral family, the centrality of
    /// the adjacency matrix comes before that of its normalization.
    pub const ALL: [Self; 7] = [
        Self::Closeness,
        Self::Harmonic,
        Self::Betweenness,
        Self::Eigenvector,
        Self::Seeley,
        Self::Katz,
        Self::PageRank,
    ];

    /// Returns the name of the centrality, for display.
    pub const fn name(self) -> &'static str {
        match self {
            Self::PageRank => "PageRank",
            Self::Katz => "Katz's Index",
            Self::Seeley => "Seeley's Index",
            Self::Eigenvector => "Eigenvector Centrality",
            Self::Closeness => "Closeness Centrality",
            Self::Harmonic => "Harmonic Centrality",
            Self::Betweenness => "Betweenness Centrality",
        }
    }
}

/// The x-axis of the curves of a [`Ranking`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// No parameter: the curves are constant.
    Flat,
    /// The iteration *k* = 0, 1, …, [`ITERATIONS`] of the power method
    /// computing eigenvector centrality or Seeley's index.
    Iteration,
    /// βρ ∈ [0 . . 1], where β is the parameter of Katz's index and ρ > 0 is
    /// the spectral radius of the adjacency matrix.
    BetaRho,
    /// The parameter β ∈ [0 . . 1] of Katz's index, when ρ = 0.
    Beta,
    /// The damping factor α ∈ [0 . . 1] of PageRank.
    Alpha,
}

impl Axis {
    /// Returns the label of the axis: the empty string for
    /// [`Flat`](Self::Flat), and "k", "βρ", "β", or "α" otherwise.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Flat => "",
            Self::Iteration => "k",
            Self::BetaRho => "βρ",
            Self::Beta => "β",
            Self::Alpha => "α",
        }
    }

    /// Returns the right end of the axis: [`ITERATIONS`] for
    /// [`Iteration`](Self::Iteration), and 1 otherwise.
    pub const fn x_max(self) -> f64 {
        match self {
            Self::Iteration => ITERATIONS as f64,
            _ => 1.0,
        }
    }
}

/// Returns a vector divided by the sum of its entries, if the sum is
/// positive, and unchanged otherwise.
fn normalized(mut v: Vec<f64>) -> Vec<f64> {
    let sum: f64 = v.iter().sum();
    if sum > 0.0 {
        for x in &mut v {
            *x /= sum;
        }
    }
    v
}

/// Returns the curves of `n` nodes from a sequence of score vectors, that
/// is, the transposed sequence: `curves[i][k]` is `vectors[k][i]`.
fn curves_of(vectors: impl IntoIterator<Item = Vec<f64>>, n: usize) -> Vec<Vec<f64>> {
    let mut curves = vec![Vec::new(); n];
    for v in vectors {
        for (curve, x) in curves.iter_mut().zip(v) {
            curve.push(x);
        }
    }
    curves
}

/// What [`Ranking::scores`] needs, besides the curves, to compute exact
/// scores between samples.
#[derive(Clone, Debug)]
enum Exact {
    /// Nothing: scores are read from the curves.
    Curves,
    /// The chain of the graph, for PageRank.
    PageRank(Chain),
    /// For Katz's index, the successor lists; ρ, or 1 if ρ = 0, so that
    /// β = *x* / `scale`; and the limit at βρ = 1, if ρ > 0.
    Katz {
        succ: Vec<Vec<usize>>,
        scale: f64,
        limit: Option<Vec<f64>>,
    },
}

/// The curves of a centrality on a graph, that is, the score of each node
/// as a function of the parameter of the centrality, sampled along the
/// [x-axis](Axis).
///
/// All score vectors, in the curves and from [`scores`](Self::scores), are
/// normalized to sum to 1, except for those that are identically zero.
#[derive(Clone, Debug)]
pub struct Ranking {
    /// The centrality.
    pub centrality: Centrality,
    /// The x-axis: [`Axis::Flat`] for closeness, harmonic, and betweenness;
    /// [`Axis::Iteration`] for eigenvector centrality and Seeley's index;
    /// for Katz's index, [`Axis::BetaRho`] if ρ > 0 and [`Axis::Beta`] if
    /// ρ = 0; and [`Axis::Alpha`] for PageRank.
    pub axis: Axis,
    /// The node identifiers, in node index order.
    pub ids: Vec<NodeId>,
    /// The sampled values of the x-axis: 0 and 1 for the
    /// [flat](Axis::Flat) centralities; the iterations *k* = 0, 1, …,
    /// [`ITERATIONS`] for eigenvector centrality and Seeley's index, stopping
    /// before [`vanished`](Self::vanished), if any; and *k* / [`SAMPLES`] for
    /// 0 ≤ *k* < [`SAMPLES`], followed by 1, for Katz's index and PageRank.
    pub xs: Vec<f64>,
    /// `curves[i][k]` is the score of `ids[i]` at `xs[k]`.
    ///
    /// For Katz's index with ρ > 0, the score at βρ = 1 is the exact
    /// [limit](Spectrum::katz_limit); for the [flat](Axis::Flat)
    /// centralities, the two samples are equal.
    pub curves: Vec<Vec<f64>>,
    /// The groups of nodes to shade, each sorted by node index, and sorted
    /// by their first node.
    ///
    /// For PageRank and Seeley's index, they are the sinks of the graph,
    /// that is, its [terminal components](terminal_sccs) other than lone
    /// dangling nodes, which the walk leaves by jumping to every node; for
    /// Katz's index and eigenvector centrality, they are the [dominant
    /// classes](Spectrum::dominant_classes), if ρ > 0; otherwise, there are
    /// none.
    pub hulls: Vec<Vec<NodeId>>,
    /// The label of the [hulls](Self::hulls): "sinks" for PageRank and
    /// Seeley's index, "dominant" for Katz's index with ρ > 0 and for
    /// eigenvector centrality, and the empty string otherwise.
    pub hull_label: &'static str,
    /// The dangling nodes, for PageRank and Seeley's index; empty otherwise.
    pub dangling: Vec<NodeId>,
    /// The spectral radius ρ of the adjacency matrix, for Katz's index and
    /// eigenvector centrality; `None` otherwise.
    pub rho: Option<f64>,
    /// For eigenvector centrality, the first iteration *k* such that **1**
    /// *A*<sup>*k*</sup> = 0, if *k* ≤ [`ITERATIONS`]; the curves stop at *k*
    /// − 1.
    pub vanished: Option<usize>,
    /// What [`scores`](Self::scores) needs besides the curves.
    exact: Exact,
}

impl Ranking {
    /// Computes the curves of a centrality on a graph.
    pub fn new(graph: &Graph, centrality: Centrality) -> Self {
        let ids: Vec<NodeId> = graph.nodes().iter().map(|node| node.id).collect();
        let n = ids.len();
        let succ = graph.successors();
        let samples = || -> Vec<f64> {
            (0..SAMPLES)
                .map(|k| k as f64 / SAMPLES as f64)
                .chain([1.0])
                .collect()
        };
        // The defaults are those of the flat centralities.
        let mut r = Self {
            centrality,
            axis: Axis::Flat,
            ids,
            xs: vec![0.0, 1.0],
            curves: Vec::new(),
            hulls: Vec::new(),
            hull_label: "",
            dangling: Vec::new(),
            rho: None,
            vanished: None,
            exact: Exact::Curves,
        };
        match centrality {
            Centrality::PageRank | Centrality::Seeley => {
                let chain = Chain::new(succ);
                r.hulls = r.node_ids(terminal_sccs(chain.successors()).into_iter().filter(
                    |component| !(component.len() == 1 && chain.is_dangling(component[0])),
                ));
                r.hull_label = "sinks";
                r.dangling = (0..n)
                    .filter(|&v| chain.is_dangling(v))
                    .map(|v| r.ids[v])
                    .collect();
                if centrality == Centrality::PageRank {
                    r.axis = Axis::Alpha;
                    r.xs = samples();
                    r.exact = Exact::PageRank(chain);
                    r.curves = curves_of(r.xs.iter().map(|&x| r.scores(x)), n);
                } else {
                    r.axis = Axis::Iteration;
                    r.xs = (0..=ITERATIONS).map(|k| k as f64).collect();
                    let iterates = seeley_iterates(&chain, ITERATIONS);
                    r.curves = curves_of(iterates.into_iter().map(normalized), n);
                }
            }
            Centrality::Katz => {
                let spectrum = Spectrum::new(&succ);
                let rho = spectrum.rho();
                r.rho = Some(rho);
                r.xs = samples();
                if rho > 0.0 {
                    r.axis = Axis::BetaRho;
                    r.hulls = r.node_ids(spectrum.dominant_classes());
                    r.hull_label = "dominant";
                    r.exact = Exact::Katz {
                        succ,
                        scale: rho,
                        limit: Some(normalized(spectrum.katz_limit())),
                    };
                } else {
                    r.axis = Axis::Beta;
                    r.exact = Exact::Katz {
                        succ,
                        scale: 1.0,
                        limit: None,
                    };
                }
                r.curves = curves_of(r.xs.iter().map(|&x| r.scores(x)), n);
            }
            Centrality::Eigenvector => {
                let spectrum = Spectrum::new(&succ);
                r.rho = Some(spectrum.rho());
                r.hulls = r.node_ids(spectrum.dominant_classes());
                r.hull_label = "dominant";
                r.axis = Axis::Iteration;
                let iterates = eigenvector_iterates(&succ, ITERATIONS);
                let len = iterates.len();
                r.vanished = (len <= ITERATIONS).then_some(len);
                r.xs = (0..len).map(|k| k as f64).collect();
                r.curves = curves_of(iterates.into_iter().map(normalized), n);
            }
            Centrality::Closeness | Centrality::Harmonic | Centrality::Betweenness => {
                let scores = normalized(match centrality {
                    Centrality::Closeness => closeness(&succ),
                    Centrality::Harmonic => harmonic(&succ),
                    _ => betweenness(&succ),
                });
                r.curves = curves_of([scores.clone(), scores], n);
            }
        }
        r
    }

    /// Maps components given by node indices to node identifiers.
    fn node_ids(&self, components: impl IntoIterator<Item = Vec<usize>>) -> Vec<Vec<NodeId>> {
        components
            .into_iter()
            .map(|component| component.into_iter().map(|v| self.ids[v]).collect())
            .collect()
    }

    /// Returns the exact scores at `x` on the [x-axis](Self::axis), aligned
    /// with [`ids`](Self::ids).
    ///
    /// - [`Axis::Alpha`]: PageRank with damping factor *x*, clamped to [0,
    ///   1].
    /// - [`Axis::BetaRho`]: Katz's index with β = *x* / ρ, for *x* clamped
    ///   to [0 . . 1), and its exact limit for *x* ≥ 1.
    /// - [`Axis::Beta`]: Katz's index with β = *x*, clamped to [0 . . 1].
    /// - [`Axis::Iteration`]: the iterate at *k* = *x*, rounded, or zero if
    ///   *k* is beyond the last sample, as it happens from the iteration at
    ///   which the iterates [vanish](Self::vanished).
    /// - [`Axis::Flat`]: the scores, whatever *x*.
    pub fn scores(&self, x: f64) -> Vec<f64> {
        match &self.exact {
            Exact::PageRank(chain) => normalized(pagerank(chain, x.clamp(0.0, 1.0))),
            Exact::Katz {
                limit: Some(limit), ..
            } if x >= 1.0 => limit.clone(),
            Exact::Katz { succ, scale, .. } => normalized(katz(succ, x.clamp(0.0, 1.0) / scale)),
            Exact::Curves => {
                let k = match self.axis {
                    Axis::Flat => 0,
                    _ => x.round().max(0.0) as usize,
                };
                if k < self.xs.len() {
                    self.curves.iter().map(|curve| curve[k]).collect()
                } else {
                    vec![0.0; self.ids.len()]
                }
            }
        }
    }

    /// Returns the curve of a node, if the node is part of this ranking.
    pub fn curve(&self, id: NodeId) -> Option<&[f64]> {
        self.ids
            .iter()
            .position(|&x| x == id)
            .map(|i| self.curves[i].as_slice())
    }

    /// Returns the maximum value over all curves (zero if there are none).
    pub fn max_value(&self) -> f64 {
        self.curves.iter().flatten().fold(0.0, |m, &x| x.max(m))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn example_after() -> Chain {
        Chain::new(vec![vec![1], vec![2], vec![0, 3], vec![4], vec![3]])
    }

    pub(crate) fn example_before() -> Chain {
        Chain::new(vec![vec![1], vec![2], vec![0, 3], vec![4], vec![]])
    }

    pub(crate) fn assert_close(actual: &[f64], expected: &[f64], tol: f64) {
        assert_eq!(actual.len(), expected.len());
        for (a, e) in actual.iter().zip(expected) {
            assert!((a - e).abs() <= tol, "{actual:?} != {expected:?}");
        }
    }

    #[test]
    fn test_ranking_sinks() {
        let mut g = Graph::new(true);
        let ids: Vec<_> = (0..3).map(|i| g.add_node([i as f32, 0.0])).collect();
        g.add_arc(ids[0], ids[1]);
        g.add_arc(ids[1], ids[2]);
        // c is terminal, but dangling
        assert!(Ranking::new(&g, Centrality::PageRank).hulls.is_empty());
        g.add_arc(ids[2], ids[2]);
        assert_eq!(
            Ranking::new(&g, Centrality::PageRank).hulls,
            vec![vec![ids[2]]]
        );
        // a dangling node beside a sink
        let d = g.add_node([3.0, 0.0]);
        g.add_arc(ids[0], d);
        assert_eq!(
            Ranking::new(&g, Centrality::PageRank).hulls,
            vec![vec![ids[2]]]
        );
    }

    #[test]
    fn test_terminal_components_are_sinks() {
        let mut g = Graph::new(true);
        let ids: Vec<_> = (0..6).map(|i| g.add_node([i as f32, 0.0])).collect();
        // f is dangling; c↔d and the self-loop on e are sinks
        for (s, t) in [(0, 1), (0, 2), (2, 3), (3, 2), (1, 4), (4, 4), (0, 5)] {
            g.add_arc(ids[s], ids[t]);
        }
        let chain = Chain::from_graph(&g);
        assert_eq!(terminal_components(&chain), vec![vec![2, 3], vec![4]]);
        assert_eq!(
            Ranking::new(&g, Centrality::PageRank).hulls,
            vec![vec![ids[2], ids[3]], vec![ids[4]]]
        );
    }

    #[test]
    fn test_ranking_new() {
        let mut g = Graph::new(true);
        let ids: Vec<_> = (0..5).map(|i| g.add_node([i as f32, 0.0])).collect();
        for (s, t) in [(0, 1), (1, 2), (2, 0), (2, 3), (3, 4), (4, 3)] {
            g.add_arc(ids[s], ids[t]);
        }
        let r = Ranking::new(&g, Centrality::PageRank);
        assert_eq!(r.ids, ids);
        assert_eq!(r.xs.len(), SAMPLES + 1);
        assert_eq!(r.xs[0], 0.0);
        assert_eq!(*r.xs.last().unwrap(), 1.0);
        assert!((r.curve(ids[3]).unwrap().last().unwrap() - 0.5).abs() < 1e-12);
        assert_eq!(r.hulls, vec![vec![ids[3], ids[4]]]);
        assert!(r.dangling.is_empty());
        assert_close(&r.scores(0.85), &pagerank(&example_after(), 0.85), 1e-12);
    }

    #[test]
    fn test_ranking_empty_graph() {
        let r = Ranking::new(&Graph::new(true), Centrality::PageRank);
        assert!(r.curves.is_empty());
        assert_eq!(r.max_value(), 0.0);
        assert!(r.scores(0.5).is_empty());
    }

    #[test]
    fn test_ranking_single_node() {
        let mut g = Graph::new(true);
        let a = g.add_node([0.0; 2]);
        let r = Ranking::new(&g, Centrality::PageRank);
        assert!(r.curve(a).unwrap().iter().all(|&x| (x - 1.0).abs() < 1e-12));
        assert_eq!(r.dangling, vec![a]);
    }

    /// Returns a graph with `n` nodes and the given arcs (edges, if the
    /// graph is undirected).
    fn graph(directed: bool, n: usize, arcs: &[(usize, usize)]) -> Graph {
        let mut g = Graph::new(directed);
        let ids: Vec<_> = (0..n).map(|i| g.add_node([i as f32, 0.0])).collect();
        for &(s, t) in arcs {
            g.add_arc(ids[s], ids[t]);
        }
        g
    }

    /// *a* → *b* → *c* → *a*, *c* → *d*, *d* ⇄ *e*.
    fn lecture_graph() -> Graph {
        graph(true, 5, &[(0, 1), (1, 2), (2, 0), (2, 3), (3, 4), (4, 3)])
    }

    /// *a* → *b* → *c*.
    fn path_graph() -> Graph {
        graph(true, 3, &[(0, 1), (1, 2)])
    }

    /// The undirected star with center a and three leaves.
    fn star_graph() -> Graph {
        graph(false, 4, &[(0, 1), (0, 2), (0, 3)])
    }

    /// The empty graph; one node; one node with a self-loop; three nodes and
    /// no arcs; the lecture example.
    fn small_graphs() -> Vec<Graph> {
        vec![
            graph(true, 0, &[]),
            graph(true, 1, &[]),
            graph(true, 1, &[(0, 0)]),
            graph(true, 3, &[]),
            lecture_graph(),
        ]
    }

    #[test]
    fn test_every_centrality_on_small_graphs() {
        for g in small_graphs() {
            for c in Centrality::ALL {
                let r = Ranking::new(&g, c);
                assert_eq!(r.centrality, c);
                assert!(r.curves.iter().flatten().all(|x| x.is_finite()));
                for curve in &r.curves {
                    assert_eq!(curve.len(), r.xs.len());
                }
                for k in 0..r.xs.len() {
                    let sum: f64 = r.curves.iter().map(|c| c[k]).sum();
                    assert!(
                        sum.abs() < 1e-12 || (sum - 1.0).abs() < 1e-12,
                        "{c:?} {sum}"
                    );
                }
                let max = r.axis.x_max();
                for x in [0.0, 0.5 * max, max] {
                    let scores = r.scores(x);
                    assert_eq!(scores.len(), r.ids.len());
                    assert!(scores.iter().all(|x| x.is_finite()));
                }
            }
        }
        // With no arcs, the geometric centralities are zero, and the
        // eigenvector iterates vanish at k = 1
        let no_arcs = graph(true, 3, &[]);
        for c in [
            Centrality::Closeness,
            Centrality::Harmonic,
            Centrality::Betweenness,
        ] {
            let r = Ranking::new(&no_arcs, c);
            assert!(r.curves.iter().flatten().all(|&x| x == 0.0), "{c:?}");
        }
        let eig = Ranking::new(&no_arcs, Centrality::Eigenvector);
        assert_eq!((eig.xs.len(), eig.vanished), (1, Some(1)));
    }

    #[test]
    fn test_ranking_axes() {
        let lecture = lecture_graph();
        assert_eq!(
            Ranking::new(&lecture, Centrality::PageRank).axis,
            Axis::Alpha
        );
        let katz = Ranking::new(&lecture, Centrality::Katz);
        assert_eq!(katz.axis, Axis::BetaRho);
        assert_eq!(katz.hulls.len(), 2); // a b c and d e both have ρ = 1
        assert_eq!(katz.hull_label, "dominant");
        assert_eq!(
            Ranking::new(&path_graph(), Centrality::Katz).axis,
            Axis::Beta
        );
        let seeley = Ranking::new(&lecture, Centrality::Seeley);
        assert_eq!(
            (seeley.axis, seeley.xs.len()),
            (Axis::Iteration, ITERATIONS + 1)
        );
        let eig = Ranking::new(&path_graph(), Centrality::Eigenvector);
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
        assert_close(
            &r.scores(0.85),
            &katz(&lecture_graph().successors(), 0.85 / s.rho()),
            1e-12,
        );
    }

    #[test]
    fn test_flat_rankings_are_normalized() {
        let r = Ranking::new(&star_graph(), Centrality::Betweenness);
        assert_close(&r.scores(0.0), &[1.0, 0.0, 0.0, 0.0], 1e-12);
    }

    /// Run with `cargo test --release -- --ignored`.
    #[test]
    #[ignore]
    fn test_ranking_speed_100_all() {
        let mut arcs = Vec::new();
        for i in 0..100 {
            arcs.push((i, (i * 7 + 1) % 100));
            arcs.push((i, (i * 13 + 5) % 100));
        }
        let pseudo_random = graph(true, 100, &arcs);
        // A directed 100-cycle with one chord
        let mut arcs: Vec<_> = (0..100).map(|i| (i, (i + 1) % 100)).collect();
        arcs.push((0, 50));
        let cycle = graph(true, 100, &arcs);
        for g in [pseudo_random, cycle] {
            for c in Centrality::ALL {
                let start = std::time::Instant::now();
                let _ = Ranking::new(&g, c);
                assert!(
                    start.elapsed().as_millis() < 50,
                    "{c:?} {:?}",
                    start.elapsed()
                );
            }
        }
    }
}
