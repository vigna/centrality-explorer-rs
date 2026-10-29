//! The graph canvas: drawing and editing.

use std::collections::HashMap;

use egui::{Align2, Color32, FontId, Pos2, Shape, Stroke, Vec2, pos2, vec2};

use super::geometry::{dist_to_segment, ease, hull, node_radius};
use super::{App, Command, centrality_menu, palette};
use crate::edit::{self, EditKey, Gesture, Target};
use crate::graph::NodeId;
use crate::layouts;

/// The duration of layout animations, in seconds.
pub(crate) const DURATION: f64 = 1.0;

const ARC_COLOR: Color32 = Color32::from_gray(70);
const OUTLINE_COLOR: Color32 = Color32::from_gray(30);
/// The outline color of a node whose score is exactly zero.
const ZERO_OUTLINE_COLOR: Color32 = Color32::from_gray(160);
/// The extra outline width of a node whose score is exactly zero.
const ZERO_OUTLINE_EXTRA: f32 = 1.0;
const ACCENT: Color32 = Color32::from_rgb(0x00, 0x71, 0xe3);
const ARC_WIDTH: f32 = 2.5;
const ARROW_LENGTH: f32 = 14.0;
const ARROW_HALF_WIDTH: f32 = 7.0;
/// The perpendicular offset of each arc of a pair of opposite arcs.
const PAIR_OFFSET: f32 = 6.0;
const LOOP_RADIUS: f32 = 14.0;
const LABEL_SIZE: f32 = 20.0;
const NODE_SLACK: f32 = 4.0;
const ARC_SLACK: f32 = 8.0;
const HULL_PAD: f32 = 14.0;

/// A layout transition: nodes move from their old positions (or from the
/// canvas center, for nodes that had none) to their current ones.
pub(crate) struct Animation {
    from: HashMap<NodeId, [f32; 2]>,
    start: f64,
}

impl Animation {
    /// Creates an animation from the given positions, starting at the given
    /// time.
    pub(crate) fn new(from: HashMap<NodeId, [f32; 2]>, start: f64) -> Self {
        Self { from, start }
    }
}

/// The mapping from stored node positions to canvas-local positions.
///
/// When every node is visible, stored positions are used as they are.
/// Otherwise (e.g., after zooming in, shrinking the window, or opening a file
/// saved on a larger screen) the drawing is scaled down, never up, and
/// translated so that all nodes fit within the layout margins. Stored
/// positions never change, so nodes do not move on their own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct View {
    scale: f32,
    offset: [f32; 2],
}

impl View {
    const IDENTITY: Self = Self {
        scale: 1.0,
        offset: [0.0, 0.0],
    };

    /// Returns the view showing the given stored positions on a canvas of the
    /// given size.
    pub(crate) fn fit(stored: &[[f32; 2]], size: Vec2) -> Self {
        let visible = stored
            .iter()
            .all(|&[x, y]| (0.0..=size.x).contains(&x) && (0.0..=size.y).contains(&y));
        if visible {
            return Self::IDENTITY;
        }
        let (mut min, mut max) = ([f32::MAX; 2], [f32::MIN; 2]);
        for p in stored {
            for c in 0..2 {
                min[c] = min[c].min(p[c]);
                max[c] = max[c].max(p[c]);
            }
        }
        let size = [size.x, size.y];
        let scale = (0..2)
            .filter(|&c| max[c] > min[c])
            .map(|c| (size[c] - 2.0 * layouts::MARGIN).max(0.0) / (max[c] - min[c]))
            .fold(1.0, f32::min)
            .max(1e-3);
        let offset = [0, 1].map(|c| size[c] / 2.0 - scale * (min[c] + max[c]) / 2.0);
        Self { scale, offset }
    }

    /// Maps a stored position to a canvas-local position.
    pub(crate) fn to_canvas(self, p: [f32; 2]) -> [f32; 2] {
        [0, 1].map(|c| self.offset[c] + self.scale * p[c])
    }

