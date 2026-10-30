//! Example graphs built into the application, for teaching without files
//! (the web build cannot open files). The graphs are in `data/examples`,
//! converted by `tools/convert_examples.py`.

use crate::files;
use crate::graph::Graph;

/// A built-in example graph.
pub struct Example {
    /// The name shown in the Examples menu.
    pub name: &'static str,
    /// Whether the graph is directed (the menu groups the examples by it).
    pub directed: bool,
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

/// Declares a built-in example from its file in `data/examples`.
macro_rules! example {
    ($name:expr, $directed:expr, $file:literal) => {
        Example {
            name: $name,
            directed: $directed,
            json: include_str!(concat!("../../data/examples/", $file, ".json")),
        }
    };
}

/// The built-in examples, in menu order, directed ones first. The first one
/// is shown when the web build starts.
pub const EXAMPLES: &[Example] = &[
    example!("Fauci Emails (32)", true, "fauci"),
    example!("Gleich 2006 Example (11)", true, "gleich2006"),
    example!("Magnetic Powers Teaser (7)", true, "teaser"),
    example!("Sampson's Monastery, Liking (18)", true, "sampson"),
    example!("Florida Bay Food Web (128)", true, "foodweb"),
    example!("Karate Club (34)", false, "karate"),
    example!("Dolphins (62)", false, "dolphins"),
    example!("Political Books (105)", false, "polbooks"),
    example!("College Football (115)", false, "football"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_example_loads_with_its_size() {
        let sizes: Vec<(&str, usize, usize, bool)> = EXAMPLES
            .iter()
            .map(|e| {
                let g = e.graph();
                (e.name, g.len(), g.arcs().count(), g.is_directed())
            })
            .collect();
        // The counts of the sources; undirected graphs store both directions
        // of each edge.
        assert_eq!(
            sizes,
            [
                ("Fauci Emails (32)", 32, 63, true),
                ("Gleich 2006 Example (11)", 11, 33, true),
                ("Magnetic Powers Teaser (7)", 7, 13, true),
                ("Sampson's Monastery, Liking (18)", 18, 97, true),
                ("Florida Bay Food Web (128)", 128, 2137, true),
                ("Karate Club (34)", 34, 156, false),
                ("Dolphins (62)", 62, 318, false),
                ("Political Books (105)", 105, 882, false),
                ("College Football (115)", 115, 1226, false),
            ]
        );
    }

    #[test]
    fn the_directed_flags_match_the_graphs() {
        for e in EXAMPLES {
            assert_eq!(e.directed, e.graph().is_directed(), "{}", e.name);
        }
    }
}
