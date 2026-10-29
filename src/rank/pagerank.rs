//! Exact PageRank.
//!
//! PageRank is computed for a [`Chain`], the transition structure of a graph
//! in which dangling nodes jump uniformly to every node; the preference
//! vector is uniform.

use super::{lu, terminal_sccs};
use crate::graph::Graph;

/// The transition structure of a graph.
///
/// A node with *d* > 0 successors moves to each of them with probability
/// 1/*d*; a dangling node (one with no successors) moves to every node with
/// probability 1/*n*.
#[derive(Clone, Debug, PartialEq)]
pub struct Chain {
    succ: Vec<Vec<usize>>,
}

impl Chain {
    /// Creates a chain from successor lists by node index; an empty list
    /// denotes a dangling node.
    pub fn new(succ: Vec<Vec<usize>>) -> Self {
        Self { succ }
    }

    /// Creates the chain of a graph.
    pub fn from_graph(graph: &Graph) -> Self {
        Self::new(graph.successors())
    }

    /// Returns the number of nodes.
    pub fn len(&self) -> usize {
        self.succ.len()
    }

    /// Returns whether the chain has no nodes.
    pub fn is_empty(&self) -> bool {
        self.succ.is_empty()
    }

    /// Returns whether node `i` is dangling.
    pub fn is_dangling(&self, i: usize) -> bool {
        self.succ[i].is_empty()
    }

    /// Returns the successor lists.
    pub fn successors(&self) -> &[Vec<usize>] {
        &self.succ
    }

    /// Returns the dense row-major transition matrix *P*, with uniform rows
    /// for dangling nodes.
    pub(crate) fn transition(&self) -> Vec<f64> {
        let n = self.len();
        let mut p = vec![0.0; n * n];
        for (i, succ) in self.succ.iter().enumerate() {
            let row = &mut p[i * n..(i + 1) * n];
            if succ.is_empty() {
                row.fill(1.0 / n as f64);
            } else {
                let w = 1.0 / succ.len() as f64;
                for &j in succ {
                    row[j] += w;
                }
            }
        }
        p
    }
}

/// Returns the PageRank vector of a chain with damping factor `alpha` and
/// uniform preference vector.
///
/// For α < 1 the result solves (*I* − α *P*ᵀ) **x** = (1 − α) **v**, where
/// **v** is the uniform distribution; for α = 1 it is the limit computed by
/// [`limit`]. It always sums to one.
pub fn pagerank(chain: &Chain, alpha: f64) -> Vec<f64> {
    debug_assert!((0.0..=1.0).contains(&alpha), "alpha must be in [0, 1]");
    if alpha >= 1.0 {
        return limit(chain);
    }
    let n = chain.len();
    let p = chain.transition();
    let mut a = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            a[i * n + j] = -alpha * p[j * n + i];
        }
        a[i * n + i] += 1.0;
    }
    let mut x = vec![(1.0 - alpha) / n as f64; n];
    lu::solve(&mut a, &mut x);
    x
}

/// Returns the terminal strongly connected components of the chain actually
/// walked, in which dangling nodes link to every node.
///
/// These are the components on which the [`limit`] is supported. If the
/// graph has [sinks](super::Ranking::hulls), they are exactly the sinks;
/// otherwise, the walk restarts from the dangling nodes, and the whole graph
/// is a single terminal component.
///
/// Each component is sorted, and components are sorted by their smallest
/// node.
pub fn terminal_components(chain: &Chain) -> Vec<Vec<usize>> {
    let n = chain.len();
    let patched: Vec<Vec<usize>> = (0..n)
        .map(|i| {
            if chain.is_dangling(i) {
                (0..n).collect()
            } else {
                chain.succ[i].clone()
            }
        })
        .collect();
    terminal_sccs(&patched)
}

