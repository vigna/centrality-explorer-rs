/*
 * SPDX-FileCopyrightText: 2026 Sebastiano Vigna
 *
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 */

//! Iterates of the power method computing eigenvector centrality and
//! Seeley's index.
//!
//! The centralities are dominant eigenvectors, and the power method is just
//! one way of computing them: its iterates may vanish, or fail to converge,
//! even though the eigenvector exists. Their means, however, converge to the
//! eigenvector whenever ρ > 0 (in the sense of Cesàro).
//!
//! Iterates are row vectors, so mass flows along arcs from a node to its
//! successors.

use super::Chain;

/// Returns the first `iterations` + 1 iterates of the power method on the
/// transition matrix *P* of a chain.
///
/// The first iterate is the uniform vector **x**₀, and **x**<sub>*k* + 1</sub>
/// = **x**<sub>*k*</sub> *P*. Every iterate sums to 1. When the chain has no
/// nodes, the result is a single empty vector.
pub fn seeley_iterates(chain: &Chain, iterations: usize) -> Vec<Vec<f64>> {
    let n = chain.len();
    if n == 0 {
        return vec![vec![]];
    }
    let p = chain.transition();
    let mut result = Vec::with_capacity(iterations + 1);
    result.push(vec![1.0 / n as f64; n]);
    for _ in 0..iterations {
        let x = &result[result.len() - 1];
        let mut next = vec![0.0; n];
        for (i, row) in p.chunks_exact(n).enumerate() {
            for (y, &q) in next.iter_mut().zip(row) {
                *y += x[i] * q;
            }
        }
        result.push(next);
    }
    result
}

/// Returns the first iterates of the power method on the adjacency matrix *A*
/// of a graph given by successor lists, at most `iterations` + 1.
///
/// The first iterate is the uniform vector **x**₀, and **x**<sub>*k* + 1</sub>
/// = **x**<sub>*k*</sub> *A* / ‖**x**<sub>*k*</sub> *A*‖₁. The iteration stops
/// before the first *k* such that **x**<sub>*k*</sub> *A* = 0, so a result with
/// fewer than `iterations` + 1 vectors means that the iterates vanish at *k*
/// equal to its length. When the graph has no nodes, the result is a single
/// empty vector.
///
/// # Implementation Notes
///
/// All entries are nonnegative, so there is no cancellation, and
/// **x**<sub>*k*</sub> *A* = 0 is detected exactly by checking that the sum of
/// its entries is 0.0. The products are sparse.
pub fn eigenvector_iterates(succ: &[Vec<usize>], iterations: usize) -> Vec<Vec<f64>> {
    let n = succ.len();
    if n == 0 {
        return vec![vec![]];
    }
    let mut result = Vec::with_capacity(iterations + 1);
    result.push(vec![1.0 / n as f64; n]);
    for _ in 0..iterations {
        let x = &result[result.len() - 1];
        let mut next = vec![0.0; n];
        for (i, list) in succ.iter().enumerate() {
            for &j in list {
                next[j] += x[i];
            }
        }
        let sum: f64 = next.iter().sum();
        if sum == 0.0 {
            break;
        }
        for y in &mut next {
            *y /= sum;
        }
        result.push(next);
    }
    result
}

/// Returns the running means of a sequence of vectors: the *k*-th mean is
/// the mean of the first *k* + 1 vectors.
fn running_means(vectors: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let mut sum = vec![0.0; vectors.first().map_or(0, Vec::len)];
    vectors
        .iter()
        .enumerate()
        .map(|(k, v)| {
            for (s, x) in sum.iter_mut().zip(v) {
                *s += x;
            }
            sum.iter().map(|s| s / (k + 1) as f64).collect()
        })
        .collect()
}

