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

use egui::{FontId, Key, KeyboardShortcut, Modifiers, RichText, TextStyle};

use canvas::Animation;
use dialogs::Dialogs;

use crate::Session;
use crate::edit::{Editor, Gesture, Target};
use crate::graph::{Graph, NodeId};
use crate::layouts;
use crate::rank::{Axis, Centrality, MAX_ITERATIONS, Normalization};

/// The application name.
const NAME: &str = "Centrality Explorer";

/// The default canvas size, used before the first frame.
const DEFAULT_CANVAS: [f32; 2] = [640.0, 640.0];

/// The size of the name of the current centrality over the plot.
const TITLE_SIZE: f32 = 32.0;

/// A layout algorithm.
type Layout = fn(&Graph, [f32; 2]) -> Vec<[f32; 2]>;

const NEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const QUIT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Q);
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
/// The modifiers of the shortcuts of the centralities.
const CMD_ALT: Modifiers = Modifiers::COMMAND.plus(Modifiers::ALT);
/// The shortcuts of the centralities, Cmd-Alt-1 to Cmd-Alt-8, in the order of
/// [`Centrality::ALL`].
const CENTRALITIES: [KeyboardShortcut; 8] = [
    KeyboardShortcut::new(CMD_ALT, Key::Num1),
    KeyboardShortcut::new(CMD_ALT, Key::Num2),
    KeyboardShortcut::new(CMD_ALT, Key::Num3),
    KeyboardShortcut::new(CMD_ALT, Key::Num4),
    KeyboardShortcut::new(CMD_ALT, Key::Num5),
    KeyboardShortcut::new(CMD_ALT, Key::Num6),
    KeyboardShortcut::new(CMD_ALT, Key::Num7),
    KeyboardShortcut::new(CMD_ALT, Key::Num8),
];

/// A command from a menu, a shortcut, or a context menu.
#[derive(Clone, Copy, Debug)]
enum Command {
    New,
    Open,
    Save,
    SaveAs,
    Quit,
    Undo,
    Redo,
    Random,
    /// Loads the built-in example with this index.
    Example(usize),
    Layout(Layout),
    Centrality(Centrality),
    Fullscreen,
}

/// Shortcuts and their commands, variants with extra modifiers first
/// (shortcuts match modifiers logically, so Cmd-Z would also match
/// Cmd-Shift-Z, and Cmd-1 would also match Cmd-Alt-1).
const SHORTCUTS: [(KeyboardShortcut, Command); 20] = [
    (SAVE_AS, Command::SaveAs),
    (REDO, Command::Redo),
    (FULLSCREEN, Command::Fullscreen),
    (CENTRALITIES[0], Command::Centrality(Centrality::ALL[0])),
    (CENTRALITIES[1], Command::Centrality(Centrality::ALL[1])),
    (CENTRALITIES[2], Command::Centrality(Centrality::ALL[2])),
    (CENTRALITIES[3], Command::Centrality(Centrality::ALL[3])),
    (CENTRALITIES[4], Command::Centrality(Centrality::ALL[4])),
    (CENTRALITIES[5], Command::Centrality(Centrality::ALL[5])),
    (CENTRALITIES[6], Command::Centrality(Centrality::ALL[6])),
    (CENTRALITIES[7], Command::Centrality(Centrality::ALL[7])),
    (NEW, Command::New),
    (OPEN, Command::Open),
    (SAVE, Command::Save),
    (QUIT, Command::Quit),
    (UNDO, Command::Undo),
    (RANDOM, Command::Random),
    (FORCE, Command::Layout(layouts::force)),
    (CIRCLE, Command::Layout(layouts::circle)),
    (COMPONENTS, Command::Layout(layouts::components)),
];

/// Starts the application on a canvas (web build).
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn start(canvas: web_sys::HtmlCanvasElement) -> Result<(), wasm_bindgen::JsValue> {
    eframe::WebRunner::new()
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(|cc| {
                // The web build cannot open files, so it starts on an example.
                let mut app = App::new(cc);
                app.load_example(0);
                Ok(Box::new(app))
            }),
        )
        .await
}

