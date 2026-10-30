/*
 * SPDX-FileCopyrightText: 2026 Sebastiano Vigna
 *
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 */

//! Indegree, closeness, harmonic centrality, and betweenness.

use std::collections::VecDeque;

/// Runs a breadth-first visit from `source`, filling `dist` (which must be
/// all `usize::MAX` on entry) and returning the nodes in visit order.
fn bfs(succ: &[Vec<usize>], source: usize, dist: &mut [usize]) -> Vec<usize> {
    let mut order = vec![source];
    dist[source] = 0;
    let mut head = 0;
    while head < order.len() {
        let v = order[head];
        head += 1;
        for &w in &succ[v] {
            if dist[w] == usize::MAX {
                dist[w] = dist[v] + 1;
                order.push(w);
            }
        }
    }
    order
}

/// Returns the indegree of every node of a graph given by successor lists,
/// that is, the number of arcs entering it, a self-loop included.
///
/// This is **1** *A*, where *A* is the adjacency matrix: on undirected graphs,
/// whose edges are pairs of opposite arcs, it is the degree. Values are raw,
/// that is, unnormalized.
pub fn indegree(succ: &[Vec<usize>]) -> Vec<f64> {
    let mut d = vec![0.0; succ.len()];
    for &w in succ.iter().flatten() {
        d[w] += 1.0;
    }
    d
}

/// Returns the closeness of every node of a graph given by successor lists.
///
/// The closeness of *x* is 1 / Σ *d*(*y*, *x*), where the sum is over the
/// nodes *y* with 0 < *d*(*y*, *x*) < ∞, and 0 if no other node reaches *x*.
/// Values are raw, that is, unnormalized.
///
/// # Implementation Notes
///
/// One breadth-first visit per source *y* adds *d*(*y*, *x*) into the sum of
/// every reached node *x*. Self-loops do not affect distances.
pub fn closeness(succ: &[Vec<usize>]) -> Vec<f64> {
    let n = succ.len();
    let mut sum = vec![0_usize; n];
    let mut dist = vec![usize::MAX; n];
    for y in 0..n {
        dist.fill(usize::MAX);
        for x in bfs(succ, y, &mut dist) {
            sum[x] += dist[x];
        }
    }
    sum.into_iter()
        .map(|s| if s == 0 { 0.0 } else { 1.0 / s as f64 })
        .collect()
}

/// Returns the harmonic centrality of every node of a graph given by
/// successor lists.
///
/// The harmonic centrality of *x* is Σ<sub>*y* ≠ *x*</sub> 1 / *d*(*y*, *x*),
/// with 1/∞ = 0. Values are raw, that is, unnormalized.
///
/// # Implementation Notes
///
/// One breadth-first visit per source *y* adds 1 / *d*(*y*, *x*) into the
/// score of every reached node *x* ≠ *y*.
pub fn harmonic(succ: &[Vec<usize>]) -> Vec<f64> {
    let n = succ.len();
    let mut score = vec![0.0; n];
    let mut dist = vec![usize::MAX; n];
    for y in 0..n {
        dist.fill(usize::MAX);
        for x in bfs(succ, y, &mut dist) {
            if x != y {
                score[x] += 1.0 / dist[x] as f64;
            }
        }
    }
    score
}

