/*
 * SPDX-FileCopyrightText: 2026 Sebastiano Vigna
 *
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 */

//! The editing session.

use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::files::{self, Settings};
use crate::graph::{Graph, NodeId};
use crate::rank::{Centrality, MAX_ITERATIONS, Normalization, Options, Ranking};

/// An editing session: the current graph, its undo/redo history, its
/// curves for the current centrality, and the curves to compare them with
/// (from before the last edit, or of the previous centrality).
///
/// The session is the single source of truth of the application: every
/// change is a new graph snapshot passed to [`commit`](Self::commit) (or
/// restored by [`undo`](Self::undo)/[`redo`](Self::redo)), and the ranking is
/// recomputed once per change, only if the structure of the graph changed.
///
/// The curves to compare with (the *ghost*) follow the last action. After a
/// change of graph: when the new graph has a different generation (a new,
/// loaded, or random graph), there is no ghost; when it has the same
/// generation but a different structure, the previous ranking, that is, the
/// current centrality on the graph before the edit, becomes the ghost;
/// otherwise (moves and renames) ranking and ghost are unchanged.
///
/// After a switch of centrality with [ℓ1 normalization](Normalization::L1),
/// under which scores are shares, on the same scale, the ghost is the
/// previous centrality on the current graph: its curves, if it belongs to the
/// same [family](crate::rank::Family) as the new one, so that the curves
/// share the x-axis, or if it is geometric, so that its curves are flat; and
/// otherwise its scores at its current parameter, as a
/// [frozen](Ranking::at) ranking, whose curves are flat. With traditional
/// normalization, the ghost is again the new centrality on the graph before
/// the last structural edit, which the session keeps for this purpose; the
/// same happens to the ghost of a previous centrality when switching to
/// traditional normalization.
///
/// Each centrality has its own parameter: the iteration *k* for eigenvector
/// centrality and Seeley's index, the parameter βρ (or β) for Katz's index,
/// and the damping factor α for PageRank. The [options](Options) of the
/// rankings (normalization, number of iterations, and means) are shared by
/// all centralities. Switching centrality and changing parameters or options
/// are neither undoable nor unsaved changes.
#[derive(Debug)]
pub struct Session {
    graph: Graph,
    ranking: Ranking,
    ghost: Option<Ranking>,
    /// The graph before the last structural edit, if any; unless
    /// `compared` is set, the ghost is its ranking.
    before: Option<Graph>,
    /// The previous centrality, if the last action was a switch of
    /// centrality within a family: the ghost is its ranking of the current
    /// graph.
    compared: Option<Centrality>,
    scores: Vec<f64>,
    shares: Vec<f64>,
    centrality: Centrality,
    options: Options,
    iteration: usize,
    katz: f64,
    alpha: f64,
    undo: Vec<Graph>,
    redo: Vec<Graph>,
    drag_origin: Option<Graph>,
    path: Option<PathBuf>,
    /// The generation of the graph associated with `path`: other
    /// generations (new or random graphs) are not associated with the file.
    path_generation: u64,
    dirty: bool,
}

impl Session {
    /// Creates a session with an empty graph.
    pub fn new(directed: bool) -> Self {
        Self::with_graph(Graph::new(directed), Settings::default(), None)
    }

    /// Creates a session with the given graph, settings, and file path, the
    /// [default](Options::default) options, an empty history, and no unsaved
    /// changes.
    ///
    /// The iteration starts at the number *K* of iterations.
    pub fn with_graph(graph: Graph, settings: Settings, path: impl Into<Option<PathBuf>>) -> Self {
        let options = Options::default();
        let ranking = Ranking::with_options(&graph, settings.centrality, options);
        let mut session = Self {
            path_generation: graph.generation(),
            graph,
            ranking,
            ghost: None,
            before: None,
            compared: None,
            scores: Vec::new(),
            shares: Vec::new(),
            centrality: settings.centrality,
            options,
            katz: settings.katz.clamp(0.0, 1.0),
            alpha: settings.alpha.clamp(0.0, 1.0),
            iteration: options.iterations,
            undo: Vec::new(),
            redo: Vec::new(),
            drag_origin: None,
            path: path.into(),
            dirty: false,
        };
        session.refresh_scores();
        session
    }

