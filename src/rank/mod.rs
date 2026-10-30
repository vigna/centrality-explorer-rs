/*
 * SPDX-FileCopyrightText: 2026 Sebastiano Vigna
 *
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 */

//! Exact centralities of small graphs.

mod geometric;
mod lu;
mod pagerank;
mod power;
mod scc;
mod spectral;

pub use geometric::{betweenness, closeness, harmonic, indegree};
pub use pagerank::{Chain, limit, pagerank, terminal_components};
pub use power::{eigenvector_iterates, eigenvector_means, seeley_iterates, seeley_means};
pub use scc::{sccs, terminal_sccs};
pub use spectral::{Spectrum, katz};

use serde::{Deserialize, Serialize};

use crate::graph::{Graph, NodeId};

/// The number of values of the parameter sampled in [0 . . 1) by [`Ranking`]
/// for Katz's index and PageRank, which adds the right end, 1.
pub const SAMPLES: usize = 256;

/// The default number *K* of iterations of the power method computing
/// Seeley's index and eigenvector centrality shown by [`Ranking`].
pub const DEFAULT_ITERATIONS: usize = 50;

/// The maximum number *K* of iterations of the power method computing
/// Seeley's index and eigenvector centrality shown by [`Ranking`].
pub const MAX_ITERATIONS: usize = 1000;

/// A centrality.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Centrality {
    /// Indegree (degree, on undirected graphs).
    InDegree,
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

/// A family of centralities, whose curves have the same kind of
/// [x-axis](Axis).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// Indegree, closeness, harmonic centrality, and betweenness, whose curves
    /// are [flat](Axis::Flat).
    Geometric,
    /// Eigenvector centrality and Seeley's index, whose curves follow the
    /// [iterations](Axis::Iteration) of the power method.
    Eigenvector,
    /// Katz's index and PageRank, whose curves follow a damping factor in
    /// [0 . . 1].
    DampedSum,
}

impl Centrality {
    /// All centralities, in menu order, that is, by family: geometric
    /// (indegree, closeness, harmonic, betweenness), spectral by eigenvector
    /// (eigenvector centrality, Seeley's index), and spectral by damped sum
    /// (Katz's index, PageRank); in each spectral family, the centrality of
    /// the adjacency matrix comes before that of its normalization.
    pub const ALL: [Self; 8] = [
        Self::InDegree,
        Self::Closeness,
        Self::Harmonic,
        Self::Betweenness,
        Self::Eigenvector,
        Self::Seeley,
        Self::Katz,
        Self::PageRank,
    ];

    /// Returns the name of the centrality, for display on a directed or an
    /// undirected graph: only indegree changes, to degree, on undirected
    /// graphs.
    pub const fn name(self, directed: bool) -> &'static str {
        match self {
            Self::InDegree if directed => "Indegree",
            Self::InDegree => "Degree",
            Self::PageRank => "PageRank",
            Self::Katz => "Katz's Index",
            Self::Seeley => "Seeley's Index",
            Self::Eigenvector => "Eigenvector Centrality",
            Self::Closeness => "Closeness Centrality",
            Self::Harmonic => "Harmonic Centrality",
            Self::Betweenness => "Betweenness Centrality",
        }
    }

    /// Returns the family of the centrality.
    pub const fn family(self) -> Family {
        match self {
            Self::InDegree | Self::Closeness | Self::Harmonic | Self::Betweenness => {
                Family::Geometric
            }
            Self::Eigenvector | Self::Seeley => Family::Eigenvector,
            Self::Katz | Self::PageRank => Family::DampedSum,
        }
    }
}

/// The normalization of score vectors chosen by the user (see
/// [`norm`](Self::norm)).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Normalization {
    /// Every centrality is normalized in ℓ1, so that scores are comparable
    /// across centralities.
    #[default]
    L1,
    /// Every centrality has its traditional normalization.
    Traditional,
}

impl Normalization {
    /// Returns the norm of the score vectors of a centrality under this
    /// normalization.
    ///
    /// Under [`L1`](Self::L1), it is always [`Norm::L1`]. Under
    /// [`Traditional`](Self::Traditional), it is [`Norm::L1`] for Seeley's
    /// index and PageRank, which are distributions; [`Norm::L2`] for
    /// eigenvector centrality; and [`Norm::None`] for the geometric
    /// centralities and Katz's index.
    pub const fn norm(self, centrality: Centrality) -> Norm {
        match (self, centrality) {
            (Self::L1, _) | (_, Centrality::Seeley | Centrality::PageRank) => Norm::L1,
            (_, Centrality::Eigenvector) => Norm::L2,
            _ => Norm::None,
        }
    }
}

