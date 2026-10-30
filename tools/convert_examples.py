#!/usr/bin/env python3
"""Convert the example graphs bundled with the web build into the Centrality
Explorer file format (version 1), writing data/examples/*.json.

Sources (from the Eigendeck repo, github.com/dgleich/eigendeck):
  * the Fauci email replied-to network, example-demos/magnetic-powers/graphs/
    fauci-email-graph.json (Benson, Veldt and Gleich, arXiv:2108.01239);
  * the other directed graphs of example-demos/magnetic-powers/graphs: the
    two-component example of Gleich (2006), the seven-node teaser graph of the
    magnetic matrix powers demo, Sampson's monastery (positive ties only; the
    explorer has no signs), and the Florida Bay food web (dry season,
    unweighted). The C. elegans neural network (297 nodes) is left out: at
    that size the plot is unreadable and each recomputation takes seconds;
  * karate, dolphins, political books and college football, from the
    COMMUNITY_DATA block of the community-examples demo in
    examples/local-networks.eigendeck.

Usage: tools/convert_examples.py <path to the eigendeck repo>
"""
import json
import pathlib
import re
import sqlite3
import sys

PALETTE_SIZE = 12
OUT = pathlib.Path(__file__).resolve().parent.parent / "data" / "examples"


def write(name, directed, labels, arcs, pos=None):
    n = len(labels)
    nodes = [{"id": i, "label": labels[i], "pos": list(pos[i]) if pos else [0.0, 0.0],
              "color": i % PALETTE_SIZE} for i in range(n)]
    doc = {"version": 1, "directed": directed, "alpha": 0.85, "centrality": "pagerank",
           "katz": 0.85, "seed": None, "nodes": nodes, "arcs": [list(a) for a in arcs]}
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / f"{name}.json").write_text(json.dumps(doc, separators=(",", ":")) + "\n")
    print(f"{name}: {n} nodes, {len(arcs)} {'arcs' if directed else 'edges'}")


def unique(labels):
    """Makes labels unique by appending a digit to repeats."""
    seen, out = {}, []
    for label in labels:
        seen[label] = seen.get(label, 0) + 1
        out.append(label if seen[label] == 1 else f"{label}{seen[label]}")
    return out


def initials(name):
    return "".join(w[0] for w in re.split(r"[\s.-]+", name) if w).upper()[:3]


def main(eigendeck):
    root = pathlib.Path(eigendeck)
    fauci = json.loads((root / "example-demos/magnetic-powers/graphs/fauci-email-graph.json").read_text())
    nodes = sorted(fauci["nodes"], key=lambda v: v["id"])
    index = {v["id"]: i for i, v in enumerate(nodes)}
    write("fauci", True, unique([initials(v["label"]) for v in nodes]),
          [(index[e["source"]], index[e["target"]]) for e in fauci["edges"]],
          pos=[(v["x"], v["y"]) for v in nodes])

    magnetic = root / "example-demos/magnetic-powers/graphs"
    for name, file, label, keep in [
        ("gleich2006", "gleich-directed-2006-example", lambda v: v["label"], lambda e: True),
        ("teaser", "powers-teaser-7node", lambda v: v["label"], lambda e: True),
        ("sampson", "sampson-monastery", lambda v: v["label"][:3], lambda e: e["weight"] > 0),
        ("foodweb", "foodweb-baydry", lambda v: v["label"].split()[-1], lambda e: True),
    ]:
        g = json.loads((magnetic / f"{file}.json").read_text())
        nodes = sorted(g["nodes"], key=lambda v: v["id"])
        index = {v["id"]: i for i, v in enumerate(nodes)}
        write(name, True, unique([label(v) for v in nodes]),
              [(index[e["source"]], index[e["target"]]) for e in g["edges"] if keep(e)],
              pos=[(v["x"], v["y"]) for v in nodes])

    db = sqlite3.connect(f"file:{root / 'examples/local-networks.eigendeck'}?mode=ro", uri=True)
    html = db.execute("select data from assets where valid_to is null and path = ?",
                      ("demos/community-examples.html",)).fetchone()[0].decode()
    block = html[html.index("__COMMUNITY_DATA_START__"):html.index("__COMMUNITY_DATA_END__")]
    data = json.loads(re.search(r"=\s*(\{.*\})", block, re.S).group(1))
    numbers = lambda n: [str(i + 1) for i in range(n)]
    for key, name, labels in [
        ("karate", "karate", lambda g: g["labels"]),
        ("dolphins", "dolphins", lambda g: unique([s[:3] for s in g["labels"]])),
        ("polbooks", "polbooks", lambda g: numbers(g["n"])),
        ("football", "football", lambda g: numbers(g["n"])),
    ]:
        g = data[key]
        write(name, False, labels(g), [tuple(e) for e in g["edges"]])


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "../eigendeck")