/// Returns the limit of the PageRank vector as α → 1.
///
/// The limit is Σ_C *a*_C **π**_C, where C ranges over the
/// [terminal components](terminal_components), **π**_C is the stationary
/// distribution of the chain restricted to C, and *a*_C is the probability
/// that a walk starting from the uniform distribution is eventually absorbed
/// into C. It is supported exactly on the terminal components.
pub fn limit(chain: &Chain) -> Vec<f64> {
    let n = chain.len();
    let p = chain.transition();
    let terminal = terminal_components(chain);
    let mut in_terminal = vec![false; n];
    for component in &terminal {
        for &v in component {
            in_terminal[v] = true;
        }
    }
    let transient: Vec<usize> = (0..n).filter(|&v| !in_terminal[v]).collect();
    let t = transient.len();
    // I − P_TT, the matrix of the absorption systems.
    let mut absorption = vec![0.0; t * t];
    for (i, &u) in transient.iter().enumerate() {
        for (j, &w) in transient.iter().enumerate() {
            absorption[i * t + j] = -p[u * n + w];
        }
        absorption[i * t + i] += 1.0;
    }

    let mut result = vec![0.0; n];
    for component in &terminal {
        // Absorption probabilities from the transient nodes into C.
        let mut h: Vec<f64> = transient
            .iter()
            .map(|&u| component.iter().map(|&w| p[u * n + w]).sum())
            .collect();
        lu::solve(&mut absorption.clone(), &mut h);
        let weight = (component.len() as f64 + h.iter().sum::<f64>()) / n as f64;

        // Stationary distribution of C: (I − P_CC)ᵀ π = 0, with the last
        // equation replaced by Σ π = 1.
        let k = component.len();
        let mut a = vec![0.0; k * k];
        for (i, &u) in component.iter().enumerate() {
            for (j, &w) in component.iter().enumerate() {
                a[i * k + j] = -p[w * n + u];
            }
            a[i * k + i] += 1.0;
        }
        a[(k - 1) * k..].fill(1.0);
        let mut pi = vec![0.0; k];
        pi[k - 1] = 1.0;
        lu::solve(&mut a, &mut pi);

        for (&v, &x) in component.iter().zip(&pi) {
            result[v] = weight * x;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rank::sccs;
    use crate::rank::tests::{assert_close, example_after, example_before};

    #[test]
    fn test_pagerank_known_values() {
        assert_close(
            &pagerank(&example_after(), 0.85),
            &[
                0.07733381437719854,
                0.09573374222061874,
                0.11137368088752596,
                0.37057230406197667,
                0.34498645845268017,
            ],
            1e-10,
        );
        assert_close(
            &pagerank(&example_before(), 0.85),
            &[
                0.16903241069316305,
                0.20925005914360068,
                0.24343506032647272,
                0.16903241069316302,
                0.20925005914360073,
            ],
            1e-10,
        );
    }

    #[test]
    fn test_pagerank_sums_to_one() {
        for chain in [
            example_after(),
            example_before(),
            Chain::new(vec![vec![1, 2, 3], vec![], vec![], vec![0]]),
        ] {
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

    #[test]
    fn test_sccs() {
        assert_eq!(
            sccs(example_after().successors()),
            vec![vec![3, 4], vec![0, 1, 2]]
        );
    }

    #[test]
    fn test_terminal_components() {
        assert_eq!(terminal_components(&example_after()), vec![vec![3, 4]]);
        // e is dangling, hence linked to everybody: one big terminal component
        assert_eq!(
            terminal_components(&example_before()),
            vec![vec![0, 1, 2, 3, 4]]
        );
    }

    #[test]
    fn test_terminal_sccs() {
        let chain = Chain::new(vec![vec![1], vec![2], vec![]]); // a → b → c
        assert_eq!(terminal_sccs(chain.successors()), vec![vec![2]]);
        assert_eq!(terminal_sccs(example_before().successors()), vec![vec![4]]);
        assert_eq!(
            terminal_sccs(example_after().successors()),
            vec![vec![3, 4]]
        );
        let path = Chain::new(vec![vec![1], vec![0, 2], vec![1, 3], vec![2]]);
        assert_eq!(terminal_sccs(path.successors()), vec![vec![0, 1, 2, 3]]);
        let two_sinks = Chain::new(vec![vec![1, 2], vec![], vec![2]]);
        assert_eq!(
            terminal_sccs(two_sinks.successors()),
            vec![vec![1], vec![2]]
        );
    }

    #[test]
    fn test_terminal_self_loop() {
        assert_eq!(
            terminal_components(&Chain::new(vec![vec![0], vec![0]])),
            vec![vec![0]]
        );
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
            // periodic + dangling
            Chain::new(vec![vec![1], vec![0], vec![0, 3], vec![]]),
            // undirected path
            Chain::new(vec![vec![1], vec![0, 2], vec![1, 3], vec![2]]),
            // self-loop terminal
            Chain::new(vec![vec![1, 2], vec![1], vec![0]]),
            // two closed cycles
            Chain::new(vec![
                vec![1],
                vec![2],
                vec![0],
                vec![4],
                vec![5],
                vec![3],
                vec![0, 3],
            ]),
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
}