/// Runs the application.
#[cfg(not(target_arch = "wasm32"))]
pub fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title(NAME),
        // The default macOS menu quits on Cmd-Q without a close request,
        // bypassing the unsaved-changes confirmation; Cmd-Q is handled as a
        // shortcut instead.
        #[cfg(target_os = "macos")]
        event_loop_builder: Some(Box::new(|builder| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            builder.with_default_menu(false);
        })),
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
    /// The drag in progress, with the view frozen for the drag.
    dragging: Option<(canvas::Drag, canvas::View)>,
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

    /// Replaces the graph (undoably) with a built-in example, laid out for the
    /// current canvas size, and animates it out of the canvas center.
    pub fn load_example(&mut self, index: usize) {
        let mut graph = crate::examples::EXAMPLES[index].graph();
        let size = self.canvas_rect.size();
        let pos = crate::layouts::force(&graph, [size.x, size.y]);
        graph.set_positions(&pos);
        self.animation = Some(Animation::new(Default::default(), self.now));
        self.session.commit(graph);
        self.prune();
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
            Command::Centrality(c) => self.session.set_centrality(c),
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
            Command::Quit => self.quit(ctx),
            Command::Random => self.open_random_dialog(),
            Command::Example(index) => self.load_example(index),
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
                self.session.step_param(step);
            }
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let mut command = None;
        let mut item =
            |ui: &mut egui::Ui, text: &str, shortcut: Option<&KeyboardShortcut>, c: Command| {
                let mut button = egui::Button::new(text);
                if let Some(shortcut) = shortcut {
                    button = button.shortcut_text(ui.ctx().format_shortcut(shortcut));
                }
                if ui.add(button).clicked() {
                    command = Some(c);
                }
            };
        let current = self.session.centrality();
        let directed = self.session.graph().is_directed();
        let mut centrality = None;
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                item(ui, "New…", Some(&NEW), Command::New);
                item(ui, "Open…", Some(&OPEN), Command::Open);
                item(ui, "Save", Some(&SAVE), Command::Save);
                item(ui, "Save As…", Some(&SAVE_AS), Command::SaveAs);
                ui.separator();
                item(ui, "Quit", Some(&QUIT), Command::Quit);
            });
            ui.menu_button("Edit", |ui| {
                item(ui, "Undo", Some(&UNDO), Command::Undo);
                item(ui, "Redo", Some(&REDO), Command::Redo);
            });
            ui.menu_button("Graph", |ui| {
                item(ui, "Random Graph…", Some(&RANDOM), Command::Random);
                ui.separator();
                item(
                    ui,
                    "Force-Directed Layout",
                    Some(&FORCE),
                    Command::Layout(layouts::force),
                );
                item(
                    ui,
                    "Circle Layout",
                    Some(&CIRCLE),
                    Command::Layout(layouts::circle),
                );
                item(
                    ui,
                    "Components Layout",
                    Some(&COMPONENTS),
                    Command::Layout(layouts::components),
                );
                ui.separator();
                item(
                    ui,
                    "Rotate 90° Clockwise",
                    None,
                    Command::Layout(layouts::rotate),
                );
                item(
                    ui,
                    "Flip Horizontally",
                    None,
                    Command::Layout(layouts::flip_horizontally),
                );
                item(
                    ui,
                    "Flip Vertically",
                    None,
                    Command::Layout(layouts::flip_vertically),
                );
            });
            ui.menu_button("Examples", |ui| {
                for (index, example) in crate::examples::EXAMPLES.iter().enumerate() {
                    item(ui, example.name, None, Command::Example(index));
                }
            });
            ui.menu_button("Centrality", |ui| {
                centrality = centrality_menu(ui, current, directed);
            });
            ui.menu_button("View", |ui| {
                ui.checkbox(&mut self.fixed_y, "Fixed y-axis [0..1]");
                let mut options = self.session.options();
                let mut l1 = options.normalization == Normalization::L1;
                if ui
                    .checkbox(&mut l1, "ℓ₁ normalization")
                    .on_hover_text(
                        "Normalize every centrality in ℓ₁. Otherwise, each centrality has its \
                         traditional normalization: ℓ₂ for eigenvector centrality; none for the \
                         geometric centralities and Katz's index; ℓ₁ for Seeley's index and \
                         PageRank.",
                    )
                    .changed()
                {
                    options.normalization = if l1 {
                        Normalization::L1
                    } else {
                        Normalization::Traditional
                    };
                    self.session.set_options(options);
                }
                item(ui, "Full Screen", Some(&FULLSCREEN), Command::Fullscreen);
                ui.separator();
                egui::gui_zoom::zoom_menu_buttons(ui);
            });
        });
        if let Some(c) = centrality {
            command = Some(Command::Centrality(c));
        }
        if let Some(command) = command {
            self.execute(ui.ctx(), command);
        }
    }

    /// Shows the name of the current centrality, large enough to be read
    /// from the back of a room.
    fn title(&self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(
                    self.session
                        .centrality()
                        .name(self.session.graph().is_directed()),
                )
                .size(TITLE_SIZE)
                .strong(),
            );
        });
    }

    /// Shows the slider of the parameter of the current centrality, if
    /// any, labeled with its value and followed, for Katz's index, by ρ, and
    /// for eigenvector centrality and Seeley's index, by the switch of the
    /// Cesàro means of the iterates and by the number *K* of iterations.
    fn param_slider(&mut self, ui: &mut egui::Ui) {
        let Some(mut x) = self.session.param() else {
            return;
        };
        let ranking = self.session.ranking();
        let (axis, rho) = (ranking.axis, ranking.rho);
        let katz = ranking.centrality == Centrality::Katz;
        ui.horizontal(|ui| {
            let label = ui.heading(param_label(axis, x));
            // The label is padded to its widest value, so that the slider
            // does not move under the pointer as the label changes.
            let widest = ui
                .painter()
                .layout_no_wrap(
                    param_label(axis, axis.x_max()),
                    TextStyle::Heading.resolve(ui.style()),
                    egui::Color32::BLACK,
                )
                .size()
                .x;
            ui.add_space((widest - label.rect.width()).max(0.0));
            if katz && let Some(rho) = rho {
                ui.add_space(ui.spacing().item_spacing.x);
                ui.heading(format!("ρ = {rho:.3}"));
            }
            if let Axis::Iteration(_) = axis {
                ui.add_space(ui.spacing().item_spacing.x);
                let mut options = self.session.options();
                let mean = ui
                    .checkbox(&mut options.mean, "Cesàro means")
                    .on_hover_text(
                        "Show, at each k, the mean of the first k + 1 iterates of the power \
                         method (on A/ρ for eigenvector centrality), which converges to the \
                         centrality even when the iterates do not",
                    );
                let iterations = ui
                    .add(
                        egui::DragValue::new(&mut options.iterations)
                            .range(1..=MAX_ITERATIONS)
                            .prefix("K = "),
                    )
                    .on_hover_text("The number of iterations of the power method");
                if mean.changed() || iterations.changed() {
                    self.session.set_options(options);
                }
            }
            ui.spacing_mut().slider_width = ui.available_width() - 8.0;
            let slider = match axis {
                Axis::Iteration(_) => egui::Slider::new(&mut x, 0.0..=axis.x_max()).integer(),
                _ => egui::Slider::new(&mut x, 0.0..=1.0),
            };
            if ui
                .add(slider.show_value(false))
                .labelled_by(label.id)
                .changed()
            {
                self.session.set_param(x);
            }
        });
    }
}

