//! The editing session.

use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::files;
use crate::graph::{Graph, NodeId};
use crate::rank::{Centrality, Ranking};

/// An editing session: the current graph, its undo/redo history, its
/// PageRank curves, and the curves from before the last edit.
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
/// renames) ranking and ghost are unchanged.
#[derive(Debug)]
pub struct Session {
    graph: Graph,
    ranking: Ranking,
    ghost: Option<Ranking>,
    scores: Vec<f64>,
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
    /// The default damping factor.
    pub const DEFAULT_ALPHA: f64 = 0.85;

    /// Creates a session with an empty graph.
    pub fn new(directed: bool) -> Self {
        Self::with_graph(Graph::new(directed), Self::DEFAULT_ALPHA, None)
    }

    /// Creates a session with the given graph, damping factor, and file
    /// path, an empty history, and no unsaved changes.
    pub fn with_graph(graph: Graph, alpha: f64, path: impl Into<Option<PathBuf>>) -> Self {
        let ranking = Ranking::new(&graph, Centrality::PageRank);
        let alpha = alpha.clamp(0.0, 1.0);
        let scores = ranking.scores(alpha);
        Self {
            path_generation: graph.generation(),
            graph,
            ranking,
            ghost: None,
            scores,
            alpha,
            undo: Vec::new(),
            redo: Vec::new(),
            drag_origin: None,
            path: path.into(),
            dirty: false,
        }
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

    /// Returns the exact PageRank at the current damping factor, aligned with
    /// the identifiers of [`ranking`](Self::ranking).
    pub fn scores(&self) -> &[f64] {
        &self.scores
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
            self.scores = self.ranking.scores(alpha);
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
        let (graph, alpha) = files::load(path)?;
        Ok(Self::with_graph(graph, alpha, path.to_path_buf()))
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
        files::save(&self.graph, self.alpha, path)?;
        self.mark_saved(path.to_path_buf());
        Ok(())
    }

    /// Associates the session with a file and marks it as saved.
    pub fn mark_saved(&mut self, path: PathBuf) {
        self.path = Some(path);
        self.path_generation = self.graph.generation();
        self.dirty = false;
    }

    /// Makes a graph current, updating ranking and ghost (see the
    /// documentation of [`Session`]).
    fn install(&mut self, new: Graph) {
        if new.generation() != self.graph.generation() {
            self.ranking = Ranking::new(&new, Centrality::PageRank);
            self.ghost = None;
            self.scores = self.ranking.scores(self.alpha);
        } else if !new.same_structure(&self.graph) {
            let ranking = Ranking::new(&new, Centrality::PageRank);
            self.ghost = Some(std::mem::replace(&mut self.ranking, ranking));
            self.scores = self.ranking.scores(self.alpha);
        }
        self.graph = new;
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
