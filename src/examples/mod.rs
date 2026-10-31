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
    /// What the graph is, and what an arc means; shown when hovering over the
    /// menu entry.
    pub description: &'static str,
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
    ($name:expr, $directed:expr, $file:literal, $description:expr) => {
        Example {
            name: $name,
            directed: $directed,
            description: $description,
            json: include_str!(concat!("../../data/examples/", $file, ".json")),
        }
    };
}

/// The built-in examples, in menu order, directed ones first. The first one
/// is shown when the web build starts.
pub const EXAMPLES: &[Example] = &[
    example!(
        "Fauci Emails (32)",
        true,
        "fauci",
        "The replied-to network of the Fauci emails: an arc from a person to someone whose message they replied to (Benson, Veldt and Gleich, arXiv:2108.01239)."
    ),
    example!(
        "Gleich 2006 Example (11)",
        true,
        "gleich2006",
        "A small directed example with two components, from Gleich's 2006 work."
    ),
    example!(
        "Small Example (7)",
        true,
        "small7",
        "A small directed example with seven nodes."
    ),
    example!(
        "Sampson's Monastery, Liking (18)",
        true,
        "sampson",
        "Sampson's monastery: novices' liking for one another. Only positive ties are kept (the explorer has no signs)."
    ),
    example!(
        "Florida Bay Food Web (128)",
        true,
        "foodweb",
        "The Florida Bay food web in the dry season, unweighted."
    ),
    example!(
        "McGrew Preschool, Toy Wins (19)",
        true,
        "mcgrew",
        "McGrew's preschool (1972): toy competitions among children. An arc points to the winner, so PageRank ranks the dominant children highest."
    ),
    example!(
        "Hens' Peck Order (32)",
        true,
        "hens",
        "Guhl's White Leghorn hens (1946): the peck order. An arc points to the winner, so PageRank ranks the dominant hens highest."
    ),
    example!(
        "Krackhardt High-Tech, Advice (21)",
        true,
        "krackhardt",
        "Krackhardt's high-tech managers (1987), advice layer: an arc from a manager to one they go to for advice."
    ),
    example!(
        "Moreno Sociogram, Grade 2 (29)",
        true,
        "moreno2",
        "Moreno's sociogram of a second-grade class (Moreno and Jennings, 1934): an arc from a pupil to one they want to sit next to."
    ),
    example!(
        "Galesburg Physicians (31)",
        true,
        "galesburg",
        "The Galesburg physicians (Coleman, Katz and Menzel, 1966), discussion ties: an arc from a physician to one they discuss cases with."
    ),
    example!(
        "Madoff Feeder Funds (61)",
        true,
        "madoff",
        "Madoff's feeder funds (Krebs, 2009): arcs follow the money."
    ),
    example!(
        "World Cup 1998 Players (35)",
        true,
        "worldcup",
        "The 1998 World Cup player market (Krempel): an arc from a national team's country to a country where its players had club contracts."
    ),
    example!(
        "Bott 1928, Talking (11)",
        true,
        "bott_talking",
        "Eleven nursery-school children (Bott, Blatz, Chant and Bott, 1928), talking: an arc from a child to another they talked to at least twice."
    ),
    example!(
        "Bott 1928, Interference (11)",
        true,
        "bott_interference",
        "Eleven nursery-school children (Bott et al., 1928), interference: an arc from a child to another they interfered with at least twice."
    ),
    example!(
        "Bott 1928, Watching (11)",
        true,
        "bott_watching",
        "Eleven nursery-school children (Bott et al., 1928), watching: an arc from a child to another they watched at least twice."
    ),
    example!(
        "Bott 1928, Imitation (11)",
        true,
        "bott_imitation",
        "Eleven nursery-school children (Bott et al., 1928), imitation: an arc from a child to another they imitated at least once."
    ),
    example!(
        "Bott 1928, Cooperation (11)",
        true,
        "bott_cooperation",
        "Eleven nursery-school children (Bott et al., 1928), cooperation: an arc from a child to another they cooperated with at least once."
    ),
    example!(
        "Karate Club (34)",
        false,
        "karate",
        "Zachary's karate club: friendships among the members of a university club that later split in two."
    ),
    example!(
        "Dolphins (62)",
        false,
        "dolphins",
        "Associations among 62 bottlenose dolphins living off New Zealand (Lusseau et al.)."
    ),
    example!(
        "Political Books (105)",
        false,
        "polbooks",
        "Books about US politics sold by an online bookseller, linked when frequently bought together by the same customers (Krebs)."
    ),
    example!(
        "College Football (115)",
        false,
        "football",
        "US college football: games played between Division I teams in a season (Girvan and Newman)."
    ),
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
    fn every_example_has_a_description() {
        assert!(EXAMPLES.iter().all(|e| !e.description.is_empty()));
    }

    #[test]
    fn the_directed_flags_match_the_graphs() {
        for e in EXAMPLES {
            assert_eq!(e.directed, e.graph().is_directed(), "{}", e.name);
        }
    }
}
