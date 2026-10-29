//! The editing session.

use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::files::{self, Settings};
use crate::graph::{Graph, NodeId};
use crate::rank::{Centrality, ITERATIONS, Ranking};

/// An editing session: the current graph, its undo/redo history, its
/// curves for the current centrality, and the curves from before the last
/// edit.
///
/// The session is the single source of truth of the application: every
/// change is a new graph snapshot passed to [`commit`](Self::commit) (or
/// restored by [`undo`](Self::undo)/[`redo`](Self::redo)), and the ranking is
/// recomputed once per change, only if the structure of the graph changed.
///
/// The curves from before the last edit (the *ghost*) follow these rules:
/// when the new graph has a different generation (a new, loaded, or random
/// graph), there is no ghost; when it has the same generation but a different
/// structure, the previous ranking becomes the ghost; otherwise (moves and
/// renames) ranking and ghost are unchanged. The session keeps the graph from
/// before the last structural edit, so that the ghost can be recomputed when
/// the centrality changes.
///
/// Each centrality has its own parameter: the damping factor α for PageRank,
/// the parameter βρ (or β) for Katz's index, and the iteration *k* for
/// Seeley's index and eigenvector centrality. Switching centrality and
/// changing parameters are neither undoable nor unsaved changes.
#[derive(Debug)]
pub struct Session {
    graph: Graph,
    ranking: Ranking,
    ghost: Option<Ranking>,
    /// The graph before the last structural edit, if any; the ghost is its
    /// ranking.
    before: Option<Graph>,
    scores: Vec<f64>,
    centrality: Centrality,
    alpha: f64,
    katz: f64,
    iteration: usize,
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

    /// Creates a session with the given graph, settings, and file path, an
    /// empty history, and no unsaved changes.
    ///
    /// The iteration starts at [`ITERATIONS`].
    pub fn with_graph(graph: Graph, settings: Settings, path: impl Into<Option<PathBuf>>) -> Self {
        let ranking = Ranking::new(&graph, settings.centrality);
        let mut session = Self {
            path_generation: graph.generation(),
            graph,
            ranking,
            ghost: None,
            before: None,
            scores: Vec::new(),
            centrality: settings.centrality,
            alpha: settings.alpha.clamp(0.0, 1.0),
            katz: settings.katz.clamp(0.0, 1.0),
            iteration: ITERATIONS,
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

    /// Returns the settings to be saved with the graph.
    pub fn settings(&self) -> Settings {
        Settings {
            centrality: self.centrality,
            alpha: self.alpha,
            katz: self.katz,
        }
    }

    /// Returns the current centrality.
    pub fn centrality(&self) -> Centrality {
        self.centrality
    }

    /// Switches to another centrality, recomputing ranking, ghost (from the
    /// graph before the last edit), and scores.
    ///
    /// This is neither undoable nor an unsaved change.
    pub fn set_centrality(&mut self, c: Centrality) {
        if c == self.centrality {
            return;
        }
        self.centrality = c;
        self.ranking = Ranking::new(&self.graph, c);
        self.ghost = self.before.as_ref().map(|g| Ranking::new(g, c));
        self.refresh_scores();
    }

    /// Returns the current damping factor.
    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    /// Sets the damping factor, clamped to [0, 1].
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

    /// Returns the current iteration *k* of Seeley's index and eigenvector
    /// centrality.
    pub fn iteration(&self) -> usize {
        self.iteration
    }

    /// Returns the parameter of the current centrality: α for PageRank, βρ
    /// (or β) for Katz's index, *k* for Seeley's index and eigenvector
    /// centrality, and `None` for the others.
    pub fn param(&self) -> Option<f64> {
        match self.centrality {
            Centrality::PageRank => Some(self.alpha),
            Centrality::Katz => Some(self.katz),
            Centrality::Seeley | Centrality::Eigenvector => Some(self.iteration as f64),
            _ => None,
        }
    }

    /// Sets the parameter of the current centrality.
    ///
    /// α and the Katz parameter are clamped to [0, 1]; *k* is rounded and
    /// clamped to 0..=[`ITERATIONS`]. Does nothing for centralities without
    /// parameter. This is neither undoable nor an unsaved change.
    pub fn set_param(&mut self, x: f64) {
        let old = self.param();
        match self.centrality {
            Centrality::PageRank => self.alpha = x.clamp(0.0, 1.0),
            Centrality::Katz => self.katz = x.clamp(0.0, 1.0),
            Centrality::Seeley | Centrality::Eigenvector => {
                self.iteration = x.round().clamp(0.0, ITERATIONS as f64) as usize;
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
                Centrality::PageRank | Centrality::Katz => {
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

    /// Recomputes the scores from the ranking and the current parameter.
    fn refresh_scores(&mut self) {
        self.scores = self.ranking.scores(self.param().unwrap_or(0.0));
    }

    /// Makes a graph current, updating ranking and ghost (see the
    /// documentation of [`Session`]).
    fn install(&mut self, new: Graph) {
        if new.generation() != self.graph.generation() {
            self.ranking = Ranking::new(&new, self.centrality);
            self.ghost = None;
            self.before = None;
        } else if !new.same_structure(&self.graph) {
            let ranking = Ranking::new(&new, self.centrality);
            self.ghost = Some(std::mem::replace(&mut self.ranking, ranking));
            self.before = Some(self.graph.clone());
        }
        self.graph = new;
        self.refresh_scores();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Graph, NodeId};

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

    #[test]
    fn test_ghost_survives_switch() {
        let (mut s, a, b) = two_nodes();
        s.commit(with_arc(&s, a, b));
        s.set_centrality(Centrality::Katz);
        let expected = Ranking::new(
            &{
                let mut g = s.graph().clone();
                g.toggle_arc(a, b);
                g
            },
            Centrality::Katz,
        );
        assert_eq!(
            s.ghost().unwrap().curve(a).unwrap(),
            expected.curve(a).unwrap()
        );
        assert_eq!(s.ranking().centrality, Centrality::Katz);
        assert!(!s.can_redo());
    }

    #[test]
    fn test_ghost_across_switches_undo_redo() {
        let (mut s, a, b) = two_nodes();
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
                Ranking::new(other, c).curve(a).unwrap()
            );
        };
        s.commit(arc.clone());
        check(&s, &no_arc);
        s.set_centrality(Centrality::Katz);
        check(&s, &no_arc);
        assert!(s.undo());
        check(&s, &arc);
        s.set_centrality(Centrality::Eigenvector);
        check(&s, &arc);
        assert!(s.redo());
        check(&s, &no_arc);
    }

    #[test]
    fn test_ghost_with_deleted_node_survives_switch() {
        let (mut s, a, _) = two_nodes();
        let mut g = s.graph().clone();
        g.remove_node(a);
        s.commit(g);
        s.set_centrality(Centrality::Harmonic);
        assert!(s.ranking().curve(a).is_none());
        assert!(s.ghost().unwrap().curve(a).is_some());
    }

    #[test]
    fn test_no_ghost_after_switch_on_new_graph() {
        let mut s = Session::new(true);
        s.set_centrality(Centrality::Seeley);
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
        assert_eq!(s.param(), Some(ITERATIONS as f64));
        s.step_param(-1);
        assert_eq!(s.iteration(), ITERATIONS - 1);
        s.set_param(1e9);
        assert_eq!(s.iteration(), ITERATIONS);
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
        assert_eq!(u.iteration(), ITERATIONS);
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
        let path = dir.path().join("lecture.json");
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
}
