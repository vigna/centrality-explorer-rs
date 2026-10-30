//! Node layouts.
//!
//! Every layout returns positions aligned with [`Graph::nodes`], in
//! canvas-local logical points, inside a box with a [`MARGIN`] on every side
//! of a canvas of the given size. Layouts are deterministic.

use std::collections::BTreeSet;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

use crate::graph::Graph;
use crate::rank::sccs;

/// The minimum distance between a node center and the canvas border.
pub const MARGIN: f32 = 60.0;

/// The minimum distance between node centers in the components layout,
/// before fitting to the canvas.
const NODE_SPACING: f32 = 120.0;

/// The number of iterations of the force-directed layout.
const FORCE_ITERATIONS: usize = 300;

/// The strength of the pull toward the center in the force-directed layout.
const GRAVITY: f32 = 0.05;

/// Places the nodes on a circle centered in the canvas, the first one at the
/// top, proceeding clockwise.
pub fn circle(graph: &Graph, size: [f32; 2]) -> Vec<[f32; 2]> {
    let n = graph.len();
    let [cx, cy] = [size[0] / 2.0, size[1] / 2.0];
    let radius = (0.5 * size[0].min(size[1]) - MARGIN).max(0.0);
    let positions = (0..n)
        .map(|i| {
            let angle = -FRAC_PI_2 + TAU * i as f32 / n as f32;
            [cx + radius * angle.cos(), cy + radius * angle.sin()]
        })
        .collect();
    clamp(positions, size)
}

/// Lays out the nodes with the Fruchterman–Reingold algorithm on the
/// underlying undirected graph, with a weak pull toward the center that keeps
/// disconnected parts together.
///
/// The layout starts from the current positions, or from [`circle`] if two
/// nodes share a position, and the result is scaled to fit the canvas.
pub fn force(graph: &Graph, size: [f32; 2]) -> Vec<[f32; 2]> {
    let n = graph.len();
    let mut pos: Vec<[f32; 2]> = graph.nodes().iter().map(|node| node.pos).collect();
    let coincide = (0..n).any(|i| (i + 1..n).any(|j| pos[i] == pos[j]));
    if coincide {
        pos = circle(graph, size);
    }
    let succ = graph.successors();
    let edges: Vec<(usize, usize)> = (0..n)
        .flat_map(|u| succ[u].iter().map(move |&v| (u.min(v), u.max(v))))
        .filter(|&(u, v)| u != v)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let [width, height] = [size[0] - 2.0 * MARGIN, size[1] - 2.0 * MARGIN];
    let k = (width.max(1.0) * height.max(1.0) / n.max(1) as f32).sqrt();
    let center = [size[0] / 2.0, size[1] / 2.0];
    let start_temperature = 0.1 * width.max(height).max(1.0);
    let mut disp = vec![[0.0f32; 2]; n];
    for iteration in 0..FORCE_ITERATIONS {
        disp.fill([0.0; 2]);
        for i in 0..n {
            for j in i + 1..n {
                let mut d = [pos[i][0] - pos[j][0], pos[i][1] - pos[j][1]];
                let mut dist = d[0].hypot(d[1]);
                if dist < 0.01 {
                    // Deterministic nudge for (almost) coincident nodes.
                    let angle = (i * n + j) as f32;
                    d = [angle.cos() * 0.01, angle.sin() * 0.01];
                    dist = 0.01;
                }
                let f = k * k / dist;
                for c in 0..2 {
                    disp[i][c] += d[c] / dist * f;
                    disp[j][c] -= d[c] / dist * f;
                }
            }
        }
        for &(u, v) in &edges {
            let d = [pos[u][0] - pos[v][0], pos[u][1] - pos[v][1]];
            let dist = d[0].hypot(d[1]).max(0.01);
            let f = dist * dist / k;
            for c in 0..2 {
                disp[u][c] -= d[c] / dist * f;
                disp[v][c] += d[c] / dist * f;
            }
        }
        for i in 0..n {
            for c in 0..2 {
                disp[i][c] += (center[c] - pos[i][c]) * GRAVITY;
            }
        }
        let temperature = start_temperature * (1.0 - iteration as f32 / FORCE_ITERATIONS as f32);
        for i in 0..n {
            let len = disp[i][0].hypot(disp[i][1]);
            if len > 0.0 {
                let step = len.min(temperature);
                for c in 0..2 {
                    pos[i][c] += disp[i][c] / len * step;
                }
            }
        }
    }
    fit(pos, size)
}