    /// Returns the current graph.
    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    /// Returns the curves of the current graph.
    pub fn ranking(&self) -> &Ranking {
        &self.ranking
    }

    /// Returns the curves from before the last edit, if any.
    pub fn ghost(&self) -> Option<&Ranking> {
        self.ghost.as_ref()
    }

    /// Returns the scores of the current centrality at the current
    /// parameter, aligned with the identifiers of [`ranking`](Self::ranking).
    pub fn scores(&self) -> &[f64] {
        &self.scores
    }

    /// Returns the [shares](Ranking::shares) of the current centrality at the
    /// current parameter, aligned with the identifiers of
    /// [`ranking`](Self::ranking).
    pub fn shares(&self) -> &[f64] {
        &self.shares
    }

    /// Returns the settings to be saved with the graph.
    pub fn settings(&self) -> Settings {
        Settings {
            centrality: self.centrality,
            katz: self.katz,
            alpha: self.alpha,
        }
    }

    /// Returns the current centrality.
    pub fn centrality(&self) -> Centrality {
        self.centrality
    }

    /// Switches to another centrality, recomputing ranking, ghost, and
    /// scores: the ghost is the previous centrality on the current graph, with
    /// ℓ1 normalization, and the new centrality on the graph before the last
    /// edit otherwise (see the documentation of [`Session`]).
    ///
    /// This is neither undoable nor an unsaved change.
    pub fn set_centrality(&mut self, c: Centrality) {
        if c == self.centrality {
            return;
        }
        let previous = std::mem::replace(&mut self.centrality, c);
        self.compared = (self.options.normalization == Normalization::L1).then_some(previous);
        self.rerank();
    }

    /// Returns the options of the rankings.
    pub fn options(&self) -> Options {
        self.options
    }

    /// Sets the options of the rankings, with the number *K* of iterations
    /// clamped to [1 . . [`MAX_ITERATIONS`]], recomputing ranking, ghost, and
    /// scores.
    ///
    /// The iteration *k* is clamped to *K*, and if it was at the previous
    /// *K*, it moves to the new one. This is neither undoable nor an unsaved
    /// change.
    pub fn set_options(&mut self, options: Options) {
        let options = Options {
            iterations: options.iterations.clamp(1, MAX_ITERATIONS),
            ..options
        };
        if options == self.options {
            return;
        }
        if self.iteration == self.options.iterations || self.iteration > options.iterations {
            self.iteration = options.iterations;
        }
        self.options = options;
        if options.normalization != Normalization::L1 {
            self.compared = None;
        }
        self.rerank();
    }

    /// Returns the current damping factor.
    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    /// Sets the damping factor, clamped to [0 . . 1].
    ///
    /// This is neither undoable nor an unsaved change.
    pub fn set_alpha(&mut self, alpha: f64) {
        let alpha = alpha.clamp(0.0, 1.0);
        if alpha != self.alpha {
            self.alpha = alpha;
            if self.centrality == Centrality::PageRank {
                self.refresh_scores();
            }
        }
    }

    /// Returns the current Katz parameter βρ (or β).
    pub fn katz(&self) -> f64 {
        self.katz
    }

    /// Returns the current iteration *k* of eigenvector centrality and
    /// Seeley's index.
    pub fn iteration(&self) -> usize {
        self.iteration
    }

    /// Returns the parameter of the current centrality: *k* for eigenvector
    /// centrality and Seeley's index, βρ (or β) for Katz's index, α for
    /// PageRank, and `None` for the others.
    pub fn param(&self) -> Option<f64> {
        self.param_of(self.centrality)
    }

    /// Returns the parameter of a centrality (see [`param`](Self::param)).
    fn param_of(&self, c: Centrality) -> Option<f64> {
        match c {
            Centrality::PageRank => Some(self.alpha),
            Centrality::Katz => Some(self.katz),
            Centrality::Eigenvector | Centrality::Seeley => Some(self.iteration as f64),
            _ => None,
        }
    }