    /// Maps a canvas-local position to a stored position.
    pub(crate) fn to_stored(self, p: [f32; 2]) -> [f32; 2] {
        [0, 1].map(|c| (p[c] - self.offset[c]) / self.scale)
    }
}

/// An arc as drawn.
struct DrawnArc {
    from: NodeId,
    to: NodeId,
    shape: ArcShape,
}

enum ArcShape {
    /// A straight segment, possibly with an arrowhead at the end.
    Segment { start: Pos2, end: Pos2, arrow: bool },
    /// A self-loop: a circle above the node, whose lower part is hidden by
    /// the node.
    Loop { center: Pos2, tip: Pos2 },
}

impl DrawnArc {
    fn distance(&self, p: Pos2) -> f32 {
        match self.shape {
            ArcShape::Segment { start, end, .. } => {
                dist_to_segment([p.x, p.y], [start.x, start.y], [end.x, end.y])
            }
            ArcShape::Loop { center, .. } => (p.distance(center) - LOOP_RADIUS).abs(),
        }
    }
}

/// Shows the canvas, handling editing gestures.
pub(crate) fn canvas(app: &mut App, ui: &mut egui::Ui) {
    let (response, painter) =
        ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
    let rect = response.rect;
    app.canvas_rect = rect;
    let origin = rect.min.to_vec2();
    if response.is_pointer_button_down_on() {
        app.animation = None;
    }

    // Positions as displayed, possibly mid-animation. The view is frozen
    // during a drag, so that it does not change under the pointer.
    let graph = app.session.graph();
    let view = match app.dragging {
        Some((_, _, view)) => view,
        None => {
            let stored: Vec<[f32; 2]> = graph.nodes().iter().map(|node| node.pos).collect();
            View::fit(&stored, rect.size())
        }
    };
    let to_screen = |p: [f32; 2]| {
        let [x, y] = view.to_canvas(p);
        pos2(x, y) + origin
    };
    let t = app
        .animation
        .as_ref()
        .map(|animation| ease(((app.now - animation.start) / DURATION) as f32));
    let center = rect.center();
    let positions: Vec<Pos2> = graph
        .nodes()
        .iter()
        .map(|node| {
            let target = to_screen(node.pos);
            match (t, &app.animation) {
                (Some(t), Some(animation)) => {
                    let from = animation
                        .from
                        .get(&node.id)
                        .map_or(center, |&p| to_screen(p));
                    from.lerp(target, t)
                }
                _ => target,
            }
        })
        .collect();
    match t {
        Some(t) if t >= 1.0 => app.animation = None,
        Some(_) => ui.ctx().request_repaint(),
        None => {}
    }

    let n = graph.len();
    let scores: HashMap<NodeId, f64> = app
        .session
        .ranking()
        .ids
        .iter()
        .copied()
        .zip(app.session.scores().iter().copied())
        .collect();
    let radii: Vec<f32> = graph
        .nodes()
        .iter()
        .map(|node| node_radius(scores.get(&node.id).copied().unwrap_or(0.0), n))
        .collect();
    let arcs = drawn_arcs(app, &positions, &radii);
    let ids: Vec<NodeId> = graph.nodes().iter().map(|node| node.id).collect();

    // Hit testing.
    let hit_node = |p: Pos2| {
        (0..n)
            .rev()
            .find(|&i| p.distance(positions[i]) <= radii[i] + NODE_SLACK)
    };
    let hit_arc = |p: Pos2| nearest_arc(&arcs, p);
    let target_at = |p: Pos2| match hit_node(p) {
        Some(i) => Target::Node(ids[i]),
        None => match hit_arc(p) {
            Some(a) => Target::Arc(arcs[a].from, arcs[a].to),
            None => Target::Empty,
        },
    };
    let pointer = response.hover_pos();
    let hovered_node = pointer.and_then(hit_node);
    let hovered_arc = match hovered_node {
        None => pointer.and_then(hit_arc),
        Some(_) => None,
    };
    if response.hovered() {
        app.hovered = hovered_node.map(|i| ids[i]);
        app.hover_seen = true;
    }
    let hovered_id = hovered_node.map(|i| ids[i]);

    // Interaction.
    let mut new_graph = None;
    let mut command = None;
    let local = |p: Pos2| view.to_stored([p.x - origin.x, p.y - origin.y]);
    if response.drag_started_by(egui::PointerButton::Primary)
        && let Some(press) = ui.input(|i| i.pointer.press_origin())
        && let Some(i) = hit_node(press)
    {
        app.dragging = Some((ids[i], press - positions[i], view));
        app.session.begin_drag();
    }
    if let Some((id, offset, _)) = app.dragging {
        if response.dragged()
            && let Some(p) = response.interact_pointer_pos()
        {
            let p = (p - offset).clamp(rect.min, rect.max);
            app.session.drag_to(id, local(p));
        }
        if response.drag_stopped() {
            app.session.end_drag();
            app.dragging = None;
        }
    }
    let graph = app.session.graph();
    if response.clicked()
        && let Some(p) = response.interact_pointer_pos()
    {
        new_graph = app.editor.click(graph, target_at(p));
    }
    if response.double_clicked()
        && let Some(p) = response.interact_pointer_pos()
        && target_at(p) == Target::Empty
    {
        new_graph = Some(app.editor.double_click_empty(graph, local(p)));
    }
    if response.secondary_clicked()
        && let Some(p) = response.interact_pointer_pos()
    {
        app.context = Some((target_at(p), local(p)));
    }
    if !ui.ctx().egui_wants_keyboard_input() {
        for key in edit_keys(ui) {
            if let Some(g) = app.editor.key(app.session.graph(), hovered_id, key) {
                new_graph = Some(g);
            }
        }
    }

    // Drawing, back to front.
    let graph = app.session.graph();
    let index: HashMap<NodeId, usize> = graph
        .nodes()
        .iter()
        .enumerate()
        .map(|(i, node)| (node.id, i))
        .collect();
    let hull_label = app.session.ranking().hull_label;
    for component in &app.session.ranking().hulls {
        let members: Vec<usize> = component
            .iter()
            .filter_map(|id| index.get(id).copied())
            .collect();
        let Some(&first) = members.first() else {
            continue;
        };
        let circles: Vec<([f32; 2], f32)> = members
            .iter()
            .map(|&i| ([positions[i].x, positions[i].y], radii[i]))
            .collect();
        let points: Vec<Pos2> = hull(&circles, HULL_PAD)
            .into_iter()
            .map(|p| pos2(p[0], p[1]))
            .collect();
        let tint = palette::color(graph.nodes()[first].color);
        painter.add(Shape::convex_polygon(
            points.clone(),
            tint.gamma_multiply(0.15),
            Stroke::NONE,
        ));
        let mut closed = points.clone();
        closed.push(points[0]);
        painter.extend(Shape::dashed_line(
            &closed,
            Stroke::new(2.0, tint.gamma_multiply(0.8)),
            8.0,
            6.0,
        ));
        let bottom = points.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        let mid = points.iter().map(|p| p.x).sum::<f32>() / points.len() as f32;
        painter.text(
            pos2(mid, bottom + 4.0),
            Align2::CENTER_TOP,
            hull_label,
            FontId::proportional(15.0),
            tint,
        );
    }

    for (a, arc) in arcs.iter().enumerate() {
        let (width, color) = if hovered_arc == Some(a) {
            (ARC_WIDTH + 2.0, ACCENT)
        } else {
            (ARC_WIDTH, ARC_COLOR)
        };
        draw_arc(&painter, arc, Stroke::new(width, color));
    }

    let pending = match app.editor.gesture() {
        Gesture::PendingEdge { from } => Some(*from),
        _ => None,
    };
    for (i, node) in graph.nodes().iter().enumerate() {
        let (p, r) = (positions[i], radii[i]);
        let fill = palette::color(node.color);
        painter.circle_filled(p, r, fill);
        // The node may be hovered on the canvas or through its curve.
        let outline = outline(
            scores.get(&node.id).copied().unwrap_or(0.0),
            app.hovered == Some(node.id),
        );
        if app.session.ranking().dangling.contains(&node.id) {
            let mut circle: Vec<Pos2> = (0..=40)
                .map(|k| p + Vec2::angled(std::f32::consts::TAU * k as f32 / 40.0) * r)
                .collect();
            circle.push(circle[0]);
            painter.extend(Shape::dashed_line(&circle, outline, 5.0, 4.0));
        } else {
            painter.circle_stroke(p, r, outline);
        }
        if pending == Some(node.id) {
            painter.circle_stroke(p, r + 5.0, Stroke::new(3.0, ACCENT));
        }
        painter.text(
            p,
            Align2::CENTER_CENTER,
            &node.label,
            FontId::proportional(LABEL_SIZE),
            palette::label_color(node.color),
        );
    }

    if let (Some(from), Some(pointer)) = (pending, pointer)
        && let Some(&i) = index.get(&from)
    {
        let dir = (pointer - positions[i]).normalized();
        if dir.is_finite() && pointer.distance(positions[i]) > radii[i] {
            let start = positions[i] + dir * radii[i];
            draw_segment(
                &painter,
                start,
                pointer,
                true,
                Stroke::new(ARC_WIDTH, ACCENT),
            );
        }
    }

    if let Gesture::Renaming { node, text } = app.editor.gesture()
        && let Some(&i) = index.get(node)
    {
        let galley = painter.layout_no_wrap(
            format!("{text}|"),
            FontId::proportional(LABEL_SIZE),
            Color32::BLACK,
        );
        let size = vec2(galley.size().x.max(40.0) + 16.0, galley.size().y + 8.0);
        let frame = egui::Rect::from_center_size(positions[i], size);
        painter.rect_filled(frame, 6.0, Color32::WHITE);
        painter.rect_stroke(
            frame,
            6.0,
            Stroke::new(2.0, ACCENT),
            egui::StrokeKind::Outside,
        );
        painter.galley(frame.center() - galley.size() / 2.0, galley, Color32::BLACK);
    }

    // Context menu.
    let context = app.context;
    response.context_menu(|ui| {
        let Some((target, pos)) = context else {
            return;
        };
        let graph = app.session.graph();
        match target {
            Target::Node(id) => {
                if ui.button("Toggle Self-Loop").clicked() {
                    new_graph = Some(edit::toggle_self_loop(graph, id));
                    ui.close();
                }
                if ui.button("Rename").clicked() {
                    app.editor.start_rename(graph, id);
                    ui.close();
                }
                if ui.button("Delete Node").clicked() {
                    new_graph = Some(edit::delete_node(graph, id));
                    ui.close();
                }
            }
            Target::Arc(from, to) => {
                if ui.button("Delete Edge").clicked() {
                    new_graph = Some(edit::delete_arc(graph, from, to));
                    ui.close();
                }
            }
            Target::Empty => {
                if ui.button("Add Node Here").clicked() {
                    new_graph = Some(app.editor.double_click_empty(graph, pos));
                    ui.close();
                }
                if ui.button("Random Graph…").clicked() {
                    command = Some(Command::Random);
                    ui.close();
                }
                ui.menu_button("Layout", |ui| {
                    for (name, layout) in [
                        ("Force-Directed", layouts::force as super::Layout),
                        ("Circle", layouts::circle),
                        ("Components", layouts::components),
                    ] {
                        if ui.button(name).clicked() {
                            command = Some(Command::Layout(layout));
                            ui.close();
                        }
                    }
                });
                ui.menu_button("Centrality", |ui| {
                    if let Some(c) = centrality_menu(ui, app.session.centrality()) {
                        command = Some(Command::Centrality(c));
                        ui.close();
                    }
                });
                ui.separator();
                if ui.button("Undo").clicked() {
                    command = Some(Command::Undo);
                    ui.close();
                }
                if ui.button("Redo").clicked() {
                    command = Some(Command::Redo);
                    ui.close();
                }
            }
        }
    });

    if let Some(g) = new_graph {
        app.session.commit(g);
        app.prune();
    }
    if let Some(command) = command {
        app.execute(ui.ctx(), command);
    }

    if let Some(id) = hovered_id
        && app.dragging.is_none()
        && let Some(node) = app.session.graph().node(id)
    {
        let score = app
            .session
            .ranking()
            .ids
            .iter()
            .position(|&x| x == id)
            .map_or(0.0, |i| app.session.scores()[i]);
        response.on_hover_text_at_pointer(format!("{}: {score:.4}", node.label));
    }
}