/// Lays out the strongly connected components left to right, with the
/// terminal components (those with no arc leaving them, including dangling
/// nodes) in the rightmost column.
///
/// The other components are placed in columns given by their longest path
/// from a source in the condensation of the graph. The nodes of a component
/// lie on a small circle.
pub fn components(graph: &Graph, size: [f32; 2]) -> Vec<[f32; 2]> {
    let n = graph.len();
    let succ = graph.successors();
    let comps = sccs(&succ);
    let mut comp_of = vec![0; n];
    for (c, comp) in comps.iter().enumerate() {
        for &v in comp {
            comp_of[v] = c;
        }
    }
    // sccs returns components in reverse topological order.
    let mut col = vec![0usize; comps.len()];
    for c in (0..comps.len()).rev() {
        for &v in &comps[c] {
            for &w in &succ[v] {
                let d = comp_of[w];
                if d != c {
                    col[d] = col[d].max(col[c] + 1);
                }
            }
        }
    }

    // Terminal components (no arc leaving them, including dangling nodes)
    // go in the rightmost column.
    let is_terminal = |c: usize| {
        comps[c]
            .iter()
            .all(|&v| succ[v].iter().all(|&w| comp_of[w] == c))
    };
    let last = (0..comps.len())
        .filter(|&c| !is_terminal(c))
        .map(|c| col[c] + 1)
        .max()
        .unwrap_or(0);
    for (c, column) in col.iter_mut().enumerate() {
        if is_terminal(c) {
            *column = last;
        }
    }

    // Components by column, each column ordered by smallest node.
    let columns = col.iter().max().map_or(0, |&c| c + 1);
    let mut by_column: Vec<Vec<usize>> = vec![Vec::new(); columns];
    for c in 0..comps.len() {
        by_column[col[c]].push(c);
    }
    // Lay out in abstract units, with node centers at least NODE_SPACING
    // apart, then fit to the canvas.
    let radius = |c: usize| {
        let k = comps[c].len();
        if k == 1 {
            0.0
        } else {
            NODE_SPACING / (2.0 * (PI / k as f32).sin())
        }
    };
    let mut pos = vec![[0.0f32; 2]; n];
    let mut x = 0.0;
    for column in by_column.iter_mut() {
        column.sort_unstable_by_key(|&c| comps[c][0]);
        let width = column.iter().map(|&c| 2.0 * radius(c)).fold(0.0, f32::max);
        let cx = x + width / 2.0;
        let height: f32 = column.iter().map(|&c| 2.0 * radius(c)).sum::<f32>()
            + NODE_SPACING * (column.len() as f32 - 1.0);
        let mut y = -height / 2.0;
        for &c in column.iter() {
            let r = radius(c);
            let k = comps[c].len();
            for (i, &v) in comps[c].iter().enumerate() {
                let angle = -FRAC_PI_2 + TAU * i as f32 / k as f32;
                pos[v] = [cx + r * angle.cos(), y + r + r * angle.sin()];
            }
            y += 2.0 * r + NODE_SPACING;
        }
        x += width + NODE_SPACING;
    }
    fit(pos, size)
}

/// Rotates the current positions by 90° clockwise on the screen, about the
/// center of their bounding box.
///
/// The result is translated as little as possible so that it lies within the
/// margins, or, if it does not fit, scaled down to fit and centered.
pub fn rotate(graph: &Graph, size: [f32; 2]) -> Vec<[f32; 2]> {
    // The y-axis points down, so clockwise takes right to down.
    transform(graph, size, |[dx, dy]| [-dy, dx])
}

/// Flips the current positions horizontally, that is, left to right, about
/// the center of their bounding box, then contains them in the canvas as
/// [`rotate`] does.
pub fn flip_horizontally(graph: &Graph, size: [f32; 2]) -> Vec<[f32; 2]> {
    transform(graph, size, |[dx, dy]| [-dx, dy])
}

/// Flips the current positions vertically, that is, top to bottom, about the
/// center of their bounding box, then contains them in the canvas as
/// [`rotate`] does.
pub fn flip_vertically(graph: &Graph, size: [f32; 2]) -> Vec<[f32; 2]> {
    transform(graph, size, |[dx, dy]| [dx, -dy])
}