/// The norm of the score vectors of a [`Ranking`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Norm {
    /// Divided by the sum of their entries, which are nonnegative.
    L1,
    /// Divided by their Euclidean norm.
    L2,
    /// Not normalized.
    None,
}

impl Norm {
    /// Returns the description of the norm, for display: "ℓ₁-normalized",
    /// "ℓ₂-normalized", or "not normalized".
    pub const fn label(self) -> &'static str {
        match self {
            Self::L1 => "ℓ₁-normalized",
            Self::L2 => "ℓ₂-normalized",
            Self::None => "not normalized",
        }
    }

    /// Returns a vector normalized in this norm; a vector that is
    /// identically zero is unchanged.
    fn apply(self, v: Vec<f64>) -> Vec<f64> {
        match self {
            Self::L1 => normalized(v),
            Self::L2 => {
                let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
                if norm > 0.0 {
                    v.into_iter().map(|x| x / norm).collect()
                } else {
                    v
                }
            }
            Self::None => v,
        }
    }
}

/// The options of a [`Ranking`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    /// The normalization of the score vectors.
    pub normalization: Normalization,
    /// The number *K* of iterations of the power method computing
    /// eigenvector centrality and Seeley's index, in
    /// [1 . . [`MAX_ITERATIONS`]].
    pub iterations: usize,
    /// Whether the curves of eigenvector centrality and Seeley's index show
    /// the means of the iterates of the power method (see
    /// [`eigenvector_means`] and [`seeley_means`]) rather than the iterates.
    pub mean: bool,
}

impl Default for Options {
    /// ℓ1 normalization and [`DEFAULT_ITERATIONS`] iterations, showing the
    /// iterates.
    fn default() -> Self {
        Self {
            normalization: Normalization::L1,
            iterations: DEFAULT_ITERATIONS,
            mean: false,
        }
    }
}