/// Returns the outline of a node with the given score: a thicker gray circle
/// if the score is exactly zero, a dark one otherwise, thicker still if the
/// node is hovered.
///
/// Zeros are structural (transient nodes in the PageRank limit, vanished
/// iterates, unreached nodes) and computed exactly, whereas a tiny positive
/// score, such as that of a transient node near α = 1, is not zero.
fn outline(score: f64, hovered: bool) -> Stroke {
    let width = if hovered { 3.5 } else { 2.0 };
    if score == 0.0 {
        Stroke::new(width + ZERO_OUTLINE_EXTRA, ZERO_OUTLINE_COLOR)
    } else {
        Stroke::new(width, OUTLINE_COLOR)
    }
}

/// Returns the index of the arc nearest to a point, if within the
/// tolerance.
fn nearest_arc(arcs: &[DrawnArc], p: Pos2) -> Option<usize> {
    arcs.iter()
        .enumerate()
        .map(|(a, arc)| (a, arc.distance(p)))
        .filter(|&(_, d)| d <= ARC_SLACK)
        .min_by(|x, y| x.1.total_cmp(&y.1))
        .map(|(a, _)| a)
}

/// Returns the arcs as drawn: opposite arcs side by side, self-loops above
/// their node, and one undirected edge per pair of opposite arcs.
fn drawn_arcs(app: &App, positions: &[Pos2], radii: &[f32]) -> Vec<DrawnArc> {
    let graph = app.session.graph();
    let directed = graph.is_directed();
    let mut arcs = Vec::new();
    for (from, to) in graph.arcs() {
        let (Some(f), Some(t)) = (graph.index_of(from), graph.index_of(to)) else {
            continue;
        };
        if f == t {
            let center = positions[f] - vec2(0.0, radii[f] + LOOP_RADIUS * 0.6);
            let tip = positions[f] + Vec2::angled(-1.15) * radii[f];
            arcs.push(DrawnArc {
                from,
                to,
                shape: ArcShape::Loop { center, tip },
            });
            continue;
        }
        if !directed && from > to {
            continue;
        }
        let dir = (positions[t] - positions[f]).normalized();
        if !dir.is_finite() {
            continue;
        }
        let offset = if directed && graph.has_arc(to, from) {
            dir.rot90() * PAIR_OFFSET
        } else {
            Vec2::ZERO
        };
        let start = positions[f] + dir * radii[f] + offset;
        let end = positions[t] - dir * (radii[t] + 2.0) + offset;
        arcs.push(DrawnArc {
            from,
            to,
            shape: ArcShape::Segment {
                start,
                end,
                arrow: directed,
            },
        });
    }
    arcs
}