/// Returns the means of the first `iterations` + 1 [iterates](seeley_iterates)
/// of the power method on the transition matrix *P* of a chain: the *k*-th
/// mean is (**x**₀ + **x**₁ + ⋯ + **x**<sub>*k*</sub>) / (*k* + 1).
///
/// Every mean sums to 1. Even when the iterates oscillate, the means converge
/// (by the mean ergodic theorem) to Seeley's index, the limit of PageRank as
/// α → 1, like 1/*k*. When the chain has no nodes, the result is a single
/// empty vector.
pub fn seeley_means(chain: &Chain, iterations: usize) -> Vec<Vec<f64>> {
    running_means(&seeley_iterates(chain, iterations))
}

/// Returns the means of the first `iterations` + 1 iterates of the power
/// method on *A* / ρ, where *A* is the adjacency matrix of a graph given by
/// successor lists and ρ > 0 is its spectral radius.
///
/// The iterates are **y**<sub>*k*</sub> = **1** *A*<sup>*k*</sup> / (*n*
/// ρ<sup>*k*</sup>), and the *k*-th mean is (**y**₀ + **y**₁ + ⋯ +
/// **y**<sub>*k*</sub>) / (*k* + 1). Even when the iterates oscillate, the
/// normalized means converge to eigenvector centrality, the limit of Katz's
/// index as βρ → 1, like 1/*k*: the latter is the Abel mean of the iterates,
/// which equals their Cesàro mean whenever the latter exists. When ν > 1 (see
/// [`Spectrum::katz_limit`](super::Spectrum::katz_limit)), the means diverge
/// like *k*<sup>ν − 1</sup>, but their normalizations still converge.
///
/// The iterates are scaled by ρ<sup>*k*</sup>, rather than normalized as in
/// [`eigenvector_iterates`], because on periodic structures the means of the
/// normalized iterates converge to a vector that is not an eigenvector: on an
/// undirected star they converge to ⅜ on the center and to ⅝ split evenly
/// among the leaves, whereas eigenvector centrality gives the center √3 times
/// the score of a leaf, that is, (√3 − 1)/2 ≈ 0.366.
pub fn eigenvector_means(succ: &[Vec<usize>], rho: f64, iterations: usize) -> Vec<Vec<f64>> {
    debug_assert!(rho > 0.0, "the means require ρ > 0");
    let n = succ.len();
    let mut iterates = Vec::with_capacity(iterations + 1);
    iterates.push(vec![1.0 / n as f64; n]);
    for _ in 0..iterations {
        let y = &iterates[iterates.len() - 1];
        let mut next = vec![0.0; n];
        for (i, list) in succ.iter().enumerate() {
            for &j in list {
                next[j] += y[i] / rho;
            }
        }
        iterates.push(next);
    }
    running_means(&iterates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rank::tests::assert_close;
    use crate::rank::{Spectrum, limit, normalized};

    #[test]
    fn test_seeley_oscillates() {
        // a ⇄ b, c → a
        let it = seeley_iterates(&Chain::new(vec![vec![1], vec![0], vec![0]]), 4);
        assert_eq!(it.len(), 5);
        assert_close(&it[1], &[2.0 / 3.0, 1.0 / 3.0, 0.0], 1e-15);
        assert_close(&it[2], &[1.0 / 3.0, 2.0 / 3.0, 0.0], 1e-15);
        assert_close(&it[3], &it[1], 1e-15);
    }

    #[test]
    fn test_seeley_converges_to_pagerank_limit() {
        // Aperiodic: example_after with a self-loop on e, and the path with
        // a dangling end
        for chain in [
            Chain::new(vec![vec![1], vec![2], vec![0, 3], vec![4], vec![3, 4]]),
            Chain::new(vec![vec![1], vec![2], vec![]]),
        ] {
            let it = seeley_iterates(&chain, 2000);
            assert_close(it.last().unwrap(), &limit(&chain), 1e-9);
            assert!(
                it.iter()
                    .all(|x| (x.iter().sum::<f64>() - 1.0).abs() < 1e-12)
            );
        }
    }

    #[test]
    fn test_eigenvector_vanishes_on_dag() {
        // a → b → c
        let it = eigenvector_iterates(&[vec![1], vec![2], vec![]], 50);
        assert_eq!(it.len(), 3); // vanishes at k = 3
        assert_close(&it[1], &[0.0, 0.5, 0.5], 1e-15);
        assert_close(&it[2], &[0.0, 0.0, 1.0], 1e-15);
        assert_eq!(eigenvector_iterates(&[vec![], vec![]], 50).len(), 1); // no arcs
    }

    #[test]
    fn test_eigenvector_star_oscillates() {
        // Undirected star, center 0
        let it = eigenvector_iterates(&[vec![1, 2, 3], vec![0], vec![0], vec![0]], 4);
        assert_close(&it[1], &[0.5, 1.0 / 6.0, 1.0 / 6.0, 1.0 / 6.0], 1e-15);
        assert_close(&it[2], &[0.25; 4], 1e-15);
    }

    #[test]
    fn test_eigenvector_slow_convergence() {
        // a (loop) → b (loop): 1·Aᵏ = (1, k + 1)
        let it = eigenvector_iterates(&[vec![0, 1], vec![1]], 50);
        assert!((it[50][0] - 1.0 / 52.0).abs() < 1e-15);
    }

    #[test]
    fn test_eigenvector_converges_to_katz_limit() {
        // ν = 1, aperiodic: triangle with a self-loop feeding a 2-cycle;
        // a ⇄ b with b → b
        for succ in [
            vec![vec![0, 1], vec![2], vec![0, 3], vec![4], vec![3]],
            vec![vec![1], vec![0, 1]],
        ] {
            let it = eigenvector_iterates(&succ, 2000);
            assert_close(it.last().unwrap(), &Spectrum::new(&succ).katz_limit(), 1e-9);
        }
    }

    #[test]
    fn test_seeley_means_converge_on_periodic() {
        // a ⇄ b, c → a: the iterates oscillate, the means converge
        let chain = Chain::new(vec![vec![1], vec![0], vec![0]]);
        let means = seeley_means(&chain, 2000);
        assert_eq!(means.len(), 2001);
        assert_close(&means[0], &[1.0 / 3.0; 3], 1e-15);
        assert_close(means.last().unwrap(), &limit(&chain), 1e-3);
        assert!(
            means
                .iter()
                .all(|x| (x.iter().sum::<f64>() - 1.0).abs() < 1e-12)
        );
    }

    #[test]
    fn test_eigenvector_means_star() {
        // Undirected star, center 0: the means of the normalized iterates
        // would tend to (3/8, 5/24, 5/24, 5/24)
        let star = [vec![1, 2, 3], vec![0], vec![0], vec![0]];
        let rho = 3.0_f64.sqrt();
        let means = eigenvector_means(&star, rho, 2000);
        let x = normalized(means.last().unwrap().clone());
        let leaf = 1.0 / (3.0 + rho);
        assert_close(&x, &[rho * leaf, leaf, leaf, leaf], 1e-3);
        // The first mean is the first iterate; then, (1 + A/ρ) / 2
        assert_close(&means[0], &[0.25; 4], 1e-15);
        let second = (1.0 + 1.0 / rho) / 8.0;
        assert_close(
            &means[1],
            &[(1.0 + rho) / 8.0, second, second, second],
            1e-15,
        );
    }

    #[test]
    fn test_eigenvector_means_converge_to_katz_limit() {
        // Periodic, ν = 2: a ⇄ b → c ⇄ d; periodic, ν = 1: c → a ⇄ b
        for succ in [
            vec![vec![1], vec![0, 2], vec![3], vec![2]],
            vec![vec![1], vec![0], vec![0]],
        ] {
            let s = Spectrum::new(&succ);
            let means = eigenvector_means(&succ, s.rho(), 2000);
            // The error decreases like 1/k
            assert_close(
                &normalized(means.last().unwrap().clone()),
                &s.katz_limit(),
                5.0 / 2000.0,
            );
        }
    }
}