/// Applies a map to the offsets of the current positions from the center of
/// their bounding box, and [contains](contain) the result in the canvas.
fn transform(graph: &Graph, size: [f32; 2], map: impl Fn([f32; 2]) -> [f32; 2]) -> Vec<[f32; 2]> {
    let pos: Vec<[f32; 2]> = graph.nodes().iter().map(|node| node.pos).collect();
    let Some((min, max)) = bounds(&pos) else {
        return pos;
    };
    let center = [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0];
    let pos = pos
        .into_iter()
        .map(|p| {
            let [dx, dy] = map([p[0] - center[0], p[1] - center[1]]);
            [center[0] + dx, center[1] + dy]
        })
        .collect();
    contain(pos, size)
}

/// Returns the corners of the bounding box of a set of positions, if any.
fn bounds(pos: &[[f32; 2]]) -> Option<([f32; 2], [f32; 2])> {
    if pos.is_empty() {
        return None;
    }
    let (mut min, mut max) = ([f32::MAX; 2], [f32::MIN; 2]);
    for p in pos {
        for c in 0..2 {
            min[c] = min[c].min(p[c]);
            max[c] = max[c].max(p[c]);
        }
    }
    Some((min, max))
}

/// Translates positions as little as possible so that they lie within the
/// margins, or, if their bounding box does not fit, [fits](fit) them.
fn contain(mut pos: Vec<[f32; 2]>, size: [f32; 2]) -> Vec<[f32; 2]> {
    let Some((min, max)) = bounds(&pos) else {
        return pos;
    };
    if (0..2).any(|c| max[c] - min[c] > size[c] - 2.0 * MARGIN) {
        return fit(pos, size);
    }
    let shift: Vec<f32> = (0..2)
        .map(|c| (MARGIN - min[c]).max(0.0) + (size[c] - MARGIN - max[c]).min(0.0))
        .collect();
    for p in &mut pos {
        for c in 0..2 {
            p[c] += shift[c];
        }
    }
    clamp(pos, size)
}

/// Scales and translates positions, preserving proportions, so that their
/// bounding box is centered in the canvas and fits within the margins.
fn fit(mut pos: Vec<[f32; 2]>, size: [f32; 2]) -> Vec<[f32; 2]> {
    let Some((min, max)) = bounds(&pos) else {
        return pos;
    };
    let extent = [max[0] - min[0], max[1] - min[1]];
    let room = [
        (size[0] - 2.0 * MARGIN).max(0.0),
        (size[1] - 2.0 * MARGIN).max(0.0),
    ];
    let scale = (0..2)
        .filter(|&c| extent[c] > 0.0)
        .map(|c| room[c] / extent[c])
        .fold(f32::INFINITY, f32::min);
    let scale = if scale.is_finite() { scale } else { 0.0 };
    for p in &mut pos {
        for c in 0..2 {
            p[c] = size[c] / 2.0 + (p[c] - (min[c] + max[c]) / 2.0) * scale;
        }
    }
    clamp(pos, size)
}