    /// Sets the parameter of the current centrality.
    ///
    /// α and the Katz parameter are clamped to [0 . . 1]; *k* is rounded and
    /// clamped to [0 . . *K*], where *K* is the number of iterations. Does
    /// nothing for centralities without parameter. This is neither undoable
    /// nor an unsaved change.
    pub fn set_param(&mut self, x: f64) {
        let old = self.param();
        match self.centrality {
            Centrality::PageRank => self.alpha = x.clamp(0.0, 1.0),
            Centrality::Katz => self.katz = x.clamp(0.0, 1.0),
            Centrality::Eigenvector | Centrality::Seeley => {
                self.iteration = x.round().clamp(0.0, self.options.iterations as f64) as usize;
            }
            _ => return,
        }
        if self.param() != old {
            self.refresh_scores();
        }
    }

    /// Moves the parameter of the current centrality by a number of steps:
    /// 0.01 for α and the Katz parameter, 1 for *k*.
    pub fn step_param(&mut self, steps: i32) {
        if let Some(v) = self.param() {
            self.set_param(match self.centrality {
                Centrality::Katz | Centrality::PageRank => {
                    ((v * 100.0).round() + f64::from(steps)) / 100.0
                }
                _ => v + f64::from(steps),
            });
        }
    }

    /// Returns the file associated with the current graph, if any.
    ///
    /// The association holds only for the generation of graph that was
    /// opened or saved: after a new or random graph replaces it, there is no
    /// file until the next save (undoing back to it restores the
    /// association).
    pub fn path(&self) -> Option<&Path> {
        self.path
            .as_deref()
            .filter(|_| self.graph.generation() == self.path_generation)
    }

    /// Returns whether there are unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Returns whether there is something to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Returns whether there is something to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Replaces the current graph with a new one, recording the current one
    /// for undo. Does nothing if the new graph equals the current one.
    pub fn commit(&mut self, new: Graph) {
        if new == self.graph {
            return;
        }
        self.undo.push(self.graph.clone());
        self.redo.clear();
        self.install(new);
        self.dirty = true;
    }

