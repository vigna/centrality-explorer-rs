use egui_kittest::Harness;
use pagerank_explorer::ui::App;

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

use pagerank_explorer::graph::NodeId;

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
