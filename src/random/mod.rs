//! Random graphs with power-law degrees.

use rand::rngs::SmallRng;
use rand::{RngExt, SeedableRng};

use crate::graph::Graph;

/// Parameters of the directed Chung–Lu model, whose in-degrees and
/// out-degrees follow a power law.
///
/// Each node gets an out-weight and an in-weight drawn independently from a
/// power law with the given exponent; both weight vectors are scaled to sum
/// to *W* = `nodes` · `avg_degree`, and each arc *u* → *v* with *u* ≠ *v* is
/// present independently with probability
/// min(1, *w*<sub>out</sub>(*u*) *w*<sub>in</sub>(*v*) / *W*).
///
/// Sparse graphs from this model often have no closed component containing
/// a cycle; this is expected, and the instructor adds the missing structure
/// by hand.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChungLu {
    /// The number of nodes.
    pub nodes: usize,
    /// The expected average degree.
    pub avg_degree: f64,
    /// The exponent of the power law of the weights.
    pub exponent: f64,
}

impl ChungLu {
    /// Generates a directed graph using the given seed.
    ///
    /// Nodes are labeled a, b, c, … and all lie at the origin, so the
    /// caller is expected to lay them out. The same seed yields the same
    /// graph within the same build.
    pub fn generate(&self, seed: u64) -> Graph {
        let n = self.nodes;
        let mut rng = SmallRng::seed_from_u64(seed);
        let total = n as f64 * self.avg_degree;
        let weights = |rng: &mut SmallRng| {
            let mut w: Vec<f64> = (0..n)
                .map(|_| (1.0 - rng.random::<f64>()).powf(-1.0 / (self.exponent - 1.0)))
                .collect();
            let sum: f64 = w.iter().sum();
            w.iter_mut().for_each(|x| *x *= total / sum);
            w
        };
        let w_out = weights(&mut rng);
        let w_in = weights(&mut rng);

        let mut graph = Graph::new(true);
        let ids: Vec<_> = (0..n).map(|_| graph.add_node([0.0, 0.0])).collect();
        for u in 0..n {
            for v in 0..n {
                if u != v && rng.random::<f64>() < (w_out[u] * w_in[v] / total).min(1.0) {
                    graph.add_arc(ids[u], ids[v]);
                }
            }
        }
        graph.set_seed(seed);
        graph
    }
}

impl Default for ChungLu {
    fn default() -> Self {
        Self {
            nodes: 10,
            avg_degree: 2.0,
            exponent: 2.5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_seed_same_graph() {
        let p = ChungLu::default();
        let (g, h) = (p.generate(42), p.generate(42));
        assert_eq!(g.arcs().collect::<Vec<_>>(), h.arcs().collect::<Vec<_>>());
        assert_eq!(g.seed(), Some(42));
    }

    #[test]
    fn test_different_seeds_differ() {
        let p = ChungLu {
            nodes: 30,
            ..ChungLu::default()
        };
        assert_ne!(
            p.generate(1).arcs().collect::<Vec<_>>(),
            p.generate(2).arcs().collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_shape() {
        let g = ChungLu {
            nodes: 50,
            ..ChungLu::default()
        }
        .generate(7);
        assert_eq!(g.len(), 50);
        assert!(g.is_directed());
        assert!(g.arcs().all(|(u, v)| u != v));
        assert_eq!(g.nodes()[0].label, "a");
    }

    #[test]
    fn test_mean_degree() {
        let p = ChungLu {
            nodes: 300,
            avg_degree: 3.0,
            exponent: 3.0,
        };
        let mean = (0..5)
            .map(|s| p.generate(s).arcs().count() as f64 / 300.0)
            .sum::<f64>()
            / 5.0;
        assert!((2.7..=3.3).contains(&mean), "{mean}");
    }

    #[test]
    fn test_heavy_tail() {
        let p = ChungLu {
            nodes: 300,
            avg_degree: 2.0,
            exponent: 2.1,
        };
        let mean_max = (0..5)
            .map(|s| {
                let g = p.generate(s);
                let mut indeg = vec![0usize; g.len()];
                for (_, v) in g.arcs() {
                    indeg[g.index_of(v).unwrap()] += 1;
                }
                *indeg.iter().max().unwrap() as f64
            })
            .sum::<f64>()
            / 5.0;
        assert!(mean_max >= 8.0, "{mean_max}");
    }
}
