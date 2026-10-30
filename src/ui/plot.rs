/*
 * SPDX-FileCopyrightText: 2026 Sebastiano Vigna
 *
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 */

//! The plot of the curves of the current centrality.

use egui::text::{LayoutJob, TextFormat};
use egui::{Align2, Color32, FontId};
use egui_plot::{
    GridInput, GridMark, Line, LineStyle, Plot, PlotPoint, PlotPoints, Points, Text, VLine,
    uniform_grid_spacer,
};

use super::{App, palette, param_label};
use crate::rank::{Axis, Ranking};

const ACCENT: Color32 = Color32::from_rgb(0x00, 0x71, 0xe3);
const CURVE_WIDTH: f32 = 3.0;
const HIGHLIGHT_WIDTH: f32 = 5.5;
const GHOST_WIDTH: f32 = 2.0;
/// The minimum vertical distance between end labels, in pixels.
const LABEL_GAP: f64 = 18.0;
/// The hover tolerance, in pixels.
const HOVER_TOLERANCE: f64 = 10.0;
/// The right end of the x axis, as a multiple of the right end of the
/// curves, leaving room for the end labels.
const X_END: f64 = 1.12;
/// The position of the end labels, as a multiple of the right end of the
/// curves.
const LABEL_X: f64 = 1.015;
/// The size of the end labels.
const LABEL_SIZE: f32 = 18.0;
/// The size of the note on vanishing iterates.
const NOTE_SIZE: f32 = 20.0;
/// The color of the note on vanishing iterates.
const NOTE_COLOR: Color32 = Color32::from_gray(60);
/// The distance of the note on vanishing iterates above 0, in pixels, so
/// that it clears the dots on the curves.
const NOTE_GAP: f64 = 10.0;
/// The size of the note naming the centrality of the dashed curves.
const DASHED_NOTE_SIZE: f32 = 18.0;
/// The distance of the note naming the centrality of the dashed curves from
/// the left end of the x-axis, as a fraction of its right end.
const DASHED_NOTE_INSET: f64 = 0.01;
/// The size of exponents, relative to the text they follow.
const EXPONENT_SCALE: f32 = 0.65;
/// The vertical margin below 0 and above the maximum, as a fraction of the
/// maximum, so that end labels at the extremes are not clipped.
const Y_MARGIN: f64 = 0.05;

/// Returns a round upper bound for values up to `x` ≥ 0: if `x` ≤ 1, the
/// smallest multiple of 0.1 that is at least `x`, clamped to [0.1 . . 1];
/// otherwise, the smallest multiple of half the largest power of 10 not
/// exceeding `x` that is at least `x`.
pub fn nice_ceiling(x: f64) -> f64 {
    if x <= 1.0 {
        return ((x * 10.0 - 1e-9).ceil() / 10.0).clamp(0.1, 1.0);
    }
    let step = 10_f64.powi(x.log10().floor() as i32) / 2.0;
    (x / step - 1e-9).ceil() * step
}

/// Returns the number of decimals of the labels of the y-axis up to
/// `y_max`: two if `y_max` ≤ 1, and otherwise the fewest, at most two, that
/// show exactly the multiples of `y_max` / 10.
fn y_decimals(y_max: f64) -> usize {
    if y_max <= 1.0 {
        return 2;
    }
    (0..2)
        .find(|&d| {
            let step = y_max / 10.0 * 10_f64.powi(d);
            (step - step.round()).abs() < 1e-9 * step
        })
        .map_or(2, |d| d as usize)
}

/// Groups end labels (index and value) whose values are closer than
/// `min_gap`, returning each group's mean value and members, sorted by
/// value.
pub fn group_end_labels(values: &[(usize, f64)], min_gap: f64) -> Vec<(f64, Vec<usize>)> {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut groups: Vec<(Vec<usize>, Vec<f64>)> = Vec::new();
    for (i, v) in sorted {
        match groups.last_mut() {
            Some((members, vs)) if v - vs.last().expect("groups are not empty") < min_gap => {
                members.push(i);
                vs.push(v);
            }
            _ => groups.push((vec![i], vec![v])),
        }
    }
    groups
        .into_iter()
        .map(|(members, vs)| (vs.iter().sum::<f64>() / vs.len() as f64, members))
        .collect()
}

