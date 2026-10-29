use centrality_explorer::ui::App;
use egui_kittest::Harness;

fn harness() -> Harness<'static, App> {
    Harness::builder()
        .with_size([1280.0, 720.0])
        .build_eframe(|cc| App::new(cc))
}

#[test]
fn test_app_starts() {
    let mut h = harness();
    h.run();
    assert!(h.state().session().graph().is_empty());
    assert_eq!(h.state().session().alpha(), 0.85);
}

#[test]
fn test_undo_shortcut() {
    let mut h = harness();
    let mut g = h.state().session().graph().clone();
    g.add_node([100.0, 100.0]);
    h.state_mut().session_mut().commit(g);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    h.run();
    assert!(h.state().session().graph().is_empty());
    h.key_press_modifiers(
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        egui::Key::Z,
    );
    h.run();
    assert_eq!(h.state().session().graph().len(), 1);
}

#[test]
fn test_alpha_arrows() {
    let mut h = harness();
    h.run();
    h.key_press(egui::Key::ArrowRight);
    h.run();
    assert!((h.state().session().alpha() - 0.86).abs() < 1e-9);
}

use centrality_explorer::graph::{Graph, NodeId};
use centrality_explorer::rank::{Centrality, ITERATIONS};

fn screen(h: &Harness<'static, App>, p: [f32; 2]) -> egui::Pos2 {
    h.state().canvas_rect().min + egui::vec2(p[0], p[1])
}

fn hover(h: &mut Harness<'static, App>, p: [f32; 2]) {
    let pos = screen(h, p);
    h.hover_at(pos);
    h.run();
}

fn click_at(h: &mut Harness<'static, App>, p: [f32; 2]) {
    let pos = screen(h, p);
    h.hover_at(pos);
    h.run();
    h.drag_at(pos);
    h.run();
    h.drop_at(pos);
    h.run();
}

fn drag(h: &mut Harness<'static, App>, from: [f32; 2], to: [f32; 2]) {
    let (a, b) = (screen(h, from), screen(h, to));
    h.hover_at(a);
    h.run();
    h.drag_at(a);
    h.run();
    h.hover_at(a + (b - a) / 2.0);
    h.run();
    h.hover_at(b);
    h.run();
    h.drop_at(b);
    h.run();
}

fn two_node_app() -> (Harness<'static, App>, NodeId, NodeId) {
    let mut h = harness();
    h.run();
    let mut g = h.state().session().graph().clone();
    let (a, b) = (g.add_node([150.0, 300.0]), g.add_node([400.0, 300.0]));
    h.state_mut().session_mut().commit(g);
    h.run();
    (h, a, b)
}

#[test]
fn test_canvas_click_click_toggles_arc() {
    let (mut h, a, b) = two_node_app();
    click_at(&mut h, [150.0, 300.0]);
    click_at(&mut h, [400.0, 300.0]);
    assert!(h.state().session().graph().has_arc(a, b));
    click_at(&mut h, [150.0, 300.0]);
    click_at(&mut h, [400.0, 300.0]);
    assert!(!h.state().session().graph().has_arc(a, b));
}

#[test]
fn test_canvas_drag_moves_node() {
    let (mut h, a, _) = two_node_app();
    drag(&mut h, [150.0, 300.0], [230.0, 300.0]);
    let pos = h.state().session().graph().node(a).unwrap().pos;
    assert!(
        (pos[0] - 230.0).abs() < 2.0 && (pos[1] - 300.0).abs() < 2.0,
        "{pos:?}"
    );
    assert!(!h.state().session().graph().has_arc(a, a));
    h.state_mut().session_mut().undo();
    assert_eq!(
        h.state().session().graph().node(a).unwrap().pos,
        [150.0, 300.0]
    );
}

#[test]
fn test_canvas_backspace_deletes_hovered() {
    let (mut h, a, _) = two_node_app();
    hover(&mut h, [150.0, 300.0]);
    h.key_press(egui::Key::Backspace);
    h.run();
    assert!(h.state().session().graph().node(a).is_none());
}

