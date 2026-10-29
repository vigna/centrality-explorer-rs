//! The user interface.
//!
//! The [`App`] reads the [`Session`] every frame and sends edits back as new
//! graph snapshots; it keeps only presentation state (gesture in progress,
//! hovered node, animations, dialogs).

mod canvas;
mod dialogs;
mod geometry;
mod palette;
mod plot;

use std::collections::HashMap;

use egui::{FontId, Key, KeyboardShortcut, Modifiers, TextStyle};

use canvas::Animation;
use dialogs::Dialogs;

use crate::Session;
use crate::edit::{Editor, Gesture, Target};
use crate::graph::{Graph, NodeId};
use crate::layouts;

/// The application name.
const NAME: &str = "PageRank Explorer";

/// The default canvas size, used before the first frame.
const DEFAULT_CANVAS: [f32; 2] = [640.0, 640.0];

/// A layout algorithm.
type Layout = fn(&Graph, [f32; 2]) -> Vec<[f32; 2]>;

const NEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const SAVE_AS: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::S);
const UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
const REDO: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z);
const RANDOM: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::R);
const FORCE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Num1);
const CIRCLE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Num2);
const COMPONENTS: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Num3);
const FULLSCREEN: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::F);

/// A command from a menu, a shortcut, or a context menu.
#[derive(Clone, Copy, Debug)]
enum Command {
    New,
    Open,
    Save,
    SaveAs,
    Undo,
    Redo,
    Random,
    Layout(Layout),
    Fullscreen,
}

/// Shortcuts and their commands, shifted variants first (shortcuts match
/// modifiers logically, so Cmd-Z would also match Cmd-Shift-Z).
const SHORTCUTS: [(KeyboardShortcut, Command); 11] = [
    (SAVE_AS, Command::SaveAs),
    (REDO, Command::Redo),
    (FULLSCREEN, Command::Fullscreen),
    (NEW, Command::New),
    (OPEN, Command::Open),
    (SAVE, Command::Save),
    (UNDO, Command::Undo),
    (RANDOM, Command::Random),
    (FORCE, Command::Layout(layouts::force)),
    (CIRCLE, Command::Layout(layouts::circle)),
    (COMPONENTS, Command::Layout(layouts::components)),
];

/// Runs the application.
pub fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title(NAME),
        ..Default::default()
    };
    eframe::run_native(NAME, options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}

/// The application.
pub struct App {
    session: Session,
    editor: Editor,
    /// The node under the pointer, on the canvas or (via its curve) on the
    /// plot.
    hovered: Option<NodeId>,
    /// Whether the canvas or the plot handled the pointer this frame.
    hover_seen: bool,
    /// The node being dragged, with the offset of the pointer from its
    /// center.
    dragging: Option<(NodeId, egui::Vec2)>,
    /// The target and canvas-local position of the last right click.
    context: Option<(Target, [f32; 2])>,
    animation: Option<Animation>,
    dialogs: Dialogs,
    fixed_y: bool,
    canvas_rect: egui::Rect,
    /// The time of the current frame, in seconds.
    now: f64,
}

