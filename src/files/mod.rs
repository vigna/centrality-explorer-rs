//! Saving and loading graphs as JSON.
//!
//! The format (version 1) is
//!
//! ```json
//! {
//!   "version": 1,
//!   "directed": true,
//!   "alpha": 0.85,
//!   "seed": null,
//!   "nodes": [{ "id": 0, "label": "a", "pos": [100.0, 200.0], "color": 0 }],
//!   "arcs": [[0, 0]]
//! }
//! ```

use std::path::Path;

use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};

use crate::graph::{Graph, Node, NodeId};

/// The current version of the file format.
const VERSION: u32 = 1;

/// The serialized form of a graph.
#[derive(Serialize, Deserialize)]
struct GraphFile {
    version: u32,
    directed: bool,
    alpha: f64,
    seed: Option<u64>,
    nodes: Vec<Node>,
    arcs: Vec<(NodeId, NodeId)>,
}

/// Serializes a graph and a damping factor as pretty-printed JSON.
pub fn to_json(graph: &Graph, alpha: f64) -> String {
    let file = GraphFile {
        version: VERSION,
        directed: graph.is_directed(),
        alpha,
        seed: graph.seed(),
        nodes: graph.nodes().to_vec(),
        arcs: graph.arcs().collect(),
    };
    serde_json::to_string_pretty(&file).expect("graphs are serializable")
}

/// Deserializes a graph and a damping factor (clamped to [0, 1]).
///
/// The graph gets a fresh generation. Fails on malformed JSON, unknown
/// versions, and inconsistent graphs (see [`Graph`]).
pub fn from_json(json: impl AsRef<str>) -> anyhow::Result<(Graph, f64)> {
    let file: GraphFile = serde_json::from_str(json.as_ref()).context("malformed graph file")?;
    ensure!(
        file.version == VERSION,
        "unsupported graph file version {} (expected {VERSION})",
        file.version
    );
    let graph = Graph::from_parts(file.directed, file.nodes, file.arcs, file.seed)?;
    Ok((graph, file.alpha.clamp(0.0, 1.0)))
}

/// Saves a graph and a damping factor to a file.
pub fn save(graph: &Graph, alpha: f64, path: impl AsRef<Path>) -> anyhow::Result<()> {
    let path = path.as_ref();
    std::fs::write(path, to_json(graph, alpha))
        .with_context(|| format!("cannot write {}", path.display()))
}

/// Loads a graph and a damping factor from a file.
pub fn load(path: impl AsRef<Path>) -> anyhow::Result<(Graph, f64)> {
    let path = path.as_ref();
    let json =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    from_json(json).with_context(|| format!("cannot load {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;

    #[test]
    fn test_roundtrip() -> anyhow::Result<()> {
        let mut g = Graph::new(false);
        let (a, b) = (g.add_node([1.0, 2.0]), g.add_node([3.0, 4.0]));
        g.toggle_arc(a, b);
        g.set_label(a, "home");
        g.set_seed(Some(9));
        let (h, alpha) = from_json(to_json(&g, 0.7))?;
        assert_eq!(alpha, 0.7);
        assert!(h.same_structure(&g));
        assert_eq!(h.nodes(), g.nodes());
        assert_eq!(h.seed(), Some(9));
        assert!(!h.is_directed());
        assert_ne!(h.generation(), g.generation());
        Ok(())
    }

    #[test]
    fn test_load_rejects_bad_version() {
        assert!(
            from_json(
                r#"{"version":2,"directed":true,"alpha":0.85,"seed":null,"nodes":[],"arcs":[]}"#
            )
            .is_err()
        );
    }

    #[test]
    fn test_load_rejects_unknown_arc_id() {
        assert!(
            from_json(
                r#"{"version":1,"directed":true,"alpha":0.85,"seed":null,
                "nodes":[{"id":0,"label":"a","pos":[0,0],"color":0}],"arcs":[[0,3]]}"#
            )
            .is_err()
        );
    }

    #[test]
    fn test_load_rejects_duplicate_node_id() {
        assert!(
            from_json(
                r#"{"version":1,"directed":true,"alpha":0.85,"seed":null,
                "nodes":[{"id":0,"label":"a","pos":[0,0],"color":0},
                         {"id":0,"label":"b","pos":[0,0],"color":1}],"arcs":[]}"#
            )
            .is_err()
        );
    }

    #[test]
    fn test_load_rejects_bad_positions() {
        for pos in ["[1e39,0]", "[0,-1e39]", "[1e9,0]"] {
            let json = format!(
                r#"{{"version":1,"directed":true,"alpha":0.85,"seed":null,
                "nodes":[{{"id":0,"label":"a","pos":{pos},"color":0}}],"arcs":[]}}"#
            );
            assert!(from_json(json).is_err(), "{pos}");
        }
    }

    #[test]
    fn test_load_rejects_malformed_json() {
        assert!(from_json("{ not json").is_err());
        assert!(from_json(r#"{"version":1}"#).is_err());
    }

    #[test]
    fn test_session_save_open() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("lecture.json");
        let mut s = Session::new(true);
        let mut g = s.graph().clone();
        g.add_node([5.0, 5.0]);
        s.commit(g);
        s.save_as(&path)?;
        assert!(!s.is_dirty());
        let t = Session::open(&path)?;
        assert_eq!(t.graph().len(), 1);
        assert_eq!(t.path(), Some(path.as_path()));
        assert!(!t.can_undo());
        Ok(())
    }
}