/// Renders the lecture example offscreen to `target/screenshots/` for visual
/// inspection. Run with `cargo test --test test_ui -- --ignored`.
#[test]
#[ignore]
fn test_render_example() {
    let mut h = Harness::builder()
        .with_size([1280.0, 720.0])
        .with_theme(egui::Theme::Light)
        .wgpu()
        .build_eframe(|cc| App::new(cc));
    h.run();
    let mut g = h.state().session().graph().clone();
    let pos = [
        [120.0, 200.0],
        [260.0, 110.0],
        [240.0, 330.0],
        [380.0, 460.0],
        [500.0, 330.0],
    ];
    let ids: Vec<_> = pos.iter().map(|&p| g.add_node(p)).collect();
    for (s, t) in [(0, 1), (1, 2), (2, 0), (2, 3), (3, 4)] {
        g.add_arc(ids[s], ids[t]);
    }
    h.state_mut().session_mut().commit(g.clone());
    g.add_arc(ids[4], ids[3]);
    h.state_mut().session_mut().commit(g);
    h.run();
    hover(&mut h, [120.0, 200.0]);
    std::fs::create_dir_all("target/screenshots").unwrap();
    h.render()
        .unwrap()
        .save("target/screenshots/example.png")
        .unwrap();
}

/// Renders self-loops, dangling nodes, and a pending edge offscreen to
/// `target/screenshots/` for visual inspection. Run with
/// `cargo test --test test_ui -- --ignored`.
#[test]
#[ignore]
fn test_render_details() {
    let mut h = Harness::builder()
        .with_size([1280.0, 720.0])
        .with_theme(egui::Theme::Light)
        .wgpu()
        .build_eframe(|cc| App::new(cc));
    h.run();
    let mut g = h.state().session().graph().clone();
    let a = g.add_node([150.0, 300.0]);
    let b = g.add_node([400.0, 300.0]);
    let c = g.add_node([300.0, 500.0]);
    g.add_arc(a, a);
    g.add_arc(a, b);
    g.add_arc(b, c);
    h.state_mut().session_mut().commit(g);
    h.run();
    click_at(&mut h, [400.0, 300.0]);
    hover(&mut h, [520.0, 180.0]);
    std::fs::create_dir_all("target/screenshots").unwrap();
    h.render()
        .unwrap()
        .save("target/screenshots/details.png")
        .unwrap();
}

#[test]
fn test_generate_random() {
    let mut h = harness();
    h.run();
    h.state_mut()
        .generate_random(centrality_explorer::random::ChungLu::default(), 3);
    h.run();
    let app = h.state();
    let g = app.session().graph();
    assert_eq!(g.len(), 10);
    assert_eq!(g.seed(), Some(3));
    let size = app.canvas_rect().size();
    assert!(
        g.nodes()
            .iter()
            .all(|n| n.pos[0] <= size.x && n.pos[1] <= size.y)
    );
    assert!(app.session().ghost().is_none());
}

/// Renders a random graph with the random-graph dialog open offscreen to
/// `target/screenshots/` for visual inspection. Run with
/// `cargo test --test test_ui -- --ignored`.
#[test]
#[ignore]
fn test_render_random() {
    let mut h = Harness::builder()
        .with_size([1280.0, 720.0])
        .with_theme(egui::Theme::Light)
        .wgpu()
        .build_eframe(|cc| App::new(cc));
    h.run();
    h.state_mut()
        .generate_random(centrality_explorer::random::ChungLu::default(), 3);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::R);
    for _ in 0..90 {
        h.step();
    }
    h.run();
    std::fs::create_dir_all("target/screenshots").unwrap();
    h.render()
        .unwrap()
        .save("target/screenshots/random.png")
        .unwrap();
}

#[test]
fn test_quit_asks_about_unsaved_changes() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness();
    h.run();
    let mut g = h.state().session().graph().clone();
    g.add_node([100.0, 100.0]);
    h.state_mut().session_mut().commit(g);
    h.run();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Q);
    h.run();
    assert!(h.query_by_label("Unsaved Changes").is_some());
}