    /// Restores the graph before the last change; returns whether there was
    /// one.
    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo.push(self.graph.clone());
        self.install(previous);
        self.dirty = true;
        true
    }

    /// Reapplies the last undone change; returns whether there was one.
    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(self.graph.clone());
        self.install(next);
        self.dirty = true;
        true
    }

    /// Starts dragging nodes: positions can then be changed in place with
    /// [`drag_to`](Self::drag_to), and [`end_drag`](Self::end_drag) records
    /// a single undo step.
    pub fn begin_drag(&mut self) {
        self.drag_origin = Some(self.graph.clone());
    }

    /// Moves a node in place, without recording history.
    pub fn drag_to(&mut self, id: NodeId, pos: [f32; 2]) {
        self.graph.set_pos(id, pos);
    }

    /// Ends a drag, recording one undo step if any position changed.
    pub fn end_drag(&mut self) {
        if let Some(origin) = self.drag_origin.take()
            && origin != self.graph
        {
            self.undo.push(origin);
            self.redo.clear();
            self.dirty = true;
        }
    }

    /// Creates a session from a file, with an empty history.
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        let (graph, settings) = files::load(path)?;
        Ok(Self::with_graph(graph, settings, path.to_path_buf()))
    }

    /// Saves the session to its file.
    pub fn save(&mut self) -> anyhow::Result<()> {
        let path = self
            .path()
            .context("the graph is not associated with a file")?
            .to_path_buf();
        self.save_as(path)
    }

    /// Saves the session to a file, which becomes the session's file.
    pub fn save_as(&mut self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let path = path.as_ref();
        files::save(&self.graph, &self.settings(), path)?;
        self.mark_saved(path.to_path_buf());
        Ok(())
    }

    /// Associates the session with a file and marks it as saved.
    pub fn mark_saved(&mut self, path: PathBuf) {
        self.path = Some(path);
        self.path_generation = self.graph.generation();
        self.dirty = false;
    }

    /// Recomputes scores and shares from the ranking and the current
    /// parameter.
    fn refresh_scores(&mut self) {
        let x = self.param().unwrap_or(0.0);
        self.scores = self.ranking.scores(x);
        self.shares = self.ranking.shares(x);
    }

    /// Returns the ranking of a graph for the current centrality and options.
    fn rank(&self, graph: &Graph) -> Ranking {
        Ranking::with_options(graph, self.centrality, self.options)
    }

    /// Returns the ghost of a previous centrality on the current graph: its
    /// ranking, if it belongs to the family of the current one or has no
    /// parameter, and its ranking [frozen](Ranking::at) at its parameter
    /// otherwise.
    fn previous(&self, c: Centrality) -> Ranking {
        let ranking = Ranking::with_options(&self.graph, c, self.options);
        match self.param_of(c) {
            Some(x) if c.family() != self.centrality.family() => ranking.at(x),
            _ => ranking,
        }
    }

    /// Recomputes ranking, ghost (see the documentation of [`Session`]), and
    /// scores.
    fn rerank(&mut self) {
        self.ranking = self.rank(&self.graph);
        self.ghost = match self.compared {
            Some(c) => Some(self.previous(c)),
            None => self.before.as_ref().map(|g| self.rank(g)),
        };
        self.refresh_scores();
    }

    /// Makes a graph current, updating ranking and ghost (see the
    /// documentation of [`Session`]).
    fn install(&mut self, new: Graph) {
        if new.generation() != self.graph.generation() {
            self.ranking = self.rank(&new);
            self.ghost = None;
            self.before = None;
            self.compared = None;
        } else if !new.same_structure(&self.graph) {
            let ranking = self.rank(&new);
            self.ghost = Some(std::mem::replace(&mut self.ranking, ranking));
            self.before = Some(self.graph.clone());
            self.compared = None;
        }
        self.graph = new;
        self.refresh_scores();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Graph, NodeId};
    use crate::rank::{Axis, DEFAULT_ITERATIONS};

    fn two_nodes() -> (Session, NodeId, NodeId) {
        let mut s = Session::new(true);
        let mut g = s.graph().clone();
        let (a, b) = (g.add_node([0.0; 2]), g.add_node([100.0, 0.0]));
        s.commit(g);
        (s, a, b)
    }

    fn with_arc(s: &Session, a: NodeId, b: NodeId) -> Graph {
        let mut g = s.graph().clone();
        g.toggle_arc(a, b);
        g
    }

    #[test]
    fn test_commit_undo_redo() {
        let (mut s, a, b) = two_nodes();
        s.commit(with_arc(&s, a, b));
        assert!(s.graph().has_arc(a, b));
        assert!(s.undo());
        assert!(!s.graph().has_arc(a, b));
        assert!(s.redo());
        assert!(s.graph().has_arc(a, b));
    }

    #[test]
    fn test_commit_identical_is_noop() {
        let mut s = Session::new(true);
        s.commit(s.graph().clone());
        assert!(!s.can_undo());
        assert!(!s.is_dirty());
    }

    #[test]
    fn test_redo_cleared_on_commit() {
        let (mut s, a, b) = two_nodes();
        s.commit(with_arc(&s, a, b));
        s.undo();
        s.commit(with_arc(&s, b, a));
        assert!(!s.can_redo());
    }

    #[test]
    fn test_ghost_after_structural_edit() {
        let (mut s, a, b) = two_nodes();
        let before = s.ranking().curve(a).unwrap().to_vec();
        s.commit(with_arc(&s, a, b));
        assert_eq!(s.ghost().unwrap().curve(a).unwrap(), &before[..]);
    }

    #[test]
    fn test_ghost_kept_on_move() {
        let (mut s, a, b) = two_nodes();
        s.commit(with_arc(&s, a, b));
        let ghost = s.ghost().unwrap().curve(a).unwrap().to_vec();
        let mut g = s.graph().clone();
        g.set_pos(a, [50.0, 50.0]);
        s.commit(g);
        assert_eq!(s.ghost().unwrap().curve(a).unwrap(), &ghost[..]);
    }

    #[test]
    fn test_ghost_cleared_on_new_generation() {
        let (mut s, a, b) = two_nodes();
        s.commit(with_arc(&s, a, b));
        s.commit(Graph::new(true));
        assert!(s.ghost().is_none());
    }

    #[test]
    fn test_undo_shows_edited_state_as_ghost() {
        let (mut s, a, b) = two_nodes();
        s.commit(with_arc(&s, a, b));
        let edited = s.ranking().curve(a).unwrap().to_vec();
        s.undo();
        assert_eq!(s.ghost().unwrap().curve(a).unwrap(), &edited[..]);
    }

    #[test]
    fn test_ghost_lookup_removed_node() {
        let (mut s, a, _) = two_nodes();
        let mut g = s.graph().clone();
        g.remove_node(a);
        s.commit(g);
        assert!(s.ranking().curve(a).is_none());
        assert!(s.ghost().unwrap().curve(a).is_some());
    }

    #[test]
    fn test_drag_is_one_step() {
        let (mut s, a, _) = two_nodes();
        s.begin_drag();
        for x in [10.0, 20.0, 30.0] {
            s.drag_to(a, [x, 0.0]);
        }
        s.end_drag();
        assert_eq!(s.graph().node(a).unwrap().pos, [30.0, 0.0]);
        assert!(s.undo());
        assert_eq!(s.graph().node(a).unwrap().pos, [0.0, 0.0]);
    }

    /// Switches a session to traditional normalization.
    fn traditional(s: &mut Session) {
        s.set_options(Options {
            normalization: Normalization::Traditional,
            ..s.options()
        });
    }

    #[test]
    fn test_ghost_survives_switch() {
        let (mut s, a, b) = two_nodes();
        traditional(&mut s);
        let before = s.graph().clone();
        s.commit(with_arc(&s, a, b));
        s.set_centrality(Centrality::Harmonic);
        let expected = Ranking::with_options(&before, Centrality::Harmonic, s.options());
        assert_eq!(
            s.ghost().unwrap().curve(a).unwrap(),
            expected.curve(a).unwrap()
        );
        assert_eq!(s.ranking().centrality, Centrality::Harmonic);
        assert!(!s.can_redo());
    }

    #[test]
    fn test_ghost_after_switch() {
        let (mut s, a, b) = two_nodes();
        s.commit(with_arc(&s, a, b));
        let edited = s.graph().clone();
        // Within a family, the curves of the previous centrality on the
        // current graph
        s.set_centrality(Centrality::Katz);
        let ghost = s.ghost().unwrap();
        assert_eq!(
            (ghost.centrality, ghost.frozen),
            (Centrality::PageRank, None)
        );
        assert_eq!(
            ghost.curves,
            Ranking::new(&edited, Centrality::PageRank).curves
        );
        s.set_centrality(Centrality::PageRank);
        assert_eq!(s.ghost().unwrap().centrality, Centrality::Katz);
        // Across families, its scores at its parameter
        s.set_centrality(Centrality::InDegree);
        let ghost = s.ghost().unwrap();
        assert_eq!(ghost.frozen, Some((Axis::Alpha, 0.85)));
        assert_eq!(
            ghost.curves,
            Ranking::new(&edited, Centrality::PageRank).at(0.85).curves
        );
        // or its curves, if it is geometric
        s.set_centrality(Centrality::Eigenvector);
        let ghost = s.ghost().unwrap();
        assert_eq!(
            (ghost.centrality, ghost.frozen),
            (Centrality::InDegree, None)
        );
        assert_eq!(
            ghost.curves,
            Ranking::new(&edited, Centrality::InDegree).curves
        );
        // The parameter follows the options
        s.set_centrality(Centrality::Katz);
        let k = DEFAULT_ITERATIONS;
        let frozen = Some((Axis::Iteration(k), k as f64));
        assert_eq!(s.ghost().unwrap().frozen, frozen);
        s.set_options(Options {
            iterations: 20,
            ..s.options()
        });
        assert_eq!(s.ghost().unwrap().frozen, Some((Axis::Iteration(20), 20.0)));
        // An edit brings back the graph before the edit
        s.commit(with_arc(&s, b, a));
        let ghost = s.ghost().unwrap();
        assert_eq!(ghost.centrality, Centrality::Katz);
        assert_eq!(ghost.curves, Ranking::new(&edited, Centrality::Katz).curves);
    }

    #[test]
    fn test_ghost_with_traditional_normalization() {
        let (mut s, a, b) = two_nodes();
        s.commit(with_arc(&s, a, b));
        s.set_centrality(Centrality::Katz);
        assert_eq!(s.ghost().unwrap().centrality, Centrality::PageRank);
        // Scores of different centralities are on different scales, so the
        // ghost is always the graph before the edit
        traditional(&mut s);
        assert_eq!(s.ghost().unwrap().centrality, Centrality::Katz);
        for c in [
            Centrality::PageRank,
            Centrality::InDegree,
            Centrality::Harmonic,
        ] {
            s.set_centrality(c);
            assert_eq!(s.ghost().unwrap().centrality, c);
        }
        // Back to ℓ1 normalization, from the next switch on
        s.set_options(Options {
            normalization: Normalization::L1,
            ..s.options()
        });
        assert_eq!(s.ghost().unwrap().centrality, Centrality::Harmonic);
        s.set_centrality(Centrality::Closeness);
        assert_eq!(s.ghost().unwrap().centrality, Centrality::Harmonic);
    }

    #[test]
    fn test_ghost_across_switches_undo_redo() {
        let (mut s, a, b) = two_nodes();
        traditional(&mut s);
        let no_arc = s.graph().clone();
        let arc = with_arc(&s, a, b);
        // The ghost is the current centrality on the other graph
        let check = |s: &Session, other: &Graph| {
            let c = s.centrality();
            assert_eq!(s.ranking().centrality, c);
            let ghost = s.ghost().unwrap();
            assert_eq!(ghost.centrality, c);
            assert_eq!(
                ghost.curve(a).unwrap(),
                Ranking::with_options(other, c, s.options())
                    .curve(a)
                    .unwrap()
            );
        };
        s.commit(arc.clone());
        check(&s, &no_arc);
        s.set_centrality(Centrality::Eigenvector);
        check(&s, &no_arc);
        assert!(s.undo());
        check(&s, &arc);
        s.set_centrality(Centrality::Harmonic);
        check(&s, &arc);
        assert!(s.redo());
        check(&s, &no_arc);
    }

    #[test]
    fn test_ghost_with_deleted_node_survives_switch() {
        let (mut s, a, _) = two_nodes();
        traditional(&mut s);
        let mut g = s.graph().clone();
        g.remove_node(a);
        s.commit(g);
        s.set_centrality(Centrality::Harmonic);
        assert!(s.ranking().curve(a).is_none());
        assert!(s.ghost().unwrap().curve(a).is_some());
    }

    #[test]
    fn test_ghost_after_switch_on_new_graph() {
        // The previous centrality, on no nodes
        let mut s = Session::new(true);
        s.set_centrality(Centrality::Seeley);
        assert!(s.ghost().unwrap().ids.is_empty());
        // No graph before an edit
        traditional(&mut s);
        s.set_centrality(Centrality::Katz);
        assert!(s.ghost().is_none());
    }

    #[test]
    fn test_params_per_centrality() {
        let (mut s, _, _) = two_nodes();
        s.mark_saved("x.json".into());
        s.set_param(0.5);
        assert_eq!(s.alpha(), 0.5);
        s.set_centrality(Centrality::Katz);
        assert_eq!(s.param(), Some(0.85));
        s.step_param(-5);
        assert!((s.katz() - 0.8).abs() < 1e-12);
        s.set_centrality(Centrality::Eigenvector);
        assert_eq!(s.param(), Some(DEFAULT_ITERATIONS as f64));
        s.step_param(-1);
        assert_eq!(s.iteration(), DEFAULT_ITERATIONS - 1);
        s.set_param(1e9);
        assert_eq!(s.iteration(), DEFAULT_ITERATIONS);
        s.set_centrality(Centrality::Closeness);
        assert_eq!(s.param(), None);
        s.step_param(3); // no effect
        s.set_centrality(Centrality::PageRank);
        assert_eq!(s.alpha(), 0.5);
        assert!(!s.is_dirty());
    }

    #[test]
    fn test_scores_follow_param() {
        let (mut s, a, b) = two_nodes();
        s.commit(with_arc(&s, a, b));
        s.set_centrality(Centrality::Eigenvector);
        s.set_param(3.0);
        assert_eq!(s.scores(), &s.ranking().scores(3.0)[..]);
    }

    #[test]
    fn test_save_open_keeps_settings() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("s.json");
        let (mut s, _, _) = two_nodes();
        s.set_centrality(Centrality::Katz);
        s.set_param(0.4);
        s.save_as(&path)?;
        let mut t = Session::open(&path)?;
        assert_eq!(t.settings(), s.settings());
        t.set_centrality(Centrality::Seeley);
        t.step_param(-2);
        t.save_as(&path)?;
        let u = Session::open(&path)?;
        assert_eq!(u.centrality(), Centrality::Seeley);
        assert_eq!(u.iteration(), DEFAULT_ITERATIONS);
        Ok(())
    }

    #[test]
    fn test_scores_at_alpha_one() {
        let (mut s, a, b) = two_nodes();
        s.commit(with_arc(&s, a, b)); // b dangling
        s.set_alpha(1.0);
        assert!(s.scores().iter().all(|x| x.is_finite()));
        assert!((s.scores().iter().sum::<f64>() - 1.0).abs() < 1e-12);
        s.set_alpha(7.0);
        assert_eq!(s.alpha(), 1.0);
    }

    #[test]
    fn test_path_follows_generation() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("example.json");
        let (mut s, _, _) = two_nodes();
        s.save_as(&path)?;
        s.commit(Graph::new(true));
        assert_eq!(s.path(), None);
        assert!(s.save().is_err());
        assert_eq!(crate::files::load(&path)?.0.len(), 2);
        s.undo();
        assert_eq!(s.path(), Some(path.as_path()));
        Ok(())
    }

    #[test]
    fn test_dirty() {
        let (mut s, _, _) = two_nodes();
        assert!(s.is_dirty());
        s.mark_saved("x.json".into());
        assert!(!s.is_dirty());
        s.set_alpha(0.5);
        assert!(!s.is_dirty());
    }

    #[test]
    fn test_options_rerank() {
        let (mut s, a, b) = two_nodes();
        s.commit(with_arc(&s, a, b));
        s.set_centrality(Centrality::Harmonic);
        let traditional = Options {
            normalization: Normalization::Traditional,
            ..s.options()
        };
        s.set_options(traditional);
        // Ranking and ghost are recomputed, with raw scores
        assert_eq!(s.ranking().curve(b).unwrap(), &[1.0, 1.0]);
        assert_eq!(s.ghost().unwrap().curve(b).unwrap(), &[0.0, 0.0]);
        assert_eq!(s.scores(), &[0.0, 1.0]);
        assert_eq!(s.shares(), &[0.0, 1.0]);
        // Options survive a switch of centrality: Katz's index of a → b at
        // β = 0.85, as ρ = 0
        s.set_centrality(Centrality::Katz);
        assert_eq!(s.options(), traditional);
        assert_eq!(s.scores(), &[1.0, 1.85]);
    }

    #[test]
    fn test_iterations_follow_k() {
        let (mut s, _, _) = two_nodes();
        s.set_centrality(Centrality::Seeley);
        let with = |s: &Session, iterations| Options {
            iterations,
            ..s.options()
        };
        // At K, k follows K
        s.set_options(with(&s, 80));
        assert_eq!((s.iteration(), s.ranking().xs.len()), (80, 81));
        // Elsewhere, k stays, unless beyond K
        s.set_param(30.0);
        s.set_options(with(&s, 60));
        assert_eq!(s.iteration(), 30);
        s.set_options(with(&s, 20));
        assert_eq!(s.iteration(), 20);
        s.set_param(1e9);
        assert_eq!(s.iteration(), 20);
        // K is clamped
        s.set_options(with(&s, 0));
        assert_eq!(s.options().iterations, 1);
        s.set_options(with(&s, usize::MAX));
        assert_eq!(s.options().iterations, MAX_ITERATIONS);
    }
}
