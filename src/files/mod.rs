//! Saving and loading graphs as JSON.
//!
//! The format (version 1) is
//!
//! ```json
//! {
//!   "version": 1,
//!   "directed": true,
//!   "alpha": 0.85,
//!   "centrality": "pagerank",
//!   "katz": 0.85,
//!   "seed": null,
//!   "nodes": [{ "id": 0, "label": "a", "pos": [100.0, 200.0], "color": 0 }],
//!   "arcs": [[0, 0]]
//! }
//! ```
//!
//! The fields `centrality` and `katz` are optional: files without them load
//! with PageRank and the default Katz parameter.

use std::path::Path;

use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};

use crate::graph::{Graph, Node, NodeId};
use crate::rank::Centrality;

/// The current version of the file format.
const VERSION: u32 = 1;

/// The default Katz parameter βρ.
const DEFAULT_KATZ: f64 = 0.85;

/// Returns the default Katz parameter, for serde.
fn default_katz() -> f64 {
    DEFAULT_KATZ
}

/// The settings saved with a graph: the current centrality and the values
/// of the damping factor α and of the Katz parameter βρ (or β).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// The current centrality.
    pub centrality: Centrality,
    /// The damping factor α, in [0, 1].
    pub alpha: f64,
    /// The Katz parameter, in [0, 1].
    pub katz: f64,
}

impl Default for Settings {
    /// PageRank, with α = 0.85 and βρ = 0.85.
    fn default() -> Self {
        Self {
            centrality: Centrality::PageRank,
            alpha: 0.85,
            katz: DEFAULT_KATZ,
        }
    }
}

/// The serialized form of a graph.
#[derive(Serialize, Deserialize)]
struct GraphFile {
    version: u32,
    directed: bool,
    alpha: f64,
    #[serde(default)]
    centrality: Centrality,
    #[serde(default = "default_katz")]
    katz: f64,
    seed: Option<u64>,
    nodes: Vec<Node>,
    arcs: Vec<(NodeId, NodeId)>,
}

/// Serializes a graph and its settings as pretty-printed JSON.
pub fn to_json(graph: &Graph, settings: &Settings) -> String {
    let file = GraphFile {
        version: VERSION,
        directed: graph.is_directed(),
        alpha: settings.alpha,
        centrality: settings.centrality,
        katz: settings.katz,
        seed: graph.seed(),
        nodes: graph.nodes().to_vec(),
        arcs: graph.arcs().collect(),
    };
    serde_json::to_string_pretty(&file).expect("graphs are serializable")
}

/// Deserializes a graph and its settings (α and the Katz parameter are
/// clamped to [0, 1]).
///
/// The graph gets a fresh generation. Fails on malformed JSON, unknown
/// versions, and inconsistent graphs (see [`Graph`]).
pub fn from_json(json: impl AsRef<str>) -> anyhow::Result<(Graph, Settings)> {
    let file: GraphFile = serde_json::from_str(json.as_ref()).context("malformed graph file")?;
    ensure!(
        file.version == VERSION,
        "unsupported graph file version {} (expected {VERSION})",
        file.version
    );
    let graph = Graph::from_parts(file.directed, file.nodes, file.arcs, file.seed)?;
    let settings = Settings {
        centrality: file.centrality,
        alpha: file.alpha.clamp(0.0, 1.0),
        katz: file.katz.clamp(0.0, 1.0),
    };
    Ok((graph, settings))
}

/// Saves a graph and its settings to a file.
pub fn save(graph: &Graph, settings: &Settings, path: impl AsRef<Path>) -> anyhow::Result<()> {
    let path = path.as_ref();
    std::fs::write(path, to_json(graph, settings))
        .with_context(|| format!("cannot write {}", path.display()))
}

/// Loads a graph and its settings from a file.
pub fn load(path: impl AsRef<Path>) -> anyhow::Result<(Graph, Settings)> {
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
        let settings = Settings {
            alpha: 0.7,
            ..Settings::default()
        };
        let (h, t) = from_json(to_json(&g, &settings))?;
        assert_eq!(t, settings);
        assert!(h.same_structure(&g));
        assert_eq!(h.nodes(), g.nodes());
        assert_eq!(h.seed(), Some(9));
        assert!(!h.is_directed());
        assert_ne!(h.generation(), g.generation());
        Ok(())
    }

    #[test]
    fn test_roundtrip_settings() -> anyhow::Result<()> {
        let s = Settings {
            centrality: Centrality::Katz,
            alpha: 0.7,
            katz: 0.3,
        };
        let (_, t) = from_json(to_json(&Graph::new(true), &s))?;
        assert_eq!(t, s);
        Ok(())
    }

    #[test]
    fn test_load_0_1_0_file() -> anyhow::Result<()> {
        let (_, s) = from_json(
            r#"{"version":1,"directed":true,"alpha":0.6,"seed":null,"nodes":[],"arcs":[]}"#,
        )?;
        assert_eq!(
            s,
            Settings {
                centrality: Centrality::PageRank,
                alpha: 0.6,
                katz: 0.85
            }
        );
        Ok(())
    }

    #[test]
    fn test_load_clamps_settings() -> anyhow::Result<()> {
        let (_, s) = from_json(
            r#"{"version":1,"directed":true,"alpha":7,"katz":-2,"seed":null,"nodes":[],"arcs":[]}"#,
        )?;
        assert_eq!((s.alpha, s.katz), (1.0, 0.0));
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