fn draw_arc(painter: &egui::Painter, arc: &DrawnArc, stroke: Stroke) {
    match arc.shape {
        ArcShape::Segment { start, end, arrow } => draw_segment(painter, start, end, arrow, stroke),
        ArcShape::Loop { center, tip } => {
            painter.circle_stroke(center, LOOP_RADIUS, stroke);
            let dir = Vec2::angled(1.9);
            arrowhead(painter, tip, dir, stroke.color);
        }
    }
}

fn draw_segment(painter: &egui::Painter, start: Pos2, end: Pos2, arrow: bool, stroke: Stroke) {
    let dir = (end - start).normalized();
    if arrow && dir.is_finite() && start.distance(end) > ARROW_LENGTH {
        painter.line_segment([start, end - dir * ARROW_LENGTH], stroke);
        arrowhead(painter, end, dir, stroke.color);
    } else {
        painter.line_segment([start, end], stroke);
    }
}

/// Draws an arrowhead with the given tip, pointing in the given direction.
fn arrowhead(painter: &egui::Painter, tip: Pos2, dir: Vec2, color: Color32) {
    let base = tip - dir * ARROW_LENGTH;
    let side = dir.rot90() * ARROW_HALF_WIDTH;
    painter.add(Shape::convex_polygon(
        vec![tip, base + side, base - side],
        color,
        Stroke::NONE,
    ));
}