/// Returns the values of the end labels of the curves of a ranking, by
/// curve index: the last sample of each curve or, if the iterates
/// [vanish](Ranking::vanished), 0, as the scores at the right end of the
/// axis.
fn end_values(ranking: &Ranking) -> Vec<(usize, f64)> {
    ranking
        .curves
        .iter()
        .enumerate()
        .map(|(i, curve)| {
            let end = match ranking.vanished {
                Some(_) => 0.0,
                None => *curve.last().expect("curves are not empty"),
            };
            (i, end)
        })
        .collect()
}

/// Returns the value of a curve sampled at `xs` at `x`, interpolating
/// linearly between samples, and extending the first and last samples
/// beyond the ends (a curve with a single sample is constant).
fn value_at(xs: &[f64], curve: &[f64], x: f64) -> f64 {
    if xs.len() == 1 {
        return curve[0];
    }
    let k = xs.partition_point(|&a| a <= x).clamp(1, xs.len() - 1);
    let (x0, x1) = (xs[k - 1], xs[k]);
    let t = ((x - x0) / (x1 - x0)).clamp(0.0, 1.0);
    curve[k - 1] * (1.0 - t) + curve[k] * t
}

/// Returns the index of the curve whose value at `x` is closest to `y`, if
/// it is within `tol`.
///
/// If the iterates [vanish](Ranking::vanished), the value of every curve
/// beyond its last sample is 0, as the scores there.
pub fn nearest_curve(ranking: &Ranking, x: f64, y: f64, tol: f64) -> Option<usize> {
    let vanished = ranking.vanished.is_some() && ranking.xs.last().is_some_and(|&last| x > last);
    ranking
        .curves
        .iter()
        .enumerate()
        .map(|(i, curve)| {
            let value = if vanished {
                0.0
            } else {
                value_at(&ranking.xs, curve, x)
            };
            (i, (value - y).abs())
        })
        .filter(|&(_, d)| d <= tol)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// Returns the distance between the labeled ticks of an axis of *K*
/// iterations: the smallest of 1, 2, 5, 10, 20, 50, … that splits [0 . . *K*]
/// into at most five intervals.
fn iteration_unit(iterations: usize) -> f64 {
    let mut unit = 1.0;
    let mut multipliers = [2.0, 2.5, 2.0].into_iter().cycle();
    while iterations as f64 > 5.0 * unit {
        unit *= multipliers.next().expect("cycles are infinite");
    }
    unit
}

/// Returns the step sizes of the grid of the x-axis: tenths and twentieths
/// for continuous axes; for iterations, the [unit](iteration_unit), its
/// half, and its tenth, replaced by 1 when they are not integers; and none
/// for flat curves.
fn x_steps(axis: Axis) -> Option<[f64; 3]> {
    match axis {
        Axis::Iteration(iterations) => {
            let unit = iteration_unit(iterations);
            Some([unit, unit / 2.0, unit / 10.0].map(|s| if s.fract() == 0.0 { s } else { 1.0 }))
        }
        Axis::Flat => None,
        _ => Some([0.5, 0.1, 0.05]),
    }
}

/// Returns the label of the tick at `x` of an axis, up to the right end of
/// the curves: multiples of 0.1 with one decimal for continuous axes,
/// multiples of the [unit](iteration_unit) for iterations, and nothing
/// otherwise.
fn x_tick_label(axis: Axis, x: f64) -> String {
    let (unit, decimals) = match axis {
        Axis::Iteration(iterations) => (iteration_unit(iterations), 0),
        Axis::Flat => return String::new(),
        _ => (0.1, 1),
    };
    let units = x / unit;
    if x <= axis.x_max() + 1e-9 && (units - units.round()).abs() < 1e-6 {
        format!("{x:.decimals$}")
    } else {
        String::new()
    }
}

/// Returns the note on iterates that vanish from iteration `v` on,
/// “1·*A*<sup>*k*</sup> = 0 for *k* ≥ `v`”, on the given background; the
/// exponent is raised by layout, as the default font has no superscript *k*.
fn vanished_note(v: usize, background: Color32) -> LayoutJob {
    let normal = TextFormat {
        background,
        ..TextFormat::simple(FontId::proportional(NOTE_SIZE), NOTE_COLOR)
    };
    let italics = TextFormat {
        italics: true,
        ..normal.clone()
    };
    let exponent = TextFormat {
        font_id: FontId::proportional(NOTE_SIZE * EXPONENT_SCALE),
        valign: egui::Align::TOP,
        ..italics.clone()
    };
    let mut job = LayoutJob::default();
    for (text, format) in [
        ("1·", &normal),
        ("A", &italics),
        ("k", &exponent),
        (" = 0 for ", &normal),
        ("k", &italics),
        (&format!(" ≥ {v}"), &normal),
    ] {
        job.append(text, 0.0, format.clone());
    }
    job
}

/// Shows the plot of the curves of the current centrality.
pub(crate) fn plot(app: &mut App, ui: &mut egui::Ui) {
    let session = &app.session;
    let ranking = session.ranking();
    let axis = ranking.axis;
    let x_max = axis.x_max();
    let graph = session.graph();
    let directed = graph.is_directed();
    let ghost = session.ghost();
    let ghost_max = ghost.map_or(0.0, |ghost| {
        ranking
            .ids
            .iter()
            .filter_map(|&id| ghost.curve(id))
            .flatten()
            .fold(0.0, |m: f64, &x| m.max(x))
    });
    let param = session.param();
    let y_max = if app.fixed_y {
        1.0
    } else if ranking.unbounded {
        // The y-axis follows the scores at the parameter, or at the last
        // sample, where they are finite.
        let max_at = |r: &Ranking| {
            let x = param.unwrap_or(0.0).min(*r.xs.last().unwrap_or(&0.0));
            r.scores(x).into_iter().fold(0.0, f64::max)
        };
        nice_ceiling(max_at(ranking).max(ghost.map_or(0.0, max_at)))
    } else {
        nice_ceiling(ranking.max_value().max(ghost_max))
    };
    let y_decimals = y_decimals(y_max);
    let colors: Vec<Color32> = ranking
        .ids
        .iter()
        .map(|&id| {
            graph
                .node(id)
                .map_or(Color32::GRAY, |n| palette::color(n.color))
        })
        .collect();
    let labels: Vec<String> = ranking
        .ids
        .iter()
        .map(|&id| graph.node(id).map_or_else(String::new, |n| n.label.clone()))
        .collect();
    let hovered = app
        .hovered
        .and_then(|id| ranking.ids.iter().position(|&x| x == id));
    let scores = session.scores();
    let points = |xs: &[f64], curve: &[f64]| -> PlotPoints<'_> {
        xs.iter()
            .zip(curve)
            .map(|(&x, &v)| [x, v])
            .collect::<Vec<_>>()
            .into()
    };
    let px_per_unit =
        (ui.available_height() as f64 - 60.0).max(100.0) / ((1.0 + 2.0 * Y_MARGIN) * y_max);
    let x_spacer: Box<dyn Fn(GridInput) -> Vec<GridMark>> = match x_steps(axis) {
        Some(steps) => uniform_grid_spacer(move |_| steps),
        None => Box::new(|_| Vec::new()),
    };
    // Iterates of a nonempty graph that vanish (on an empty graph, they
    // vanish trivially).
    let vanished = ranking.vanished.filter(|_| !ranking.ids.is_empty());
    let background = ui.visuals().extreme_bg_color;

    let response = Plot::new("curves")
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .allow_double_click_reset(false)
        .show_x(false)
        .show_y(false)
        .show_crosshair(false)
        .x_grid_spacer(x_spacer)
        .y_grid_spacer(uniform_grid_spacer(move |_| {
            [y_max, y_max / 2.0, y_max / 10.0]
        }))
        .x_axis_label(axis.label())
        .y_axis_label(format!(
            "{} ({})",
            ranking.centrality.name(directed),
            ranking.norm.label()
        ))
        .y_axis_formatter(move |mark, _| {
            if mark.value >= -1e-9 {
                format!("{:.y_decimals$}", mark.value.abs())
            } else {
                String::new()
            }
        })
        .x_axis_formatter(move |mark, _| x_tick_label(axis, mark.value))
        .show(ui, |plot_ui| {
            plot_ui.set_plot_bounds_x(0.0..=x_max * X_END);
            plot_ui.set_plot_bounds_y(-Y_MARGIN * y_max..=(1.0 + Y_MARGIN) * y_max);
            if let Some(ghost) = ghost {
                // The ghost has its own samples: for example, its iterates
                // may vanish at a different iteration. Flat curves, of a
                // geometric centrality, span the whole axis.
                let xs = match ghost.axis {
                    Axis::Flat => vec![0.0, x_max],
                    _ => ghost.xs.clone(),
                };
                for (i, &id) in ranking.ids.iter().enumerate() {
                    if let Some(curve) = ghost.curve(id) {
                        plot_ui.line(
                            Line::new(format!("before {}", labels[i]), points(&xs, curve))
                                .color(colors[i].gamma_multiply(0.45))
                                .width(GHOST_WIDTH)
                                .style(LineStyle::dashed_loose())
                                .allow_hover(false),
                        );
                    }
                }
            }
            // The highlighted curve is drawn last, on top of the others.
            let mut order: Vec<usize> = (0..ranking.ids.len()).collect();
            if let Some(h) = hovered {
                order.retain(|&i| i != h);
                order.push(h);
            }
            for &i in &order {
                let (color, width) = match hovered {
                    Some(h) if h == i => (colors[i], HIGHLIGHT_WIDTH),
                    Some(_) => (colors[i].gamma_multiply(0.25), CURVE_WIDTH),
                    None => (colors[i], CURVE_WIDTH),
                };
                plot_ui.line(
                    Line::new(labels[i].clone(), points(&ranking.xs, &ranking.curves[i]))
                        .color(color)
                        .width(width)
                        .allow_hover(false),
                );
            }
            if let Some(x) = param {
                plot_ui.vline(VLine::new(axis.label(), x).color(ACCENT).width(1.5));
                // Scores may be infinite, as Katz's index at βρ = 1 when not
                // normalized.
                for &i in order.iter().filter(|&&i| scores[i].is_finite()) {
                    let point = vec![[x, scores[i]]];
                    plot_ui.points(
                        Points::new("", point.clone())
                            .radius(7.0)
                            .color(Color32::WHITE)
                            .filled(true),
                    );
                    plot_ui.points(
                        Points::new(labels[i].clone(), point)
                            .radius(5.0)
                            .color(colors[i])
                            .filled(true),
                    );
                }
            }
            // End labels above the plot, as those of unbounded scores, are
            // grouped at its top, and wrapped within the margin on the right
            // of the curves.
            let ends = end_values(ranking);
            let above: Vec<bool> = ends.iter().map(|&(_, v)| v > y_max).collect();
            let ends: Vec<_> = ends.into_iter().map(|(i, v)| (i, v.min(y_max))).collect();
            let margin = plot_ui.transform().frame().width() * ((X_END - LABEL_X) / X_END) as f32;
            for (value, members) in group_end_labels(&ends, LABEL_GAP / px_per_unit) {
                let mut job = LayoutJob::default();
                for (k, &i) in members.iter().enumerate() {
                    let format = TextFormat::simple(FontId::proportional(LABEL_SIZE), colors[i]);
                    let text = if k == 0 {
                        labels[i].clone()
                    } else {
                        format!(" {}", labels[i])
                    };
                    job.append(&text, 0.0, format);
                }
                let (y, anchor) = if members.iter().any(|&i| above[i]) {
                    ((1.0 + Y_MARGIN) * y_max, Align2::LEFT_TOP)
                } else {
                    (value, Align2::LEFT_CENTER)
                };
                job.wrap.max_width = margin;
                // A laid-out galley, as plot texts override the wrapping of
                // layout jobs.
                let galley = plot_ui.ctx().fonts_mut(|f| f.layout_job(job));
                plot_ui.text(
                    Text::new("", PlotPoint::new(x_max * LABEL_X, y), galley)
                        .anchor(anchor)
                        .allow_hover(false),
                );
            }
            // At the top left, the centrality of the dashed curves, and the
            // value of its parameter at which they are frozen, if any, if they
            // are not the current centrality on the graph before the last
            // edit.
            if let Some(ghost) = ghost.filter(|g| g.centrality != ranking.centrality) {
                let format = TextFormat {
                    background,
                    ..TextFormat::simple(FontId::proportional(DASHED_NOTE_SIZE), NOTE_COLOR)
                };
                let mut job = LayoutJob::default();
                let name = ghost.centrality.name(directed);
                let text = match ghost.frozen {
                    Some((axis, x)) => format!("Dashed: {name} at {}", param_label(axis, x)),
                    None => format!("Dashed: {name}"),
                };
                job.append(&text, 0.0, format);
                plot_ui.text(
                    Text::new(
                        "",
                        PlotPoint::new(
                            x_max * DASHED_NOTE_INSET,
                            (1.0 + Y_MARGIN) * y_max - NOTE_GAP / px_per_unit,
                        ),
                        job,
                    )
                    .anchor(Align2::LEFT_TOP)
                    .allow_hover(false),
                );
            }
            // At the bottom right of the curves, just above 0, masking the
            // vertical line.
            if let Some(v) = vanished {
                plot_ui.text(
                    Text::new(
                        "",
                        PlotPoint::new(x_max, NOTE_GAP / px_per_unit),
                        vanished_note(v, background),
                    )
                    .anchor(Align2::RIGHT_BOTTOM)
                    .allow_hover(false),
                );
            }
            plot_ui.pointer_coordinate()
        });

    if response.response.hovered() {
        app.hover_seen = true;
        app.hovered = response.inner.and_then(|p| {
            nearest_curve(
                ranking,
                p.x.clamp(0.0, x_max),
                p.y,
                HOVER_TOLERANCE / px_per_unit,
            )
            .map(|i| ranking.ids[i])
        });
    }
    if (response.response.clicked() || response.response.dragged())
        && let Some(p) = response.inner
    {
        app.session.set_param(p.x.clamp(0.0, x_max));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Graph;
    use crate::rank::{Centrality, DEFAULT_ITERATIONS};

    /// Returns eigenvector centrality on the 4-cycle *a* → *b* → *c* → *d* →
    /// *a*, and on the same graph after deleting *d* → *a*, that is, on the
    /// path *a* → *b* → *c* → *d*, where the iterates vanish at *k* = 4, and
    /// the last one, at *k* = 3, is 1 on *d*.
    fn cycle_and_path() -> (Ranking, Ranking) {
        let mut g = Graph::new(true);
        let ids: Vec<_> = (0..4).map(|i| g.add_node([i as f32, 0.0])).collect();
        for (s, t) in [(0, 1), (1, 2), (2, 3), (3, 0)] {
            g.add_arc(ids[s], ids[t]);
        }
        let cycle = Ranking::new(&g, Centrality::Eigenvector);
        g.remove_arc(ids[3], ids[0]);
        (cycle, Ranking::new(&g, Centrality::Eigenvector))
    }

    #[test]
    fn test_nice_ceiling() {
        assert!((nice_ceiling(0.37) - 0.4).abs() < 1e-12);
        assert!((nice_ceiling(0.5) - 0.5).abs() < 1e-12);
        assert!((nice_ceiling(0.0) - 0.1).abs() < 1e-12);
        assert!((nice_ceiling(1.0) - 1.0).abs() < 1e-12);
        assert!((nice_ceiling(1.2) - 1.5).abs() < 1e-12);
        assert!((nice_ceiling(3.7) - 4.0).abs() < 1e-12);
        assert!((nice_ceiling(10.0) - 10.0).abs() < 1e-12);
        assert!((nice_ceiling(37.0) - 40.0).abs() < 1e-12);
        assert!((nice_ceiling(1234.0) - 1500.0).abs() < 1e-9);
    }

    #[test]
    fn test_y_decimals() {
        assert_eq!(y_decimals(0.4), 2);
        assert_eq!(y_decimals(1.0), 2);
        assert_eq!(y_decimals(1.5), 2);
        assert_eq!(y_decimals(3.0), 1);
        assert_eq!(y_decimals(15.0), 1);
        assert_eq!(y_decimals(30.0), 0);
        assert_eq!(y_decimals(1500.0), 0);
    }

    #[test]
    fn test_iteration_ticks() {
        for (iterations, unit) in [(1, 1.0), (7, 2.0), (20, 5.0), (50, 10.0), (51, 20.0)] {
            assert_eq!(iteration_unit(iterations), unit, "{iterations}");
        }
        assert_eq!(iteration_unit(1000), 200.0);
        assert_eq!(
            x_steps(Axis::Iteration(DEFAULT_ITERATIONS)),
            Some([10.0, 5.0, 1.0])
        );
        assert_eq!(x_steps(Axis::Iteration(20)), Some([5.0, 1.0, 1.0]));
        assert_eq!(x_tick_label(Axis::Iteration(20), 15.0), "15");
        assert_eq!(x_tick_label(Axis::Iteration(20), 12.0), "");
        assert_eq!(x_tick_label(Axis::Iteration(17), 20.0), "");
    }

    #[test]
    fn test_group_end_labels() {
        let groups = group_end_labels(&[(0, 0.0), (1, 0.5), (2, 0.001), (3, 0.5)], 0.02);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].1, vec![0, 2]);
        assert_eq!(groups[1].1, vec![1, 3]);
        assert!((groups[1].0 - 0.5).abs() < 1e-12);
    }

    #[test]
    fn test_value_at_single_sample() {
        assert_eq!(value_at(&[0.0], &[0.3], 5.0), 0.3);
    }

    #[test]
    fn test_nearest_curve() {
        let mut g = Graph::new(true);
        let ids: Vec<_> = (0..5).map(|i| g.add_node([i as f32, 0.0])).collect();
        for (s, t) in [(0, 1), (1, 2), (2, 0), (2, 3), (3, 4), (4, 3)] {
            g.add_arc(ids[s], ids[t]);
        }
        let r = Ranking::new(&g, Centrality::PageRank);
        assert_eq!(nearest_curve(&r, 1.0, 0.0, 0.01).map(|i| i < 3), Some(true));
        assert_eq!(nearest_curve(&r, 0.85, 0.9, 0.05), None);
    }

    #[test]
    fn test_end_values() {
        let (cycle, path) = cycle_and_path();
        // On the cycle, the iterates are uniform, and the labels sit at the
        // last sample
        assert_eq!(cycle.vanished, None);
        assert_eq!(
            end_values(&cycle),
            vec![(0, 0.25), (1, 0.25), (2, 0.25), (3, 0.25)]
        );
        // On the path, the iterates vanish at k = 4: the labels sit at 0,
        // with the dots at k = K, not at the last sample (1 on d)
        assert_eq!(path.vanished, Some(4));
        assert_eq!(path.curves[3].last(), Some(&1.0));
        assert_eq!(
            end_values(&path),
            vec![(0, 0.0), (1, 0.0), (2, 0.0), (3, 0.0)]
        );
        let at_k: Vec<_> = path
            .scores(DEFAULT_ITERATIONS as f64)
            .into_iter()
            .enumerate()
            .collect();
        assert_eq!(end_values(&path), at_k);
    }

    #[test]
    fn test_nearest_curve_past_vanishing() {
        let (_, path) = cycle_and_path();
        // At the last sample, k = 3, the curve of d is at 1
        assert_eq!(nearest_curve(&path, 3.0, 1.0, 0.01), Some(3));
        // Past it, no curve is drawn, and every score is 0
        assert_eq!(nearest_curve(&path, 3.5, 1.0, 0.01), None);
        assert_eq!(
            nearest_curve(&path, DEFAULT_ITERATIONS as f64, 1.0, 0.01),
            None
        );
        assert!(nearest_curve(&path, DEFAULT_ITERATIONS as f64, 0.0, 0.01).is_some());
    }
}
