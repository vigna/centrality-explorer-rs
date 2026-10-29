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
