//! The graph document model.
//!
//! A [`Graph`] is what the instructor edits: nodes with stable identifiers,
//! labels, positions, and colors, plus a set of arcs. Node identifiers are
//! never reused within a graph, so colors and before/after curves can be
//! keyed by [`NodeId`] across edits.

use std::collections::{BTreeSet, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{bail, ensure};
use serde::{Deserialize, Serialize};

/// The number of distinct node colors.
pub const PALETTE_SIZE: usize = 12;

/// The source of fresh graph generations.
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(0);

/// A stable node identifier, never reused within a graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodeId(pub u32);

/// A node of a [`Graph`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    /// The stable identifier.
    pub id: NodeId,
    /// The label drawn inside the node.
    pub label: String,
    /// The position, in canvas-local logical points.
    pub pos: [f32; 2],
    /// The palette index, smaller than [`PALETTE_SIZE`].
    pub color: usize,
}

/// A directed or undirected graph, as edited by the instructor.
///
/// Undirected graphs store each edge as a pair of opposite arcs (a self-loop
/// is a single arc), so the PageRank code sees every graph as directed.
///
/// Every graph has a *generation*: graphs built from scratch (new, loaded,
/// or random) get a fresh one, whereas clones keep it. Edits of the same
/// document thus share a generation, which tells wholesale replacements
/// apart from edits.
#[derive(Clone, Debug, PartialEq)]
pub struct Graph {
    directed: bool,
    generation: u64,
    nodes: Vec<Node>,
    arcs: BTreeSet<(NodeId, NodeId)>,
    next_id: u32,
    seed: Option<u64>,
}

/// Returns the `k`-th label in the sequence a, b, …, z, aa, ab, …
/// (bijective base 26).
pub fn label_for(k: usize) -> String {
    let mut k = k + 1;
    let mut label = Vec::new();
    while k > 0 {
        k -= 1;
        label.push(b'a' + (k % 26) as u8);
        k /= 26;
    }
    label.reverse();
    String::from_utf8(label).expect("labels are ASCII")
}

impl Graph {
    /// Creates an empty graph with a fresh generation.
    pub fn new(directed: bool) -> Self {
        Self {
            directed,
            generation: NEXT_GENERATION.fetch_add(1, Ordering::Relaxed),
            nodes: Vec::new(),
            arcs: BTreeSet::new(),
            next_id: 0,
            seed: None,
        }
    }

    /// Creates a graph from its parts, with a fresh generation.
    ///
    /// Fails on duplicate node identifiers, arcs with unknown endpoints, and
    /// colors not smaller than [`PALETTE_SIZE`]. Duplicate arcs are merged
    /// and, for undirected graphs, missing reverse arcs are added.
    pub(crate) fn from_parts(
        directed: bool,
        nodes: Vec<Node>,
        arcs: impl IntoIterator<Item = (NodeId, NodeId)>,
        seed: Option<u64>,
    ) -> anyhow::Result<Self> {
        let mut ids = HashSet::new();
        for node in &nodes {
            ensure!(ids.insert(node.id), "duplicate node id {}", node.id.0);
            ensure!(
                node.color < PALETTE_SIZE,
                "node {} has color {} (must be smaller than {PALETTE_SIZE})",
                node.id.0,
                node.color
            );
        }
        let next_id = nodes.iter().map(|n| n.id.0 + 1).max().unwrap_or(0);
        let mut graph = Self {
            next_id,
            nodes,
            seed,
            ..Self::new(directed)
        };
        for (from, to) in arcs {
            if !ids.contains(&from) || !ids.contains(&to) {
                bail!("arc ({}, {}) has an unknown endpoint", from.0, to.0);
            }
            graph.add_arc(from, to);
        }
        Ok(graph)
    }

    /// Returns whether the graph is directed.
    pub fn is_directed(&self) -> bool {
        self.directed
    }

    /// Returns the generation of this graph.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns the seed that generated this graph, if it is random.
    pub fn seed(&self) -> Option<u64> {
        self.seed
    }

    /// Sets the seed that generated this graph.
    pub fn set_seed(&mut self, seed: impl Into<Option<u64>>) {
        self.seed = seed.into();
    }

    /// Returns the number of nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns whether the graph has no nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Returns the nodes, in index order.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// Returns the node with the given identifier.
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Returns the index of the node with the given identifier.
    pub fn index_of(&self, id: NodeId) -> Option<usize> {
        self.nodes.iter().position(|n| n.id == id)
    }

