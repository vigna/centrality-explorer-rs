//! Exact PageRank.
//!
//! PageRank is computed for a [`Chain`], the transition structure of a graph
//! in which dangling nodes jump uniformly to every node; the preference
//! vector is uniform.

mod lu;

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
/// The result solves (*I* − α *P*ᵀ) *x* = (1 − α) *v*, where *v* is the
/// uniform distribution, and sums to one.
pub fn pagerank(chain: &Chain, alpha: f64) -> Vec<f64> {
    debug_assert!((0.0..1.0).contains(&alpha), "alpha must be in [0, 1)");
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
}
