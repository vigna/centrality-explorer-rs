//! Dialogs, file commands, and the window title.

use std::path::Path;

use egui::ViewportCommand;

use super::{App, NAME};
use crate::Session;
use crate::graph::Graph;
use crate::random::ChungLu;

/// The file-dialog filter name for graph files.
const FILTER: &str = "PageRank Explorer graph";

/// Returns the window title for a session file and its unsaved state.
pub fn window_title(path: Option<&Path>, dirty: bool) -> String {
    match path.and_then(Path::file_name) {
        None => NAME.to_owned(),
        Some(name) => format!(
            "{NAME} — {}{}",
            name.to_string_lossy(),
            if dirty { "*" } else { "" }
        ),
    }
}

/// An action waiting for the unsaved-changes confirmation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Pending {
    Open,
    Close,
}

/// The state of the random-graph dialog.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RandomDialog {
    params: ChungLu,
    seed: u64,
}

/// The state of all dialogs.
#[derive(Debug, Default)]
pub(crate) struct Dialogs {
    /// The new-graph dialog, with the selected directedness, if open.
    new_graph: Option<bool>,
    /// The random-graph dialog, if open.
    random: Option<RandomDialog>,
    /// The parameters last used for a random graph.
    last_params: Option<ChungLu>,
    /// An error message, if one is being shown.
    error: Option<String>,
    /// An action waiting for the unsaved-changes confirmation.
    confirm: Option<Pending>,
    /// Whether closing the window has been confirmed.
    allow_close: bool,
    /// The last title sent to the window.
    title: String,
}

/// Returns a short seed from the system clock.
fn clock_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_micros() as u64 % 1_000_000)
}

impl App {
    /// Generates a random graph, lays it out for the current canvas, commits
    /// it (undoably), and animates it out of the canvas center.
    pub fn generate_random(&mut self, params: ChungLu, seed: u64) {
        let mut graph = params.generate(seed);
        let size = self.canvas_rect.size();
        let pos = crate::layouts::force(&graph, [size.x, size.y]);
        graph.set_positions(&pos);
        self.animation = Some(super::Animation::new(Default::default(), self.now));
        self.session.commit(graph);
        self.prune();
    }

    pub(crate) fn open_new_graph_dialog(&mut self) {
        self.dialogs.new_graph = Some(true);
    }

    pub(crate) fn open_random_dialog(&mut self) {
        self.dialogs.random = Some(RandomDialog {
            params: self.dialogs.last_params.unwrap_or_default(),
            seed: clock_seed(),
        });
    }

    /// Quits, asking for confirmation if there are unsaved changes.
    pub(crate) fn quit(&mut self, ctx: &egui::Context) {
        if self.session.is_dirty() {
            self.dialogs.confirm = Some(Pending::Close);
        } else {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }

    /// Opens a file, asking for confirmation if there are unsaved changes.
    pub(crate) fn open(&mut self) {
        if self.session.is_dirty() {
            self.dialogs.confirm = Some(Pending::Open);
        } else {
            self.open_file();
        }
    }

    fn open_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter(FILTER, &["json"])
            .pick_file()
        else {
            return;
        };
        match Session::open(&path) {
            Ok(session) => {
                self.session = session;
                self.animation = None;
                self.dragging = None;
                self.editor.cancel();
                self.prune();
            }
            Err(e) => self.dialogs.error = Some(format!("{e:#}")),
        }
    }

    /// Saves to the session file, asking for one if there is none; returns
    /// whether the session was saved.
    pub(crate) fn save(&mut self) -> bool {
        if self.session.path().is_none() {
            return self.save_as();
        }
        match self.session.save() {
            Ok(()) => true,
            Err(e) => {
                self.dialogs.error = Some(format!("{e:#}"));
                false
            }
        }
    }

    /// Saves to a file chosen by the user; returns whether the session was
    /// saved.
    pub(crate) fn save_as(&mut self) -> bool {
        let Some(path) = rfd::FileDialog::new()
            .add_filter(FILTER, &["json"])
            .set_file_name("graph.json")
            .save_file()
        else {
            return false;
        };
        match self.session.save_as(&path) {
            Ok(()) => true,
            Err(e) => {
                self.dialogs.error = Some(format!("{e:#}"));
                false
            }
        }
    }