/// Clamps positions within the margins (absorbing rounding errors).
fn clamp(mut pos: Vec<[f32; 2]>, size: [f32; 2]) -> Vec<[f32; 2]> {
    for p in &mut pos {
        for c in 0..2 {
            let hi = (size[c] - MARGIN).max(MARGIN);
            p[c] = p[c].clamp(MARGIN, hi);
        }
    }
    pos
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::NodeId;

    fn graph(n: usize, arcs: &[(usize, usize)]) -> (Graph, Vec<NodeId>) {
        let mut g = Graph::new(true);
        let ids: Vec<_> = (0..n)
            .map(|i| g.add_node([10.0 * i as f32, 5.0 * (i % 3) as f32]))
            .collect();
        for &(s, t) in arcs {
            g.add_arc(ids[s], ids[t]);
        }
        (g, ids)
    }

    const SIZE: [f32; 2] = [800.0, 600.0];

    fn inside(p: &[[f32; 2]]) -> bool {
        p.iter().all(|&[x, y]| {
            (MARGIN..=SIZE[0] - MARGIN).contains(&x) && (MARGIN..=SIZE[1] - MARGIN).contains(&y)
        })
    }

    #[test]
    fn test_layouts_inside_and_deterministic() {
        let (g, _) = graph(5, &[(0, 1), (1, 2), (2, 0), (2, 3), (3, 4), (4, 3)]);
        for layout in [circle, force, components] {
            let p = layout(&g, SIZE);
            assert_eq!(p.len(), 5);
            assert!(inside(&p), "{p:?}");
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
        let d =
            |i: usize, j: usize| ((p[i][0] - p[j][0]).powi(2) + (p[i][1] - p[j][1]).powi(2)).sqrt();
        assert!(d(0, 1) < d(0, 3) && d(3, 4) < d(2, 5), "{p:?}");
    }

    #[test]
    fn test_components_terminal_right() {
        let (g, _) = graph(5, &[(0, 1), (1, 2), (2, 0), (2, 3), (3, 4), (4, 3)]);
        let p = components(&g, SIZE);
        assert!(
            (0..3).all(|i| p[i][0] < p[3][0] && p[i][0] < p[4][0]),
            "{p:?}"
        );
    }

    #[test]
    fn test_components_terminal_column() {
        // a→b→c (c dangling, hence terminal), a→d, d↔e closed
        let (g, _) = graph(5, &[(0, 1), (1, 2), (0, 3), (3, 4), (4, 3)]);
        let p = components(&g, SIZE);
        assert!(p[1][0] < p[2][0], "{p:?}");
        assert!((p[2][0] - p[3][0]).abs() < 1e-3, "{p:?}");
        assert!((p[3][0] - p[4][0]).abs() < 1e-3, "{p:?}");
    }

    #[test]
    fn test_components_spacing() {
        let (g, _) = graph(5, &[(0, 1), (1, 2), (2, 0), (2, 3), (3, 4), (4, 3)]);
        let p = components(&g, SIZE);
        for i in 0..5 {
            for j in i + 1..5 {
                let d = (p[i][0] - p[j][0]).hypot(p[i][1] - p[j][1]);
                assert!(d >= 100.0, "nodes {i} and {j} are {d} apart: {p:?}");
            }
        }
    }

    /// Returns a graph whose nodes are at the given positions.
    fn at(pos: &[[f32; 2]]) -> Graph {
        let mut g = Graph::new(true);
        for &p in pos {
            g.add_node(p);
        }
        g
    }

    #[test]
    fn test_rotate() {
        // Center (200, 200): left goes to top, right to bottom, top to right,
        // and bottom to left
        let p = rotate(
            &at(&[
                [100.0, 200.0],
                [300.0, 200.0],
                [200.0, 150.0],
                [200.0, 250.0],
            ]),
            SIZE,
        );
        assert_eq!(
            p,
            [
                [200.0, 100.0],
                [200.0, 300.0],
                [250.0, 200.0],
                [150.0, 200.0]
            ]
        );
        // Four rotations are the identity
        let start = [[100.0, 120.0], [400.0, 200.0], [250.0, 330.0]];
        let mut g = at(&start);
        for _ in 0..4 {
            let p = rotate(&g, SIZE);
            assert!(inside(&p), "{p:?}");
            g.set_positions(&p);
        }
        let p: Vec<_> = g.nodes().iter().map(|node| node.pos).collect();
        assert_eq!(p, start);
    }

    #[test]
    fn test_rotate_contains() {
        // Rotating a wide graph near the border moves it inside the margins
        let p = rotate(&at(&[[100.0, 80.0], [400.0, 80.0]]), SIZE);
        assert!(inside(&p), "{p:?}");
        assert_eq!(p[1][1] - p[0][1], 300.0);
        // A graph that does not fit once rotated is scaled down
        let p = rotate(&at(&[[MARGIN, 300.0], [SIZE[0] - MARGIN, 300.0]]), SIZE);
        assert!(inside(&p), "{p:?}");
        assert_eq!(p[0][0], p[1][0]);
        assert!(p[1][1] - p[0][1] < SIZE[1], "{p:?}");
    }

    #[test]
    fn test_flips() {
        let start = [[100.0, 120.0], [400.0, 200.0], [250.0, 330.0]];
        let h = flip_horizontally(&at(&start), SIZE);
        assert_eq!(h, [[400.0, 120.0], [100.0, 200.0], [250.0, 330.0]]);
        let v = flip_vertically(&at(&start), SIZE);
        assert_eq!(v, [[100.0, 330.0], [400.0, 250.0], [250.0, 120.0]]);
        assert_eq!(flip_horizontally(&at(&h), SIZE), start);
        assert_eq!(flip_vertically(&at(&v), SIZE), start);
        for map in [rotate, flip_horizontally, flip_vertically] {
            assert!(map(&at(&[]), SIZE).is_empty());
        }
    }

    #[test]
    fn test_components_single_terminal_follows_dag() {
        let (g, _) = graph(2, &[(0, 1)]); // b dangling: one patched terminal component
        let p = components(&g, SIZE);
        assert!(p[0][0] < p[1][0], "{p:?}");
    }
}