/// Returns the editing keys pressed this frame.
fn edit_keys(ui: &egui::Ui) -> Vec<EditKey> {
    ui.input(|i| {
        let mut keys = Vec::new();
        for event in &i.events {
            match event {
                egui::Event::Text(text) => keys.extend(text.chars().map(EditKey::Char)),
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } if !modifiers.command => match key {
                    egui::Key::Backspace => keys.push(EditKey::Backspace),
                    egui::Key::Delete => keys.push(EditKey::Delete),
                    egui::Key::Enter => keys.push(EditKey::Enter),
                    egui::Key::Escape => keys.push(EditKey::Escape),
                    _ => {}
                },
                _ => {}
            }
        }
        keys
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_outline() {
        assert_eq!(outline(0.0, false).color, ZERO_OUTLINE_COLOR);
        assert!(outline(0.0, false).width > outline(0.2, false).width);
        assert!(outline(0.0, true).width > outline(0.0, false).width);
        // Small is not zero: near α = 1 transient nodes are tiny, not dead.
        assert_eq!(outline(1e-300, false), outline(0.2, false));
        assert_eq!(outline(0.2, false).color, OUTLINE_COLOR);
    }

    #[test]
    fn test_view_identity_when_visible() {
        let view = View::fit(&[[10.0, 10.0], [600.0, 500.0]], vec2(640.0, 640.0));
        assert_eq!(view.to_canvas([600.0, 500.0]), [600.0, 500.0]);
        assert_eq!(view.to_stored([10.0, 10.0]), [10.0, 10.0]);
    }

    #[test]
    fn test_view_fits_invisible_nodes() {
        let stored = [[2000.0, 300.0], [100.0, 100.0], [-50.0, 900.0]];
        let size = vec2(640.0, 480.0);
        let view = View::fit(&stored, size);
        for &p in &stored {
            let [x, y] = view.to_canvas(p);
            assert!(
                (layouts::MARGIN..=size.x - layouts::MARGIN).contains(&x),
                "{x}"
            );
            assert!(
                (layouts::MARGIN..=size.y - layouts::MARGIN).contains(&y),
                "{y}"
            );
            let back = view.to_stored([x, y]);
            assert!((back[0] - p[0]).abs() < 1e-2 && (back[1] - p[1]).abs() < 1e-2);
        }
    }

    #[test]
    fn test_nearest_arc() {
        let (a, b) = (NodeId(0), NodeId(1));
        let segment = |from, to, start: Pos2, end: Pos2| DrawnArc {
            from,
            to,
            shape: ArcShape::Segment {
                start,
                end,
                arrow: true,
            },
        };
        let arcs = [
            segment(a, b, pos2(0.0, -6.0), pos2(100.0, -6.0)),
            segment(b, a, pos2(100.0, 6.0), pos2(0.0, 6.0)),
        ];
        // Within the tolerance of both, but nearer to the second.
        assert_eq!(nearest_arc(&arcs, pos2(50.0, 1.0)), Some(1));
        assert_eq!(nearest_arc(&arcs, pos2(50.0, -2.0)), Some(0));
        assert_eq!(nearest_arc(&arcs, pos2(50.0, 40.0)), None);
    }
}