    /// Shows the open dialogs, handles window-close requests, and keeps the
    /// window title up to date.
    pub(crate) fn dialogs(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested())
            && self.session.is_dirty()
            && !self.dialogs.allow_close
        {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.dialogs.confirm = Some(Pending::Close);
        }

        let title = window_title(self.session.path(), self.session.is_dirty());
        if title != self.dialogs.title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.dialogs.title = title;
        }

        self.new_graph_dialog(ctx);
        self.random_dialog(ctx);
        self.confirm_dialog(ctx);

        if let Some(error) = self.dialogs.error.clone() {
            let mut open = true;
            egui::Window::new("Error")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label(error);
                    if ui.button("OK").clicked() {
                        self.dialogs.error = None;
                    }
                });
            if !open {
                self.dialogs.error = None;
            }
        }
    }

    fn new_graph_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut directed) = self.dialogs.new_graph else {
            return;
        };
        let mut close = false;
        egui::Window::new("New Graph")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.radio_value(&mut directed, true, "Directed");
                ui.radio_value(&mut directed, false, "Undirected");
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        self.session.commit(Graph::new(directed));
                        self.editor.cancel();
                        self.prune();
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });
        self.dialogs.new_graph = if close { None } else { Some(directed) };
    }

    fn random_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.dialogs.random else {
            return;
        };
        let mut open = true;
        let mut generate = false;
        egui::Window::new("Random Graph")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                egui::Grid::new("random").num_columns(2).show(ui, |ui| {
                    let label = ui.label("Nodes");
                    ui.add(egui::DragValue::new(&mut dialog.params.nodes).range(2..=100))
                        .labelled_by(label.id);
                    ui.end_row();
                    let label = ui.label("Average degree");
                    ui.add(
                        egui::DragValue::new(&mut dialog.params.avg_degree)
                            .range(0.5..=10.0)
                            .speed(0.1)
                            .fixed_decimals(1),
                    )
                    .labelled_by(label.id);
                    ui.end_row();
                    let label = ui.label("Exponent");
                    ui.add(
                        egui::DragValue::new(&mut dialog.params.exponent)
                            .range(2.0..=4.0)
                            .speed(0.1)
                            .fixed_decimals(1),
                    )
                    .labelled_by(label.id);
                    ui.end_row();
                    let label = ui.label("Seed");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut dialog.seed))
                            .labelled_by(label.id);
                        if ui.button("New Seed").clicked() {
                            dialog.seed = clock_seed();
                        }
                    });
                    ui.end_row();
                });
                if ui.button("Generate").clicked() {
                    generate = true;
                }
            });
        if generate {
            self.dialogs.last_params = Some(dialog.params);
            self.generate_random(dialog.params, dialog.seed);
        }
        self.dialogs.random = open.then_some(dialog);
    }

    fn confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.dialogs.confirm else {
            return;
        };
        let mut choice = None;
        egui::Window::new("Unsaved Changes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label("The graph has unsaved changes.");
                ui.horizontal(|ui| {
                    for (text, value) in [("Save", 0), ("Discard", 1), ("Cancel", 2)] {
                        if ui.button(text).clicked() {
                            choice = Some(value);
                        }
                    }
                });
            });
        let proceed = match choice {
            None => return,
            Some(0) => self.save(),
            Some(1) => true,
            _ => false,
        };
        self.dialogs.confirm = None;
        if proceed {
            match pending {
                Pending::Open => self.open_file(),
                Pending::Close => {
                    self.dialogs.allow_close = true;
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_window_title() {
        assert_eq!(window_title(None, true), "PageRank Explorer");
        assert_eq!(
            window_title(Some(Path::new("/x/lecture.json")), false),
            "PageRank Explorer — lecture.json"
        );
        assert_eq!(
            window_title(Some(Path::new("/x/lecture.json")), true),
            "PageRank Explorer — lecture.json*"
        );
    }
}