/// The x-axis of the curves of a [`Ranking`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// No parameter: the curves are constant.
    Flat,
    /// The iteration *k* = 0, 1, …, *K* of the power method computing
    /// eigenvector centrality or Seeley's index, where *K* is the number of
    /// iterations.
    Iteration(usize),
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
    /// [`Flat`](Self::Flat), "k (iterations of the power method)" for
    /// [`Iteration`](Self::Iteration), and "βρ", "β", or "α" otherwise.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Flat => "",
            Self::Iteration(_) => "k (iterations of the power method)",
            Self::BetaRho => "βρ",
            Self::Beta => "β",
            Self::Alpha => "α",
        }
    }

    /// Returns the right end of the axis: the number *K* of iterations for
    /// [`Iteration`](Self::Iteration), and 1 otherwise.
    pub const fn x_max(self) -> f64 {
        match self {
            Self::Iteration(iterations) => iterations as f64,
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
    /// β = *x* / `scale`; and, if ρ > 0, the limit at βρ = 1 of the scores
    /// and of the shares (see [`Ranking::shares`]), which differ only if the
    /// scores are not normalized: then, they are infinite on the nodes
    /// reachable from a dominant class
    /// ([`Spectrum::katz_unnormalized_limit`]).
    Katz {
        succ: Vec<Vec<usize>>,
        scale: f64,
        limit: Option<KatzLimit>,
    },
}

/// The limit of Katz's index at βρ = 1, when ρ > 0.
#[derive(Clone, Debug)]
struct KatzLimit {
    scores: Vec<f64>,
    shares: Vec<f64>,
}

/// The curves of a centrality on a graph, that is, the score of each node
/// as a function of the parameter of the centrality, sampled along the
/// [x-axis](Axis).
///
/// All score vectors, in the curves and from [`scores`](Self::scores), are
/// normalized in the [norm](Self::norm) of the ranking, except for those that
/// are identically zero; [`shares`](Self::shares) are always normalized in ℓ1.
#[derive(Clone, Debug)]
pub struct Ranking {
    /// The centrality.
    pub centrality: Centrality,
    /// The norm of the score vectors (see [`Normalization::norm`]).
    pub norm: Norm,
    /// The x-axis: [`Axis::Flat`] for closeness, harmonic, and betweenness;
    /// [`Axis::Iteration`], with the number *K* of iterations of the power
    /// method, for eigenvector centrality and Seeley's index; for Katz's
    /// index, [`Axis::BetaRho`] if ρ > 0 and [`Axis::Beta`] if ρ = 0; and
    /// [`Axis::Alpha`] for PageRank.
    pub axis: Axis,
    /// The node identifiers, in node index order.
    pub ids: Vec<NodeId>,
    /// The sampled values of the x-axis: 0 and 1 for the
    /// [flat](Axis::Flat) centralities; the iterations *k* = 0, 1, …, *K* for
    /// eigenvector centrality and Seeley's index, stopping before
    /// [`vanished`](Self::vanished), if any; and *k* / [`SAMPLES`] for 0 ≤
    /// *k* < [`SAMPLES`], followed by 1 unless the scores are
    /// [unbounded](Self::unbounded), for Katz's index and PageRank.
    pub xs: Vec<f64>,
    /// `curves[i][k]` is the score of `ids[i]` at `xs[k]`.
    ///
    /// For Katz's index with ρ > 0, the score at βρ = 1, if sampled, is the
    /// exact [limit](Spectrum::katz_limit); for the [flat](Axis::Flat)
    /// centralities, the two samples are equal; for eigenvector centrality
    /// and Seeley's index, the scores are the iterates of the power method
    /// or, if so [requested](Options::mean), their means.
    pub curves: Vec<Vec<f64>>,
    /// Whether the scores diverge at the right end of the axis, as those of
    /// Katz's index with ρ > 0, when [not normalized](Norm::None), do as βρ
    /// → 1; then, the right end is not sampled.
    pub unbounded: bool,
    /// The groups of nodes to shade, each sorted by node index, and sorted
    /// by their first node.
    ///
    /// For PageRank and Seeley's index, they are the buckets of the graph,
    /// that is, its [terminal components](terminal_sccs) other than lone
    /// dangling nodes (the sinks of the graph), which the walk leaves by
    /// jumping to every node; for
    /// Katz's index and eigenvector centrality, they are the [dominant
    /// classes](Spectrum::dominant_classes), if ρ > 0; otherwise, there are
    /// none.
    pub hulls: Vec<Vec<NodeId>>,
    /// The label of the [hulls](Self::hulls): "buckets" for PageRank and
    /// Seeley's index, "dominant" for Katz's index with ρ > 0 and for
    /// eigenvector centrality, and the empty string otherwise.
    pub hull_label: &'static str,
    /// The dangling nodes, for PageRank and Seeley's index; empty otherwise.
    pub dangling: Vec<NodeId>,
    /// The spectral radius ρ of the adjacency matrix, for Katz's index and
    /// eigenvector centrality; `None` otherwise.
    pub rho: Option<f64>,
    /// For eigenvector centrality, the first iteration *k* such that **1**
    /// *A*<sup>*k*</sup> = 0, if *k* ≤ *K*; the curves stop at *k* − 1. Since
    /// the iterates vanish only if ρ = 0, and their means are shown only if ρ >
    /// 0, the means never vanish.
    pub vanished: Option<usize>,
    /// If this is another ranking [frozen](Self::at) at a value of its
    /// parameter, the x-axis of the other ranking and the value.
    pub frozen: Option<(Axis, f64)>,
    /// What [`scores`](Self::scores) needs besides the curves.
    exact: Exact,
}

impl Ranking {
    /// Computes the curves of a centrality on a graph, with the
    /// [default](Options::default) options.
    pub fn new(graph: &Graph, centrality: Centrality) -> Self {
        Self::with_options(graph, centrality, Options::default())
    }

    /// Computes the curves of a centrality on a graph, with the given options.
    ///
    /// The means of the iterates of eigenvector centrality are shown only if
    /// ρ > 0: if ρ = 0, there are no means, and the curves show the iterates,
    /// which vanish.
    ///
    /// Betweenness, when not normalized, counts the shortest paths of an
    /// undirected graph once, rather than once in each direction.
    pub fn with_options(graph: &Graph, centrality: Centrality, options: Options) -> Self {
        let iterations = options.iterations;
        let norm = options.normalization.norm(centrality);
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
            norm,
            axis: Axis::Flat,
            ids,
            xs: vec![0.0, 1.0],
            curves: Vec::new(),
            unbounded: false,
            hulls: Vec::new(),
            hull_label: "",
            dangling: Vec::new(),
            rho: None,
            vanished: None,
            frozen: None,
            exact: Exact::Curves,
        };
        match centrality {
            Centrality::PageRank | Centrality::Seeley => {
                let chain = Chain::new(succ);
                r.hulls = r.node_ids(terminal_sccs(chain.successors()).into_iter().filter(
                    |component| !(component.len() == 1 && chain.is_dangling(component[0])),
                ));
                r.hull_label = "buckets";
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
                    r.axis = Axis::Iteration(iterations);
                    r.xs = (0..=iterations).map(|k| k as f64).collect();
                    let vectors = if options.mean {
                        seeley_means(&chain, iterations)
                    } else {
                        seeley_iterates(&chain, iterations)
                    };
                    r.curves = curves_of(vectors.into_iter().map(|v| norm.apply(v)), n);
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
                    let shares = normalized(spectrum.katz_limit());
                    let scores = match norm {
                        Norm::None => spectrum.katz_unnormalized_limit(),
                        _ => norm.apply(shares.clone()),
                    };
                    r.unbounded = norm == Norm::None;
                    if r.unbounded {
                        r.xs.pop();
                    }
                    r.exact = Exact::Katz {
                        succ,
                        scale: rho,
                        limit: Some(KatzLimit { scores, shares }),
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
                let rho = spectrum.rho();
                r.rho = Some(rho);
                r.hulls = r.node_ids(spectrum.dominant_classes());
                r.hull_label = "dominant";
                r.axis = Axis::Iteration(iterations);
                let vectors = if options.mean && rho > 0.0 {
                    eigenvector_means(&succ, rho, iterations)
                } else {
                    eigenvector_iterates(&succ, iterations)
                };
                let len = vectors.len();
                r.vanished = (len <= iterations).then_some(len);
                r.xs = (0..len).map(|k| k as f64).collect();
                r.curves = curves_of(vectors.into_iter().map(|v| norm.apply(v)), n);
            }
            Centrality::InDegree
            | Centrality::Closeness
            | Centrality::Harmonic
            | Centrality::Betweenness => {
                let mut scores = match centrality {
                    Centrality::InDegree => indegree(&succ),
                    Centrality::Closeness => closeness(&succ),
                    Centrality::Harmonic => harmonic(&succ),
                    _ => betweenness(&succ),
                };
                if centrality == Centrality::Betweenness && !graph.is_directed() {
                    for x in &mut scores {
                        *x /= 2.0;
                    }
                }
                let scores = norm.apply(scores);
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
    ///   to [0 . . 1), and its exact limit for *x* ≥ 1, which is infinite on
    ///   some nodes if the scores are [unbounded](Self::unbounded).
    /// - [`Axis::Beta`]: Katz's index with β = *x*, clamped to [0 . . 1].
    /// - [`Axis::Iteration`]: the iterate (or the mean) at *k* = *x*,
    ///   rounded, or zero if *k* is beyond the last sample, as it happens
    ///   from the iteration at which the iterates [vanish](Self::vanished).
    /// - [`Axis::Flat`]: the scores, whatever *x*.
    pub fn scores(&self, x: f64) -> Vec<f64> {
        match &self.exact {
            Exact::PageRank(chain) => normalized(pagerank(chain, x.clamp(0.0, 1.0))),
            Exact::Katz {
                limit: Some(limit), ..
            } if x >= 1.0 => limit.scores.clone(),
            Exact::Katz { succ, scale, .. } => {
                self.norm.apply(katz(succ, x.clamp(0.0, 1.0) / scale))
            }
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

    /// Returns the shares at `x` on the [x-axis](Self::axis), aligned with
    /// [`ids`](Self::ids): the [scores](Self::scores) normalized in ℓ1, or,
    /// for Katz's index at βρ ≥ 1, the normalized
    /// [limit](Spectrum::katz_limit), even when the scores are infinite.
    pub fn shares(&self, x: f64) -> Vec<f64> {
        match &self.exact {
            Exact::Katz {
                limit: Some(limit), ..
            } if x >= 1.0 => limit.shares.clone(),
            _ => normalized(self.scores(x)),
        }
    }

    /// Returns the ranking frozen at `x` on the [x-axis](Self::axis): a
    /// [flat](Axis::Flat) ranking whose scores are the [scores](Self::scores)
    /// at `x`, recording the axis and `x` in [`frozen`](Self::frozen).
    ///
    /// The scores of an [unbounded](Self::unbounded) ranking at the right end
    /// of the axis, and thus the curves of the result, are infinite.
    pub fn at(&self, x: f64) -> Self {
        Self {
            axis: Axis::Flat,
            xs: vec![0.0, 1.0],
            curves: self.scores(x).into_iter().map(|s| vec![s, s]).collect(),
            unbounded: false,
            vanished: None,
            frozen: Some((self.axis, x)),
            exact: Exact::Curves,
            ..self.clone()
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
    fn test_ranking_buckets() {
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
        // a dangling node beside a bucket
        let d = g.add_node([3.0, 0.0]);
        g.add_arc(ids[0], d);
        assert_eq!(
            Ranking::new(&g, Centrality::PageRank).hulls,
            vec![vec![ids[2]]]
        );
    }

    #[test]
    fn test_terminal_components_are_buckets() {
        let mut g = Graph::new(true);
        let ids: Vec<_> = (0..6).map(|i| g.add_node([i as f32, 0.0])).collect();
        // f is dangling; c↔d and the self-loop on e are buckets
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
    fn example_graph() -> Graph {
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
    /// no arcs; the example graph.
    fn small_graphs() -> Vec<Graph> {
        vec![
            graph(true, 0, &[]),
            graph(true, 1, &[]),
            graph(true, 1, &[(0, 0)]),
            graph(true, 3, &[]),
            example_graph(),
        ]
    }

    /// Asserts that a vector is identically zero or has norm 1 in a norm
    /// (any vector passes if it is not normalized).
    fn assert_normalized(v: &[f64], norm: Norm) {
        let size = match norm {
            Norm::L1 => v.iter().sum::<f64>(),
            Norm::L2 => v.iter().map(|x| x * x).sum::<f64>(),
            Norm::None => return,
        };
        assert!(
            size.abs() < 1e-12 || (size - 1.0).abs() < 1e-12,
            "{norm:?} {v:?}"
        );
    }

    /// The default options; traditional normalization; means; both; and
    /// seven iterations.
    fn all_options() -> Vec<Options> {
        let default = Options::default();
        let traditional = Options {
            normalization: Normalization::Traditional,
            ..default
        };
        vec![
            default,
            traditional,
            Options {
                mean: true,
                ..default
            },
            Options {
                mean: true,
                ..traditional
            },
            Options {
                iterations: 7,
                ..default
            },
        ]
    }

    #[test]
    fn test_every_centrality_on_small_graphs() {
        for g in small_graphs() {
            for c in Centrality::ALL {
                for options in all_options() {
                    let r = Ranking::with_options(&g, c, options);
                    assert_eq!(r.centrality, c);
                    assert_eq!(r.norm, options.normalization.norm(c));
                    assert!(r.curves.iter().flatten().all(|x| x.is_finite()));
                    for curve in &r.curves {
                        assert_eq!(curve.len(), r.xs.len());
                    }
                    for k in 0..r.xs.len() {
                        let v: Vec<f64> = r.curves.iter().map(|c| c[k]).collect();
                        assert_normalized(&v, r.norm);
                    }
                    let max = r.axis.x_max();
                    for x in [0.0, 0.5 * max, max] {
                        let scores = r.scores(x);
                        assert_eq!(scores.len(), r.ids.len());
                        assert!(
                            scores.iter().all(|x| x.is_finite()) || r.unbounded && x == max,
                            "{c:?} {options:?}"
                        );
                        assert_normalized(&r.shares(x), Norm::L1);
                    }
                }
            }
        }
        // With no arcs, the geometric centralities are zero, and the
        // eigenvector iterates vanish at k = 1
        let no_arcs = graph(true, 3, &[]);
        for c in [
            Centrality::InDegree,
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
        let example = example_graph();
        assert_eq!(
            Ranking::new(&example, Centrality::PageRank).axis,
            Axis::Alpha
        );
        let katz = Ranking::new(&example, Centrality::Katz);
        assert_eq!(katz.axis, Axis::BetaRho);
        assert_eq!(katz.hulls.len(), 2); // a b c and d e both have ρ = 1
        assert_eq!(katz.hull_label, "dominant");
        assert_eq!(
            Ranking::new(&path_graph(), Centrality::Katz).axis,
            Axis::Beta
        );
        let seeley = Ranking::new(&example, Centrality::Seeley);
        assert_eq!(
            (seeley.axis, seeley.xs.len()),
            (Axis::Iteration(DEFAULT_ITERATIONS), DEFAULT_ITERATIONS + 1)
        );
        let eig = Ranking::new(&path_graph(), Centrality::Eigenvector);
        assert_eq!((eig.xs.len(), eig.vanished), (3, Some(3)));
        assert!(eig.scores(10.0).iter().all(|&x| x == 0.0));
        let flat = Ranking::new(&example, Centrality::Harmonic);
        assert_eq!((flat.axis, flat.xs.clone()), (Axis::Flat, vec![0.0, 1.0]));
        assert!(flat.hulls.is_empty() && flat.dangling.is_empty());
    }

    #[test]
    fn test_katz_ranking_endpoints() {
        let r = Ranking::new(&example_graph(), Centrality::Katz);
        assert_close(&r.scores(0.0), &[0.2; 5], 1e-15);
        let s = Spectrum::new(&example_graph().successors());
        assert_close(&r.scores(1.0), &s.katz_limit(), 1e-15);
        assert_close(
            &r.scores(0.85),
            &normalized(katz(&example_graph().successors(), 0.85 / s.rho())),
            1e-12,
        );
    }

    #[test]
    fn test_flat_rankings_are_normalized() {
        let r = Ranking::new(&star_graph(), Centrality::Betweenness);
        assert_close(&r.scores(0.0), &[1.0, 0.0, 0.0, 0.0], 1e-12);
    }

    /// Returns the ranking of a centrality with traditional normalization.
    fn traditional(g: &Graph, c: Centrality) -> Ranking {
        Ranking::with_options(
            g,
            c,
            Options {
                normalization: Normalization::Traditional,
                ..Options::default()
            },
        )
    }

    #[test]
    fn test_traditional_geometric() {
        // On a → b → c, harmonic centrality is 0, 1, and 1 + 1/2
        let r = traditional(&path_graph(), Centrality::Harmonic);
        assert_eq!(r.norm, Norm::None);
        assert_close(&r.scores(0.0), &[0.0, 1.0, 1.5], 1e-12);
        assert_close(&r.shares(0.0), &[0.0, 0.4, 0.6], 1e-12);
        // On the undirected star, each of the three pairs of leaves is
        // counted once
        let r = traditional(&star_graph(), Centrality::Betweenness);
        assert_close(&r.scores(0.0), &[3.0, 0.0, 0.0, 0.0], 1e-12);
        let directed = graph(true, 3, &[(0, 1), (1, 2), (1, 0), (2, 1)]);
        let r = traditional(&directed, Centrality::Betweenness);
        assert_close(&r.scores(0.0), &[0.0, 2.0, 0.0], 1e-12);
    }

    #[test]
    fn test_indegree_is_first_iterate() {
        // On a graph with self-loops, indegree is the first iterate of the
        // power method for eigenvector centrality
        let g = graph(true, 3, &[(0, 0), (0, 1), (1, 2), (2, 0), (2, 1)]);
        let (d, e) = (
            Ranking::new(&g, Centrality::InDegree),
            Ranking::new(&g, Centrality::Eigenvector),
        );
        assert_eq!(d.axis, Axis::Flat);
        assert_close(&d.scores(0.0), &e.scores(1.0), 1e-15);
        assert_close(
            &traditional(&g, Centrality::InDegree).scores(0.0),
            &[2.0, 2.0, 1.0],
            0.0,
        );
    }

    #[test]
    fn test_families() {
        assert_eq!(Centrality::ALL[0], Centrality::InDegree);
        for c in Centrality::ALL {
            let axis = Ranking::new(&example_graph(), c).axis;
            let expected = match axis {
                Axis::Flat => Family::Geometric,
                Axis::Iteration(_) => Family::Eigenvector,
                _ => Family::DampedSum,
            };
            assert_eq!(c.family(), expected, "{c:?}");
        }
        assert_eq!(Centrality::InDegree.name(true), "Indegree");
        assert_eq!(Centrality::InDegree.name(false), "Degree");
        assert_eq!(Centrality::Katz.name(false), "Katz's Index");
    }

    #[test]
    fn test_traditional_eigenvector() {
        let r = traditional(&star_graph(), Centrality::Eigenvector);
        assert_eq!(r.norm, Norm::L2);
        // The second iterate is uniform
        assert_close(&r.scores(2.0), &[0.5; 4], 1e-12);
        assert_close(&r.shares(2.0), &[0.25; 4], 1e-12);
    }

    #[test]
    fn test_traditional_distributions() {
        for c in [Centrality::Seeley, Centrality::PageRank] {
            let (r, t) = (
                Ranking::new(&example_graph(), c),
                traditional(&example_graph(), c),
            );
            assert_eq!(t.norm, Norm::L1);
            assert_eq!(r.curves, t.curves);
        }
    }

    #[test]
    fn test_traditional_katz() {
        let r = traditional(&example_graph(), Centrality::Katz);
        assert_eq!(r.norm, Norm::None);
        assert!(r.unbounded);
        // The right end is not sampled
        assert_eq!(r.xs.len(), SAMPLES);
        assert_close(&r.scores(0.0), &[1.0; 5], 1e-15);
        let succ = example_graph().successors();
        let s = Spectrum::new(&succ);
        assert_close(&r.scores(0.85), &katz(&succ, 0.85 / s.rho()), 1e-12);
        assert!(r.scores(1.0).iter().all(|x| x.is_infinite()));
        assert_close(&r.shares(1.0), &s.katz_limit(), 1e-15);
        // With ρ = 0, the scores are bounded: on a → b → c at β = 1, 1, 2,
        // and 3
        let r = traditional(&path_graph(), Centrality::Katz);
        assert!(!r.unbounded);
        assert_close(&r.scores(1.0), &[1.0, 2.0, 3.0], 1e-12);
    }

    #[test]
    fn test_at() {
        let r = Ranking::new(&example_graph(), Centrality::PageRank);
        let frozen = r.at(0.85);
        assert_eq!(frozen.axis, Axis::Flat);
        assert_eq!(frozen.frozen, Some((Axis::Alpha, 0.85)));
        assert_eq!(frozen.centrality, Centrality::PageRank);
        for x in [0.0, 0.5, 1.0] {
            assert_eq!(frozen.scores(x), r.scores(0.85));
        }
        // Past the vanishing of the iterates, the scores are zero
        let r = Ranking::new(&path_graph(), Centrality::Eigenvector);
        assert_eq!(r.at(10.0).scores(0.0), [0.0; 3]);
    }

    #[test]
    fn test_iterations_option() {
        let options = Options {
            iterations: 7,
            ..Options::default()
        };
        let r = Ranking::with_options(&example_graph(), Centrality::Seeley, options);
        assert_eq!((r.axis, r.xs.len()), (Axis::Iteration(7), 8));
        assert_eq!(r.axis.x_max(), 7.0);
        // The iterates of a → b → c vanish at k = 3, whatever K ≥ 3
        let r = Ranking::with_options(&path_graph(), Centrality::Eigenvector, options);
        assert_eq!((r.xs.len(), r.vanished), (3, Some(3)));
    }

    #[test]
    fn test_mean_option() {
        let options = Options {
            iterations: MAX_ITERATIONS,
            mean: true,
            ..Options::default()
        };
        // On the star, the means converge to eigenvector centrality
        let r = Ranking::with_options(&star_graph(), Centrality::Eigenvector, options);
        let scores = r.scores(MAX_ITERATIONS as f64);
        assert!((scores[0] / scores[1] - 3.0_f64.sqrt()).abs() < 1e-2);
        // With ρ = 0, the curves show the iterates, which vanish
        let (r, t) = (
            Ranking::with_options(&path_graph(), Centrality::Eigenvector, options),
            Ranking::new(&path_graph(), Centrality::Eigenvector),
        );
        assert_eq!((r.curves, r.vanished), (t.curves, t.vanished));
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
