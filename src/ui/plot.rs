//! The PageRank curve plot.

use egui::text::{LayoutJob, TextFormat};
use egui::{Align2, Color32, FontId};
use egui_plot::{
    Line, LineStyle, Plot, PlotPoint, PlotPoints, Points, Text, VLine, uniform_grid_spacer,
};

use super::{App, palette};
use crate::rank::Ranking;

const ACCENT: Color32 = Color32::from_rgb(0x00, 0x71, 0xe3);
const CURVE_WIDTH: f32 = 3.0;
const HIGHLIGHT_WIDTH: f32 = 5.5;
const GHOST_WIDTH: f32 = 2.0;
/// The minimum vertical distance between end labels, in pixels.
const LABEL_GAP: f64 = 18.0;
/// The hover tolerance, in pixels.
const HOVER_TOLERANCE: f64 = 10.0;
/// The right end of the x axis, leaving room for the end labels.
const X_MAX: f64 = 1.12;
/// The vertical margin below 0 and above the maximum, as a fraction of the
/// maximum, so that end labels at the extremes are not clipped.
const Y_MARGIN: f64 = 0.05;

/// Returns the smallest multiple of 0.1 that is at least `x`, clamped to
/// [0.1, 1].
pub fn nice_ceiling(x: f64) -> f64 {
    ((x * 10.0 - 1e-9).ceil() / 10.0).clamp(0.1, 1.0)
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

/// Returns the value of a curve at `alpha`, interpolating linearly between
/// samples.
fn value_at(alphas: &[f64], curve: &[f64], alpha: f64) -> f64 {
    let k = alphas
        .partition_point(|&a| a <= alpha)
        .clamp(1, alphas.len() - 1);
    let (a0, a1) = (alphas[k - 1], alphas[k]);
    let t = ((alpha - a0) / (a1 - a0)).clamp(0.0, 1.0);
    curve[k - 1] * (1.0 - t) + curve[k] * t
}

/// Returns the index of the curve whose value at `alpha` is closest to `y`,
/// if it is within `tol`.
pub fn nearest_curve(ranking: &Ranking, alpha: f64, y: f64, tol: f64) -> Option<usize> {
    ranking
        .curves
        .iter()
        .enumerate()
        .map(|(i, curve)| (i, (value_at(&ranking.alphas, curve, alpha) - y).abs()))
        .filter(|&(_, d)| d <= tol)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// Shows the plot of the PageRank curves.
pub(crate) fn plot(app: &mut App, ui: &mut egui::Ui) {
    let session = &app.session;
    let ranking = session.ranking();
    let graph = session.graph();
    let ghost = session.ghost();
    let ghost_max = ghost.map_or(0.0, |ghost| {
        ranking
            .ids
            .iter()
            .filter_map(|&id| ghost.curve(id))
            .flatten()
            .fold(0.0, |m: f64, &x| m.max(x))
    });
    let y_max = if app.fixed_y {
        1.0
    } else {
        nice_ceiling(ranking.max_value().max(ghost_max))
    };
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
    let alpha = session.alpha();
    let scores = session.scores();
    let points = |curve: &[f64]| -> PlotPoints<'_> {
        ranking
            .alphas
            .iter()
            .zip(curve)
            .map(|(&a, &v)| [a, v])
            .collect::<Vec<_>>()
            .into()
    };
    let px_per_unit =
        (ui.available_height() as f64 - 60.0).max(100.0) / ((1.0 + 2.0 * Y_MARGIN) * y_max);

    let response = Plot::new("curves")
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .allow_double_click_reset(false)
        .show_x(false)
        .show_y(false)
        .show_crosshair(false)
        .x_grid_spacer(uniform_grid_spacer(|_| [0.5, 0.1, 0.05]))
        .y_grid_spacer(uniform_grid_spacer(move |_| {
            [y_max, y_max / 2.0, y_max / 10.0]
        }))
        .x_axis_label("α")
        .y_axis_label("PageRank")
        .y_axis_formatter(|mark, _| {
            if mark.value >= -1e-9 {
                format!("{:.2}", mark.value.abs())
            } else {
                String::new()
            }
        })
        .x_axis_formatter(|mark, _| {
            let tenths = mark.value * 10.0;
            if mark.value <= 1.0 + 1e-9 && (tenths - tenths.round()).abs() < 1e-6 {
                format!("{:.1}", mark.value)
            } else {
                String::new()
            }
        })
        .show(ui, |plot_ui| {
            plot_ui.set_plot_bounds_x(0.0..=X_MAX);
            plot_ui.set_plot_bounds_y(-Y_MARGIN * y_max..=(1.0 + Y_MARGIN) * y_max);
            if let Some(ghost) = ghost {
                for (i, &id) in ranking.ids.iter().enumerate() {
                    if let Some(curve) = ghost.curve(id) {
                        plot_ui.line(
                            Line::new(format!("before {}", labels[i]), points(curve))
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
                    Line::new(labels[i].clone(), points(&ranking.curves[i]))
                        .color(color)
                        .width(width)
                        .allow_hover(false),
                );
            }
            plot_ui.vline(VLine::new("α", alpha).color(ACCENT).width(1.5));
            for &i in &order {
                let point = vec![[alpha, scores[i]]];
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
            let ends: Vec<(usize, f64)> = ranking
                .curves
                .iter()
                .enumerate()
                .map(|(i, curve)| (i, *curve.last().expect("curves are not empty")))
                .collect();
            for (value, members) in group_end_labels(&ends, LABEL_GAP / px_per_unit) {
                let mut job = LayoutJob::default();
                for (k, &i) in members.iter().enumerate() {
                    let format = TextFormat::simple(FontId::proportional(18.0), colors[i]);
                    let text = if k == 0 {
                        labels[i].clone()
                    } else {
                        format!(" {}", labels[i])
                    };
                    job.append(&text, 0.0, format);
                }
                plot_ui.text(
                    Text::new("", PlotPoint::new(1.015, value), job)
                        .anchor(Align2::LEFT_CENTER)
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
                p.x.clamp(0.0, 1.0),
                p.y,
                HOVER_TOLERANCE / px_per_unit,
            )
            .map(|i| ranking.ids[i])
        });
    }
    if (response.response.clicked() || response.response.dragged())
        && let Some(p) = response.inner
    {
        app.session.set_alpha(p.x.clamp(0.0, 1.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Graph;

    #[test]
    fn test_nice_ceiling() {
        assert!((nice_ceiling(0.37) - 0.4).abs() < 1e-12);
        assert!((nice_ceiling(0.5) - 0.5).abs() < 1e-12);
        assert!((nice_ceiling(0.0) - 0.1).abs() < 1e-12);
        assert!((nice_ceiling(1.0) - 1.0).abs() < 1e-12);
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
    fn test_nearest_curve() {
        let mut g = Graph::new(true);
        let ids: Vec<_> = (0..5).map(|i| g.add_node([i as f32, 0.0])).collect();
        for (s, t) in [(0, 1), (1, 2), (2, 0), (2, 3), (3, 4), (4, 3)] {
            g.add_arc(ids[s], ids[t]);
        }
        let r = Ranking::new(&g);
        assert_eq!(nearest_curve(&r, 1.0, 0.0, 0.01).map(|i| i < 3), Some(true));
        assert_eq!(nearest_curve(&r, 0.85, 0.9, 0.05), None);
    }
}
