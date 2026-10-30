/*
 * SPDX-FileCopyrightText: 2026 Sebastiano Vigna
 *
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 */

//! Geometry helpers for the canvas.

use std::f32::consts::TAU;

/// The radius of a node with uniform score 1/*n*.
pub const R0: f32 = 24.0;
/// The minimum node radius (the label must stay readable).
pub const R_MIN: f32 = 14.0;
/// The maximum node radius.
pub const R_MAX: f32 = 64.0;

/// Returns the radius of a node with the given score in a graph with `n`
/// nodes: the area is proportional to the score, normalized so that the
/// uniform score 1/*n* gives [`R0`], and clamped to [[`R_MIN`] . . [`R_MAX`]].
pub fn node_radius(score: f64, n: usize) -> f32 {
    (R0 * (score * n as f64).sqrt() as f32).clamp(R_MIN, R_MAX)
}

/// Returns the distance between a point and a segment.
pub fn dist_to_segment(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let ap = [p[0] - a[0], p[1] - a[1]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if len2 > 0.0 {
        ((ap[0] * ab[0] + ap[1] * ab[1]) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let q = [a[0] + t * ab[0], a[1] + t * ab[1]];
    (p[0] - q[0]).hypot(p[1] - q[1])
}

/// Returns the convex hull, in counterclockwise order, of a set of circles
/// (center and radius), each enlarged by `pad`.
///
/// # Implementation Notes
///
/// The hull is computed with Andrew's monotone chain on 16 points sampled on
/// each circle.
pub fn hull(circles: &[([f32; 2], f32)], pad: f32) -> Vec<[f32; 2]> {
    const SAMPLES: usize = 16;
    let mut points: Vec<[f32; 2]> = circles
        .iter()
        .flat_map(|&(c, r)| {
            (0..SAMPLES).map(move |i| {
                let angle = TAU * i as f32 / SAMPLES as f32;
                [
                    c[0] + (r + pad) * angle.cos(),
                    c[1] + (r + pad) * angle.sin(),
                ]
            })
        })
        .collect();
    points.sort_by(|p, q| p[0].total_cmp(&q[0]).then(p[1].total_cmp(&q[1])));
    points.dedup();
    if points.len() < 3 {
        return points;
    }
    let cross = |o: [f32; 2], a: [f32; 2], b: [f32; 2]| {
        (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    };
    let mut hull: Vec<[f32; 2]> = Vec::with_capacity(2 * points.len());
    for pass in 0..2 {
        let start = hull.len();
        let iter: Box<dyn Iterator<Item = &[f32; 2]>> = if pass == 0 {
            Box::new(points.iter())
        } else {
            Box::new(points.iter().rev())
        };
        for &p in iter {
            while hull.len() >= start + 2
                && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0
            {
                hull.pop();
            }
            hull.push(p);
        }
        hull.pop();
    }
    hull
}

/// Returns a cubic ease-in-out of `t`, clamped to [0 . . 1].
pub fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(
            h.iter()
                .all(|&[x, y]| ((x * x + y * y).sqrt() - 15.0).abs() < 1e-3)
        );
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
}
