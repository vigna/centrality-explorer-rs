//! Editing gestures.
//!
//! An [`Editor`] turns clicks and key presses on the canvas into modified
//! copies of a [`Graph`], to be committed to the session. Hit testing and
//! drawing are left to the user interface.

use crate::graph::{Graph, NodeId};

/// The gesture in progress.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Gesture {
    /// Nothing is in progress.
    #[default]
    Idle,
    /// A source node was clicked: the next click on a node toggles an edge.
    PendingEdge {
        /// The source node.
        from: NodeId,
    },
    /// A node is being renamed.
    Renaming {
        /// The node being renamed.
        node: NodeId,
        /// The new label typed so far.
        text: String,
    },
}

/// What a click hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    /// A node.
    Node(NodeId),
    /// An arc, from the first node to the second.
    Arc(NodeId, NodeId),
    /// The empty canvas.
    Empty,
}

/// A key relevant to editing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EditKey {
    /// A typed character.
    Char(char),
    /// The backspace key.
    Backspace,
    /// The delete key.
    Delete,
    /// The enter key.
    Enter,
    /// The escape key.
    Escape,
}

/// The editing state machine.
///
/// Clicking a node and then another node toggles the edge between them;
/// clicking an edge removes it; typing over a node renames it; Backspace or
/// Delete over a node deletes it; Escape cancels whatever is in progress.
#[derive(Debug, Default)]
pub struct Editor {
    gesture: Gesture,
}

impl Editor {
    /// Returns the gesture in progress.
    pub fn gesture(&self) -> &Gesture {
        &self.gesture
    }

    /// Handles a primary click; returns the modified graph, if the click
    /// changed it.
    pub fn click(&mut self, graph: &Graph, target: Target) -> Option<Graph> {
        match std::mem::take(&mut self.gesture) {
            Gesture::Renaming { node, text } => rename(graph, node, &text),
            Gesture::PendingEdge { from } => match target {
                Target::Node(to) if to != from => {
                    let mut graph = graph.clone();
                    graph.toggle_arc(from, to);
                    Some(graph)
                }
                _ => None,
            },
            Gesture::Idle => match target {
                Target::Node(node) => {
                    self.gesture = Gesture::PendingEdge { from: node };
                    None
                }
                Target::Arc(from, to) => Some(delete_arc(graph, from, to)),
                Target::Empty => None,
            },
        }
    }

    /// Handles a double click on the empty canvas by adding a node at the
    /// given position.
    pub fn double_click_empty(&mut self, graph: &Graph, pos: [f32; 2]) -> Graph {
        self.gesture = Gesture::Idle;
        let mut graph = graph.clone();
        graph.add_node(pos);
        graph
    }

    /// Handles a key press, given the node under the pointer; returns the
    /// modified graph, if the key changed it.
    pub fn key(&mut self, graph: &Graph, hovered: Option<NodeId>, key: EditKey) -> Option<Graph> {
        if let Gesture::Renaming { node, text } = &mut self.gesture {
            match key {
                EditKey::Char(c) => text.push(c),
                EditKey::Backspace => {
                    text.pop();
                }
                EditKey::Enter => {
                    let (node, text) = (*node, std::mem::take(text));
                    self.gesture = Gesture::Idle;
                    return rename(graph, node, &text);
                }
                EditKey::Escape => self.gesture = Gesture::Idle,
                EditKey::Delete => {}
            }
            return None;
        }
        match (key, hovered) {
            (EditKey::Escape, _) => {
                self.gesture = Gesture::Idle;
                None
            }
            (EditKey::Char(c), Some(node)) if c.is_alphanumeric() => {
                self.gesture = Gesture::Renaming {
                    node,
                    text: c.to_string(),
                };
                None
            }
            (EditKey::Backspace | EditKey::Delete, Some(node)) => {
                self.gesture = Gesture::Idle;
                Some(delete_node(graph, node))
            }
            _ => None,
        }
    }

    /// Starts renaming a node, starting from its current label.
    pub fn start_rename(&mut self, graph: &Graph, node: NodeId) {
        if let Some(n) = graph.node(node) {
            self.gesture = Gesture::Renaming {
                node,
                text: n.label.clone(),
            };
        }
    }

    /// Cancels the gesture in progress.
    pub fn cancel(&mut self) {
        self.gesture = Gesture::Idle;
    }

    /// Cancels the gesture in progress if it refers to a node that is not in
    /// the given graph (e.g., after an undo).
    pub fn prune(&mut self, graph: &Graph) {
        let node = match &self.gesture {
            Gesture::Idle => return,
            Gesture::PendingEdge { from } => *from,
            Gesture::Renaming { node, .. } => *node,
        };
        if graph.node(node).is_none() {
            self.gesture = Gesture::Idle;
        }
    }
}

