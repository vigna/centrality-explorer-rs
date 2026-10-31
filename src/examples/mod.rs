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
    example!("Small Example (7)", true, "small7"),
    example!("Sampson's Monastery, Liking (18)", true, "sampson"),
    example!("Florida Bay Food Web (128)", true, "foodweb"),
    example!("McGrew Preschool, Toy Wins (19)", true, "mcgrew"),
    example!("Hens' Peck Order (32)", true, "hens"),
    example!("Krackhardt High-Tech, Advice (21)", true, "krackhardt"),
    example!("Moreno Sociogram, Grade 2 (29)", true, "moreno2"),
    example!("Galesburg Physicians (31)", true, "galesburg"),
    example!("Madoff Feeder Funds (61)", true, "madoff"),
    example!("World Cup 1998 Players (35)", true, "worldcup"),
    example!("Bott 1928, Talking (11)", true, "bott_talking"),
    example!("Bott 1928, Interference (11)", true, "bott_interference"),
    example!("Bott 1928, Watching (11)", true, "bott_watching"),
    example!("Bott 1928, Imitation (11)", true, "bott_imitation"),
    example!("Bott 1928, Cooperation (11)", true, "bott_cooperation"),
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
                ("Small Example (7)", 7, 13, true),
                ("Sampson's Monastery, Liking (18)", 18, 97, true),
                ("Florida Bay Food Web (128)", 128, 2137, true),
                ("McGrew Preschool, Toy Wins (19)", 19, 32, true),
                ("Hens' Peck Order (32)", 32, 496, true),
                ("Krackhardt High-Tech, Advice (21)", 21, 190, true),
                ("Moreno Sociogram, Grade 2 (29)", 29, 59, true),
                ("Galesburg Physicians (31)", 31, 82, true),
                ("Madoff Feeder Funds (61)", 61, 60, true),
                ("World Cup 1998 Players (35)", 35, 118, true),
                ("Bott 1928, Talking (11)", 11, 57, true),
                ("Bott 1928, Interference (11)", 11, 48, true),
                ("Bott 1928, Watching (11)", 11, 27, true),
                ("Bott 1928, Imitation (11)", 11, 35, true),
                ("Bott 1928, Cooperation (11)", 11, 23, true),
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