    /// Adds a node at the given position and returns its identifier.
    ///
    /// The label is the first label of [`label_for`] not used by any node,
    /// and the color is the least used palette index (the lowest one in case
    /// of ties).
    pub fn add_node(&mut self, pos: [f32; 2]) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id += 1;
        let used: HashSet<&str> = self.nodes.iter().map(|n| n.label.as_str()).collect();
        let label = (0..)
            .map(label_for)
            .find(|l| !used.contains(l.as_str()))
            .expect("there are infinitely many labels");
        let mut counts = [0usize; PALETTE_SIZE];
        for node in &self.nodes {
            counts[node.color] += 1;
        }
        let color = (0..PALETTE_SIZE)
            .min_by_key(|&c| counts[c])
            .expect("the palette is not empty");
        self.nodes.push(Node {
            id,
            label,
            pos,
            color,
        });
        id
    }

    /// Removes a node and its incident arcs; returns whether it existed.
    pub fn remove_node(&mut self, id: NodeId) -> bool {
        let Some(index) = self.index_of(id) else {
            return false;
        };
        self.nodes.remove(index);
        self.arcs.retain(|&(from, to)| from != id && to != id);
        true
    }

    /// Sets the label of a node; returns whether the node exists.
    pub fn set_label(&mut self, id: NodeId, label: impl AsRef<str>) -> bool {
        match self.nodes.iter_mut().find(|n| n.id == id) {
            Some(node) => {
                node.label = label.as_ref().to_owned();
                true
            }
            None => false,
        }
    }

    /// Sets the position of a node; returns whether the node exists.
    pub fn set_pos(&mut self, id: NodeId, pos: [f32; 2]) -> bool {
        match self.nodes.iter_mut().find(|n| n.id == id) {
            Some(node) => {
                node.pos = pos;
                true
            }
            None => false,
        }
    }

    /// Sets the positions of all nodes, in index order.
    pub fn set_positions(&mut self, pos: &[[f32; 2]]) {
        assert_eq!(pos.len(), self.nodes.len());
        for (node, &p) in self.nodes.iter_mut().zip(pos) {
            node.pos = p;
        }
    }

    /// Returns whether the arc `from` → `to` exists.
    pub fn has_arc(&self, from: NodeId, to: NodeId) -> bool {
        self.arcs.contains(&(from, to))
    }

    /// Adds the arc `from` → `to` (and its reverse, if the graph is
    /// undirected); returns false if the arc exists or an endpoint is
    /// unknown.
    pub fn add_arc(&mut self, from: NodeId, to: NodeId) -> bool {
        if self.has_arc(from, to) || self.index_of(from).is_none() || self.index_of(to).is_none() {
            return false;
        }
        self.arcs.insert((from, to));
        if !self.directed {
            self.arcs.insert((to, from));
        }
        true
    }

    /// Removes the arc `from` → `to` (and its reverse, if the graph is
    /// undirected); returns whether the arc existed.
    pub fn remove_arc(&mut self, from: NodeId, to: NodeId) -> bool {
        let removed = self.arcs.remove(&(from, to));
        if removed && !self.directed {
            self.arcs.remove(&(to, from));
        }
        removed
    }

    /// Adds the arc `from` → `to` if missing, removes it otherwise; returns
    /// whether the arc exists after the call.
    pub fn toggle_arc(&mut self, from: NodeId, to: NodeId) -> bool {
        if self.remove_arc(from, to) {
            false
        } else {
            self.add_arc(from, to)
        }
    }

    /// Returns the arcs, in increasing order.
    pub fn arcs(&self) -> impl Iterator<Item = (NodeId, NodeId)> + '_ {
        self.arcs.iter().copied()
    }

    /// Returns the successor lists by node index, each sorted.
    pub fn successors(&self) -> Vec<Vec<usize>> {
        let mut succ = vec![Vec::new(); self.nodes.len()];
        for &(from, to) in &self.arcs {
            let (Some(f), Some(t)) = (self.index_of(from), self.index_of(to)) else {
                unreachable!("arcs have known endpoints");
            };
            succ[f].push(t);
        }
        for list in &mut succ {
            list.sort_unstable();
        }
        succ
    }

    /// Returns whether the two graphs have the same directedness, the same
    /// node identifiers, and the same arcs (labels, positions, colors, seeds,
    /// and generations are ignored).
    pub fn same_structure(&self, other: &Graph) -> bool {
        let ids = |g: &Graph| g.nodes.iter().map(|n| n.id).collect::<BTreeSet<_>>();
        self.directed == other.directed && self.arcs == other.arcs && ids(self) == ids(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_label_for() {
        assert_eq!(label_for(0), "a");
        assert_eq!(label_for(25), "z");
        assert_eq!(label_for(26), "aa");
        assert_eq!(label_for(27), "ab");
        assert_eq!(label_for(52), "ba");
        assert_eq!(label_for(701), "zz");
        assert_eq!(label_for(702), "aaa");
    }

    #[test]
    fn test_add_node_labels_and_colors() {
        let mut g = Graph::new(true);
        let a = g.add_node([0.0, 0.0]);
        let b = g.add_node([1.0, 0.0]);
        let _c = g.add_node([2.0, 0.0]);
        assert!(g.remove_node(b));
        let d = g.add_node([3.0, 0.0]);
        let labels: Vec<_> = g.nodes().iter().map(|n| n.label.as_str()).collect();
        assert_eq!(labels, ["a", "c", "b"]);
        assert_eq!(g.node(d).unwrap().color, 1);
        assert_eq!(g.node(a).unwrap().color, 0);
        assert_eq!(d, NodeId(3)); // ids are never reused
    }

    #[test]
    fn test_colors_wrap() {
        let mut g = Graph::new(true);
        let ids: Vec<_> = (0..=PALETTE_SIZE)
            .map(|i| g.add_node([i as f32, 0.0]))
            .collect();
        assert_eq!(g.node(ids[PALETTE_SIZE]).unwrap().color, 0);
    }

    #[test]
    fn test_toggle_arc_directed() {
        let mut g = Graph::new(true);
        let (a, b) = (g.add_node([0.0, 0.0]), g.add_node([1.0, 0.0]));
        assert!(g.toggle_arc(a, b));
        assert!(g.has_arc(a, b));
        assert!(!g.has_arc(b, a));
        assert!(!g.toggle_arc(a, b));
        assert!(!g.has_arc(a, b));
    }

    #[test]
    fn test_toggle_arc_undirected() {
        let mut g = Graph::new(false);
        let (a, b) = (g.add_node([0.0, 0.0]), g.add_node([1.0, 0.0]));
        assert!(g.toggle_arc(a, b));
        assert!(g.has_arc(a, b) && g.has_arc(b, a));
        assert!(!g.toggle_arc(b, a));
        assert_eq!(g.arcs().count(), 0);
        assert!(g.toggle_arc(a, a));
        assert_eq!(g.arcs().count(), 1);
    }

    #[test]
    fn test_remove_node_removes_arcs() {
        let mut g = Graph::new(true);
        let (a, b, c) = (
            g.add_node([0.0; 2]),
            g.add_node([1.0, 0.0]),
            g.add_node([2.0, 0.0]),
        );
        g.add_arc(a, b);
        g.add_arc(b, c);
        g.add_arc(c, a);
        g.remove_node(b);
        assert_eq!(g.arcs().collect::<Vec<_>>(), [(c, a)]);
        assert!(!g.add_arc(a, b));
    }

    #[test]
    fn test_successors() {
        let mut g = Graph::new(true);
        let ids: Vec<_> = (0..3).map(|i| g.add_node([i as f32, 0.0])).collect();
        g.add_arc(ids[0], ids[2]);
        g.add_arc(ids[0], ids[1]);
        g.add_arc(ids[2], ids[0]);
        assert_eq!(g.successors(), vec![vec![1, 2], vec![], vec![0]]);
    }

    #[test]
    fn test_same_structure() {
        let mut g = Graph::new(true);
        let (a, b) = (g.add_node([0.0; 2]), g.add_node([1.0, 0.0]));
        let mut h = g.clone();
        h.set_pos(a, [5.0, 5.0]);
        h.set_label(b, "x");
        assert!(g.same_structure(&h));
        h.add_arc(a, b);
        assert!(!g.same_structure(&h));
        let mut k = g.clone();
        k.add_node([2.0, 0.0]);
        assert!(!g.same_structure(&k));
    }

    #[test]
    fn test_generation() {
        let g = Graph::new(true);
        assert_ne!(Graph::new(true).generation(), g.generation());
        assert_eq!(g.clone().generation(), g.generation());
    }

    #[test]
    fn test_from_parts_validation() {
        let node = |id, color| Node {
            id: NodeId(id),
            label: "n".into(),
            pos: [0.0; 2],
            color,
        };
        assert!(Graph::from_parts(true, vec![node(0, 0), node(0, 1)], [], None).is_err());
        assert!(Graph::from_parts(true, vec![node(0, 0)], [(NodeId(0), NodeId(7))], None).is_err());
        assert!(Graph::from_parts(true, vec![node(0, PALETTE_SIZE)], [], None).is_err());
        let mut g = Graph::from_parts(
            false,
            vec![node(0, 0), node(5, 1)],
            [(NodeId(0), NodeId(5))],
            None,
        )
        .unwrap();
        assert!(g.has_arc(NodeId(5), NodeId(0)));
        assert_eq!(g.add_node([0.0; 2]), NodeId(6));
    }
}