impl App {
    /// Creates the application, setting up a light theme with large fonts.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = &cc.egui_ctx;
        ctx.set_theme(egui::Theme::Light);
        ctx.all_styles_mut(|style| {
            style.text_styles = [
                (TextStyle::Heading, FontId::proportional(26.0)),
                (TextStyle::Body, FontId::proportional(20.0)),
                (TextStyle::Button, FontId::proportional(20.0)),
                (TextStyle::Monospace, FontId::monospace(18.0)),
                (TextStyle::Small, FontId::proportional(15.0)),
            ]
            .into();
        });
        Self {
            session: Session::new(true),
            editor: Editor::default(),
            hovered: None,
            hover_seen: false,
            dragging: None,
            context: None,
            animation: None,
            dialogs: Dialogs::default(),
            fixed_y: false,
            canvas_rect: egui::Rect::from_min_size(egui::Pos2::ZERO, DEFAULT_CANVAS.into()),
            now: 0.0,
        }
    }

    /// Returns the session.
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// Returns the session, mutably.
    pub fn session_mut(&mut self) -> &mut Session {
        &mut self.session
    }

    /// Returns the canvas rectangle of the last frame.
    pub fn canvas_rect(&self) -> egui::Rect {
        self.canvas_rect
    }

    /// Lays out the graph for the current canvas size, commits the new
    /// positions, and animates the nodes to them.
    pub fn apply_layout(&mut self, layout: Layout) {
        let size = self.canvas_rect.size();
        let pos = layout(self.session.graph(), [size.x, size.y]);
        let mut graph = self.session.graph().clone();
        graph.set_positions(&pos);
        self.animate_from_current();
        self.session.commit(graph);
    }

    /// Starts an animation from the current node positions.
    fn animate_from_current(&mut self) {
        let from: HashMap<NodeId, [f32; 2]> = self
            .session
            .graph()
            .nodes()
            .iter()
            .map(|node| (node.id, node.pos))
            .collect();
        self.animation = Some(Animation::new(from, self.now));
    }

    /// Drops references to nodes that no longer exist.
    fn prune(&mut self) {
        self.editor.prune(self.session.graph());
        if let Some(id) = self.hovered
            && self.session.graph().node(id).is_none()
        {
            self.hovered = None;
        }
    }

    fn execute(&mut self, ctx: &egui::Context, command: Command) {
        match command {
            Command::Undo => {
                self.session.undo();
                self.prune();
            }
            Command::Redo => {
                self.session.redo();
                self.prune();
            }
            Command::Layout(layout) => self.apply_layout(layout),
            Command::Fullscreen => {
                let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
            }
            Command::New => self.open_new_graph_dialog(),
            Command::Open => self.open(),
            Command::Save => {
                self.save();
            }
            Command::SaveAs => {
                self.save_as();
            }
            Command::Random => self.open_random_dialog(),
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        let commands: Vec<Command> = ctx.input_mut(|i| {
            SHORTCUTS
                .iter()
                .filter(|(shortcut, _)| i.consume_shortcut(shortcut))
                .map(|&(_, command)| command)
                .collect()
        });
        for command in commands {
            self.execute(ctx, command);
        }

        let renaming = matches!(self.editor.gesture(), Gesture::Renaming { .. });
        if !renaming && !ctx.egui_wants_keyboard_input() {
            let step = ctx.input_mut(|i| {
                let mut step = 0;
                step += 5 * i.count_and_consume_key(Modifiers::SHIFT, Key::ArrowRight) as i32;
                step -= 5 * i.count_and_consume_key(Modifiers::SHIFT, Key::ArrowLeft) as i32;
                step += i.count_and_consume_key(Modifiers::NONE, Key::ArrowRight) as i32;
                step -= i.count_and_consume_key(Modifiers::NONE, Key::ArrowLeft) as i32;
                step
            });
            if step != 0 {
                let alpha = ((self.session.alpha() * 100.0).round() + step as f64) / 100.0;
                self.session.set_alpha(alpha);
            }
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let mut command = None;
        let mut item = |ui: &mut egui::Ui, text: &str, shortcut: &KeyboardShortcut, c: Command| {
            let shortcut = ui.ctx().format_shortcut(shortcut);
            if ui
                .add(egui::Button::new(text).shortcut_text(shortcut))
                .clicked()
            {
                command = Some(c);
            }
        };
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                item(ui, "New…", &NEW, Command::New);
                item(ui, "Open…", &OPEN, Command::Open);
                item(ui, "Save", &SAVE, Command::Save);
                item(ui, "Save As…", &SAVE_AS, Command::SaveAs);
            });
            ui.menu_button("Edit", |ui| {
                item(ui, "Undo", &UNDO, Command::Undo);
                item(ui, "Redo", &REDO, Command::Redo);
            });
            ui.menu_button("Graph", |ui| {
                item(ui, "Random Graph…", &RANDOM, Command::Random);
                ui.separator();
                item(
                    ui,
                    "Force-Directed Layout",
                    &FORCE,
                    Command::Layout(layouts::force),
                );
                item(
                    ui,
                    "Circle Layout",
                    &CIRCLE,
                    Command::Layout(layouts::circle),
                );
                item(
                    ui,
                    "Components Layout",
                    &COMPONENTS,
                    Command::Layout(layouts::components),
                );
            });
            ui.menu_button("View", |ui| {
                ui.checkbox(&mut self.fixed_y, "Fixed y-axis [0, 1]");
                item(ui, "Full Screen", &FULLSCREEN, Command::Fullscreen);
                ui.separator();
                egui::gui_zoom::zoom_menu_buttons(ui);
            });
        });
        if let Some(command) = command {
            self.execute(ui.ctx(), command);
        }
    }

    fn alpha_slider(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let label = ui.heading(format!("α = {:.2}", self.session.alpha()));
            ui.spacing_mut().slider_width = ui.available_width() - 8.0;
            let mut alpha = self.session.alpha();
            if ui
                .add(egui::Slider::new(&mut alpha, 0.0..=1.0).show_value(false))
                .labelled_by(label.id)
                .changed()
            {
                self.session.set_alpha(alpha);
            }
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.now = ctx.input(|i| i.time);
        self.hover_seen = false;
        self.handle_keys(&ctx);
        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        let half = ui.available_width() / 2.0;
        egui::Panel::right("plot")
            .exact_size(half)
            .resizable(false)
            .show(ui, |ui| {
                egui::Panel::bottom("alpha").show(ui, |ui| self.alpha_slider(ui));
                egui::CentralPanel::default().show(ui, |ui| plot::plot(self, ui));
            });
        egui::CentralPanel::default().show(ui, |ui| canvas::canvas(self, ui));
        if !self.hover_seen {
            self.hovered = None;
        }
        self.dialogs(&ctx);
    }
}