/// Returns a copy of the graph with the label of a node replaced, or `None`
/// if the label is empty or the node does not exist.
fn rename(graph: &Graph, node: NodeId, text: &str) -> Option<Graph> {
    if text.is_empty() {
        return None;
    }
    let mut graph = graph.clone();
    graph.set_label(node, text).then_some(graph)
}

/// Returns a copy of the graph with the self-loop of a node toggled.
pub fn toggle_self_loop(graph: &Graph, node: NodeId) -> Graph {
    let mut graph = graph.clone();
    graph.toggle_arc(node, node);
    graph
}

/// Returns a copy of the graph without a node.
pub fn delete_node(graph: &Graph, node: NodeId) -> Graph {
    let mut graph = graph.clone();
    graph.remove_node(node);
    graph
}

/// Returns a copy of the graph without an arc (and its reverse, if the
/// graph is undirected).
pub fn delete_arc(graph: &Graph, from: NodeId, to: NodeId) -> Graph {
    let mut graph = graph.clone();
    graph.remove_arc(from, to);
    graph
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (Graph, NodeId, NodeId) {
        let mut g = Graph::new(true);
        let (a, b) = (g.add_node([0.0; 2]), g.add_node([100.0, 0.0]));
        (g, a, b)
    }

    #[test]
    fn test_click_click_toggles() {
        let (g, a, b) = setup();
        let mut e = Editor::default();
        assert_eq!(e.click(&g, Target::Node(a)), None);
        assert_eq!(e.gesture(), &Gesture::PendingEdge { from: a });
        let g = e.click(&g, Target::Node(b)).unwrap();
        assert!(g.has_arc(a, b));
        e.click(&g, Target::Node(a));
        let g = e.click(&g, Target::Node(b)).unwrap();
        assert!(!g.has_arc(a, b));
        assert_eq!(e.gesture(), &Gesture::Idle);
    }

    #[test]
    fn test_click_same_node_cancels() {
        let (g, a, _) = setup();
        let mut e = Editor::default();
        e.click(&g, Target::Node(a));
        assert_eq!(e.click(&g, Target::Node(a)), None);
        assert_eq!(e.gesture(), &Gesture::Idle);
    }

    #[test]
    fn test_click_arc_removes() {
        let (mut g, a, b) = setup();
        g.add_arc(a, b);
        let mut e = Editor::default();
        assert!(!e.click(&g, Target::Arc(a, b)).unwrap().has_arc(a, b));
    }

    #[test]
    fn test_escape_and_empty_cancel() {
        let (g, a, _) = setup();
        let mut e = Editor::default();
        e.click(&g, Target::Node(a));
        e.click(&g, Target::Empty);
        assert_eq!(e.gesture(), &Gesture::Idle);
        e.click(&g, Target::Node(a));
        assert_eq!(e.key(&g, None, EditKey::Escape), None);
        assert_eq!(e.gesture(), &Gesture::Idle);
    }

    #[test]
    fn test_rename() {
        let (g, a, _) = setup();
        let mut e = Editor::default();
        assert_eq!(e.key(&g, Some(a), EditKey::Char('h')), None);
        e.key(&g, Some(a), EditKey::Char('x'));
        e.key(&g, Some(a), EditKey::Backspace);
        e.key(&g, Some(a), EditKey::Char('i'));
        let g = e.key(&g, Some(a), EditKey::Enter).unwrap();
        assert_eq!(g.node(a).unwrap().label, "hi");
    }

    #[test]
    fn test_rename_escape_keeps_label() {
        let (g, a, _) = setup();
        let mut e = Editor::default();
        e.key(&g, Some(a), EditKey::Char('z'));
        assert_eq!(e.key(&g, Some(a), EditKey::Escape), None);
        assert_eq!(g.node(a).unwrap().label, "a");
    }

    #[test]
    fn test_delete_hovered() {
        let (g, a, _) = setup();
        let mut e = Editor::default();
        let g = e.key(&g, Some(a), EditKey::Backspace).unwrap();
        assert!(g.node(a).is_none());
        assert_eq!(e.key(&g, None, EditKey::Delete), None);
    }

    #[test]
    fn test_double_click_adds_node() {
        let (g, _, _) = setup();
        let g = Editor::default().double_click_empty(&g, [42.0, 7.0]);
        assert_eq!(g.len(), 3);
        assert_eq!(g.nodes()[2].pos, [42.0, 7.0]);
    }

    #[test]
    fn test_prune_drops_stale_ids() {
        let (g, a, _) = setup();
        let mut e = Editor::default();
        e.click(&g, Target::Node(a));
        let mut h = g.clone();
        h.remove_node(a);
        e.prune(&h);
        assert_eq!(e.gesture(), &Gesture::Idle);
    }

    #[test]
    fn test_toggle_self_loop() {
        let (g, a, _) = setup();
        let g = toggle_self_loop(&g, a);
        assert!(g.has_arc(a, a));
        assert!(!toggle_self_loop(&g, a).has_arc(a, a));
    }
}
