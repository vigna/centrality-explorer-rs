//! Example graphs built into the application, for teaching without files
//! (the web build cannot open files). The graphs are in `data/examples`,
//! converted by `tools/convert_examples.py`.

use crate::files;
use crate::graph::Graph;

/// A built-in example graph.
pub struct Example {
    /// The name shown in the Examples menu.
    pub name: &'static str,
    /// The graph, in the format of [`files`].
    json: &'static str,
}

impl Example {
    /// Returns the example's graph. Nodes without a layout of their own all
    /// sit at the origin; lay the graph out before showing it.
    pub fn graph(&self) -> Graph {
        files::from_json(self.json)
            .expect("the built-in examples are valid")
            .0
    }
}

/// The built-in examples, in menu order. The first one is shown when the
/// web build starts.
pub const EXAMPLES: &[Example] = &[
    Example {
        name: "Fauci Emails (32, directed)",
        json: include_str!("../../data/examples/fauci.json"),
    },
    Example {
        name: "Karate Club (34)",
        json: include_str!("../../data/examples/karate.json"),
    },
    Example {
        name: "Dolphins (62)",
        json: include_str!("../../data/examples/dolphins.json"),
    },
    Example {
        name: "Political Books (105)",
        json: include_str!("../../data/examples/polbooks.json"),
    },
    Example {
        name: "College Football (115)",
        json: include_str!("../../data/examples/football.json"),
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_example_loads_with_its_size() {
        let sizes: Vec<(usize, usize, bool)> = EXAMPLES
            .iter()
            .map(|e| {
                let g = e.graph();
                (g.len(), g.arcs().count(), g.is_directed())
            })
            .collect();
        // Undirected graphs store both directions of each edge.
        assert_eq!(
            sizes,
            [
                (32, 63, true),
                (34, 156, false),
                (62, 318, false),
                (105, 882, false),
                (115, 1226, false)
            ]
        );
    }
}