/// Returns the betweenness of every node of a graph given by successor
/// lists.
///
/// The betweenness of *x* is Σ σ<sub>*yz*</sub>(*x*) / σ<sub>*yz*</sub>
/// over the ordered pairs of distinct nodes *y*, *z* different from *x*,
/// where σ<sub>*yz*</sub> is the number of shortest paths from *y* to *z*
/// (assumed positive) and σ<sub>*yz*</sub>(*x*) the number of those passing
/// through *x*. In an undirected graph, ordered pairs count each path
/// twice. Values are raw, that is, unnormalized.
///
/// # Implementation Notes
///
/// This is Brandes's algorithm: a breadth-first visit from each source
/// counts shortest paths, and dependencies are then accumulated in reverse
/// visit order. Self-loops never lie on a shortest path.
pub fn betweenness(succ: &[Vec<usize>]) -> Vec<f64> {
    let n = succ.len();
    let mut score = vec![0.0; n];
    let mut dist = vec![usize::MAX; n];
    let mut sigma = vec![0.0_f64; n];
    let mut delta = vec![0.0_f64; n];
    let mut queue = VecDeque::new();
    let mut order = Vec::with_capacity(n);
    for s in 0..n {
        dist.fill(usize::MAX);
        sigma.fill(0.0);
        delta.fill(0.0);
        order.clear();
        dist[s] = 0;
        sigma[s] = 1.0;
        queue.push_back(s);
        while let Some(v) = queue.pop_front() {
            order.push(v);
            for &w in &succ[v] {
                if dist[w] == usize::MAX {
                    dist[w] = dist[v] + 1;
                    queue.push_back(w);
                }
                if dist[w] == dist[v] + 1 {
                    sigma[w] += sigma[v];
                }
            }
        }
        for &w in order.iter().rev() {
            for &v in &succ[w] {
                if dist[v] == dist[w] + 1 {
                    delta[w] += sigma[w] / sigma[v] * (1.0 + delta[v]);
                }
            }
            if w != s {
                score[w] += delta[w];
            }
        }
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rank::tests::assert_close;

    #[test]
    fn test_indegree() {
        // a → b → c, a → c, and a self-loop on c
        let succ = [vec![1, 2], vec![2], vec![2]];
        assert_close(&indegree(&succ), &[0.0, 1.0, 3.0], 0.0);
        // The undirected star: the degree
        let succ = [vec![1, 2, 3], vec![0], vec![0], vec![0]];
        assert_close(&indegree(&succ), &[3.0, 1.0, 1.0, 1.0], 0.0);
    }

    #[test]
    fn test_directed_path() {
        // a → b → c
        let succ = vec![vec![1], vec![2], vec![]];
        assert_close(&closeness(&succ), &[0.0, 1.0, 1.0 / 3.0], 1e-12);
        assert_close(&harmonic(&succ), &[0.0, 1.0, 1.5], 1e-12);
        assert_close(&betweenness(&succ), &[0.0, 1.0, 0.0], 1e-12);
    }

    #[test]
    fn test_directed_cycle() {
        let succ = vec![vec![1], vec![2], vec![3], vec![0]];
        assert_close(&closeness(&succ), &[1.0 / 6.0; 4], 1e-12);
        assert_close(&harmonic(&succ), &[11.0 / 6.0; 4], 1e-12);
        assert_close(&betweenness(&succ), &[3.0; 4], 1e-12);
    }

    #[test]
    fn test_undirected_star() {
        let succ = vec![vec![1, 2, 3], vec![0], vec![0], vec![0]];
        assert_close(&closeness(&succ), &[1.0 / 3.0, 0.2, 0.2, 0.2], 1e-12);
        assert_close(&harmonic(&succ), &[3.0, 2.0, 2.0, 2.0], 1e-12);
        assert_close(&betweenness(&succ), &[6.0, 0.0, 0.0, 0.0], 1e-12);
    }

    #[test]
    fn test_not_strongly_connected() {
        // a → b, c → b, b → d
        let succ = vec![vec![1], vec![3], vec![1], vec![]];
        assert_close(&closeness(&succ), &[0.0, 0.5, 0.0, 0.2], 1e-12);
        assert_close(&harmonic(&succ), &[0.0, 2.0, 0.0, 2.0], 1e-12);
        assert_close(&betweenness(&succ), &[0.0, 2.0, 0.0, 0.0], 1e-12);
    }

    #[test]
    fn test_betweenness_splits_shortest_paths() {
        // diamond a → b, a → c, b → d, c → d
        let succ = vec![vec![1, 2], vec![3], vec![3], vec![]];
        assert_close(&betweenness(&succ), &[0.0, 0.5, 0.5, 0.0], 1e-12);
    }

    #[test]
    fn test_self_loops_ignored() {
        assert_close(
            &harmonic(&[vec![0, 1], vec![1]]),
            &harmonic(&[vec![1], vec![]]),
            1e-12,
        );
        assert_close(
            &betweenness(&[vec![0, 1], vec![1, 2], vec![2]]),
            &betweenness(&[vec![1], vec![2], vec![]]),
            1e-12,
        );
        assert!(closeness(&[]).is_empty());
    }
}