/// Returns the lecture example on the graph of the harness before and after
/// the edit that closes {d, e}, that is, a → b → c → a, c → d → e, and the
/// same graph with e → d, with the identifiers of a, …, e.
fn lecture_example(h: &Harness<'static, App>) -> (Graph, Graph, Vec<NodeId>) {
    let mut before = h.state().session().graph().clone();
    let pos = [
        [120.0, 200.0],
        [260.0, 110.0],
        [240.0, 330.0],
        [380.0, 460.0],
        [500.0, 330.0],
    ];
    let ids: Vec<_> = pos.iter().map(|&p| before.add_node(p)).collect();
    for (s, t) in [(0, 1), (1, 2), (2, 0), (2, 3), (3, 4)] {
        before.add_arc(ids[s], ids[t]);
    }
    let mut after = before.clone();
    after.add_arc(ids[4], ids[3]);
    (before, after, ids)
}

/// Returns a light-theme harness rendering with wgpu.
fn wgpu_harness() -> Harness<'static, App> {
    Harness::builder()
        .with_size([1280.0, 720.0])
        .with_theme(egui::Theme::Light)
        .wgpu()
        .build_eframe(|cc| App::new(cc))
}

/// Builds the lecture example (after closing {d, e}) in a light-theme
/// harness rendering with wgpu.
fn lecture_harness() -> (Harness<'static, App>, Vec<NodeId>) {
    let mut h = wgpu_harness();
    h.run();
    let (_, after, ids) = lecture_example(&h);
    h.state_mut().session_mut().commit(after);
    h.run();
    (h, ids)
}

/// Mean luminance of the pixels on a circle.
fn ring_luminance(img: &image::RgbaImage, center: egui::Pos2, radius: f32) -> f32 {
    let samples = 16;
    (0..samples)
        .map(|k| {
            let angle = std::f32::consts::TAU * k as f32 / samples as f32;
            let p = center + egui::Vec2::angled(angle) * radius;
            let px = img.get_pixel(p.x.round() as u32, p.y.round() as u32);
            0.2126 * px[0] as f32 + 0.7152 * px[1] as f32 + 0.0722 * px[2] as f32
        })
        .sum::<f32>()
        / samples as f32
}

/// Hovering a curve in the plot highlights its node on the canvas (the node
/// outline thickens).
#[test]
#[ignore = "needs a GPU"]
fn test_curve_hover_highlights_node() {
    let (mut h, _) = lecture_harness();
    // Pointer on an empty part of the plot.
    h.hover_at(egui::pos2(900.0, 80.0));
    h.run();
    let plain = h.render().unwrap();
    // Find a pixel of curve a (blue) in the plot, away from the α line.
    let blue = image::Rgba([0x1f, 0x77, 0xb4, 0xff]);
    let (x, y) = (720..1100)
        .flat_map(|x| (40..640).map(move |y| (x, y)))
        .find(|&(x, y)| *plain.get_pixel(x, y) == blue)
        .expect("curve a is visible");
    h.hover_at(egui::pos2(x as f32, y as f32));
    h.run();
    let hovered = h.render().unwrap();
    // Node a: score 0.0773 with 5 nodes gives radius 14.92; the normal
    // outline ends at 15.92, the highlighted one at 16.67.
    let center = h.state().canvas_rect().min + egui::vec2(120.0, 200.0);
    let (before, after) = (
        ring_luminance(&plain, center, 16.4),
        ring_luminance(&hovered, center, 16.4),
    );
    assert!(after < before - 40.0, "before {before}, after {after}");
}

/// Renders the lecture example in the components layout, laid out at zoom 1
/// and then shown at zoom 1.5, offscreen to `target/screenshots/` for visual
/// inspection. Run with `cargo test --test test_ui -- --ignored`.
#[test]
#[ignore = "needs a GPU"]
fn test_render_zoomed_components() {
    let (mut h, _) = lecture_harness();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num3);
    for _ in 0..90 {
        h.step();
    }
    h.run();
    h.ctx.set_zoom_factor(1.5);
    h.run();
    std::fs::create_dir_all("target/screenshots").unwrap();
    h.render()
        .unwrap()
        .save("target/screenshots/zoomed_components.png")
        .unwrap();
}