/// Returns the label of the slider of a parameter with value `x` on an
/// axis: "k = *x*" for iterations, and the label of the axis followed by
/// *x* with two decimals otherwise.
fn param_label(axis: Axis, x: f64) -> String {
    match axis {
        Axis::Iteration(_) => format!("k = {x:.0}"),
        _ => format!("{} = {x:.2}", axis.label()),
    }
}

/// Shows one selectable item per centrality, named as on a directed or an
/// undirected graph, and grouped by family, with the current one checked and
/// the shortcut of each; returns the centrality clicked, if any.
fn centrality_menu(ui: &mut egui::Ui, current: Centrality, directed: bool) -> Option<Centrality> {
    let mut clicked = None;
    let mut family = None;
    for (c, shortcut) in Centrality::ALL.into_iter().zip(&CENTRALITIES) {
        if family.is_some_and(|f| f != c.family()) {
            ui.separator();
        }
        family = Some(c.family());
        let shortcut = ui.ctx().format_shortcut(shortcut);
        if ui
            .add(egui::Button::selectable(c == current, c.name(directed)).shortcut_text(shortcut))
            .clicked()
        {
            clicked = Some(c);
        }
    }
    clicked
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
                egui::Panel::top("centrality").show(ui, |ui| self.title(ui));
                if self.session.param().is_some() {
                    egui::Panel::bottom("param").show(ui, |ui| self.param_slider(ui));
                }
                egui::CentralPanel::default().show(ui, |ui| plot::plot(self, ui));
            });
        egui::CentralPanel::default().show(ui, |ui| canvas::canvas(self, ui));
        if !self.hover_seen {
            self.hovered = None;
        }
        self.dialogs(&ctx);
    }
}