#[test]
fn test_centrality_shortcuts() {
    let (mut h, _, _) = two_node_app();
    h.key_press_modifiers(
        egui::Modifiers::COMMAND | egui::Modifiers::ALT,
        egui::Key::Num2,
    );
    h.run();
    assert_eq!(h.state().session().centrality(), Centrality::Katz);
    let before = h.state().session().graph().clone();
    h.key_press_modifiers(
        egui::Modifiers::COMMAND | egui::Modifiers::ALT,
        egui::Key::Num1,
    );
    h.run();
    assert_eq!(h.state().session().centrality(), Centrality::PageRank);
    // Cmd-Alt-1 is not taken for Cmd-1, the force-directed layout
    assert_eq!(h.state().session().graph(), &before);
}

#[test]
fn test_iteration_arrows() {
    let mut h = harness();
    h.run();
    h.state_mut()
        .session_mut()
        .set_centrality(Centrality::Seeley);
    h.key_press(egui::Key::ArrowLeft);
    h.run();
    assert_eq!(h.state().session().iteration(), ITERATIONS - 1);
}

/// With no arcs, the eigenvector iterates vanish at k = 1, so the curves
/// have a single sample: hovering them must not panic.
#[test]
fn test_vanished_eigenvector_hover() {
    let (mut h, _, _) = two_node_app();
    h.state_mut()
        .session_mut()
        .set_centrality(Centrality::Eigenvector);
    h.run();
    h.hover_at(egui::pos2(900.0, 300.0));
    h.run();
    h.key_press(egui::Key::ArrowLeft);
    h.run();
    let session = h.state().session();
    assert_eq!(session.iteration(), ITERATIONS - 1);
    assert!(session.scores().iter().all(|&x| x == 0.0));
}

#[test]
fn test_every_centrality_renders() {
    let mut h = harness();
    h.run();
    let (_, after, _) = lecture_example(&h);
    h.state_mut().session_mut().commit(after);
    h.run();
    for c in Centrality::ALL {
        h.state_mut().session_mut().set_centrality(c);
        h.run();
        assert_eq!(h.state().session().ranking().centrality, c);
    }
}

/// Renders the lecture example after the edit that closes {d, e} under each
/// centrality offscreen to `target/screenshots/` for visual inspection. Run
/// with `cargo test --test test_ui -- --ignored`.
#[test]
#[ignore = "needs a GPU"]
fn test_render_centralities() {
    let mut h = wgpu_harness();
    h.run();
    let (before, after, _) = lecture_example(&h);
    h.state_mut().session_mut().commit(before);
    h.state_mut().session_mut().commit(after);
    h.run();
    std::fs::create_dir_all("target/screenshots").unwrap();
    for c in Centrality::ALL {
        h.state_mut().session_mut().set_centrality(c);
        h.run();
        // The lowercase variant name, as in files: pagerank, katz, …
        let name = format!("{c:?}").to_lowercase();
        h.render()
            .unwrap()
            .save(format!("target/screenshots/{name}.png"))
            .unwrap();
    }
}

/// Renders eigenvector centrality on the 4-cycle a → b → c → d → a after
/// deleting d → a, so that the iterates vanish at k = 4, offscreen to
/// `target/screenshots/` for visual inspection: the end labels must sit at 0,
/// next to the note. Run with `cargo test --test test_ui -- --ignored`.
#[test]
#[ignore = "needs a GPU"]
fn test_render_eigenvector_vanishing() {
    let mut h = wgpu_harness();
    h.run();
    h.state_mut()
        .session_mut()
        .set_centrality(Centrality::Eigenvector);
    let mut g = h.state().session().graph().clone();
    let pos = [
        [150.0, 150.0],
        [450.0, 150.0],
        [450.0, 450.0],
        [150.0, 450.0],
    ];
    let ids: Vec<_> = pos.iter().map(|&p| g.add_node(p)).collect();
    for (s, t) in [(0, 1), (1, 2), (2, 3), (3, 0)] {
        g.add_arc(ids[s], ids[t]);
    }
    h.state_mut().session_mut().commit(g.clone());
    g.remove_arc(ids[3], ids[0]);
    h.state_mut().session_mut().commit(g);
    h.run();
    assert_eq!(h.state().session().ranking().vanished, Some(4));
    std::fs::create_dir_all("target/screenshots").unwrap();
    h.render()
        .unwrap()
        .save("target/screenshots/eigenvector_vanishing.png")
        .unwrap();
}
