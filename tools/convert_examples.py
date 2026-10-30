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

  * seven classic directed networks from Netzschleuder (Tiago P. Peixoto's
    network catalogue, https://networks.skewed.de), downloaded from it (see
    each entry there for the full citation):
      - McGrew's preschool toy competitions (W. C. McGrew, 1972), and
      - Guhl's White Leghorn hens peck order (A. M. Guhl, 1946),
        both reversed so that an arc points to the winner (PageRank then ranks
        the dominant animals and children highest);
      - Krackhardt's high-tech managers, advice layer (D. Krackhardt, 1987):
        an arc from a manager to one they go to for advice;
      - Moreno's sociograms, second grade (J. L. Moreno and H. H. Jennings,
        1934): an arc from a pupil to one they want to sit next to;
      - the Galesburg physicians, discussion ties (Coleman, Katz and Menzel,
        1966): an arc from a physician to one they discuss cases with;
      - Madoff's feeder funds (V. Krebs, 2009): arcs follow the money;
      - the World Cup 1998 player market (L. Krempel): an arc from a national
        team's country to a country where its players had club contracts.
    Weights, layers other than the one named, and duplicate arcs are dropped.
  * the five graphs of H. Bott's (1928) table of social responses among eleven
    nursery-school children (tools/bott_table2.py, transcribed from the printed
    table): an arc from a child to another toward whom they showed the
    behaviour, at least twice for talking, interference and watching, and at
    least once for the rarer imitation and cooperation.

Usage: tools/convert_examples.py <path to the eigendeck repo>
"""
import csv
import io
import json
import pathlib
import re
import sqlite3
import sys
import urllib.request
import zipfile

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


def netzschleuder(net, sub):
    """Returns the (nodes, edges) rows of a Netzschleuder network, as dicts."""
    url = f"https://networks.skewed.de/net/{net}/files/{sub}.csv.zip"
    with urllib.request.urlopen(url, timeout=60) as response:
        archive = zipfile.ZipFile(io.BytesIO(response.read()))

    def rows(name):
        reader = csv.reader(io.TextIOWrapper(archive.open(name), "utf-8"))
        header = [h.strip().lstrip("#").strip() for h in next(reader)]
        return [dict(zip(header, (c.strip() for c in row))) for row in reader]

    return rows("nodes.csv"), rows("edges.csv")


def position(text):
    """Parses a Netzschleuder `_pos` value such as 'array([1.0, -2.0])'."""
    x, y = (float(v) for v in re.findall(r"-?[\d.]+(?:e-?\d+)?", text)[:2])
    return (x * 100.0, -y * 100.0)


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

    numbers = lambda nodes: [str(i + 1) for i in range(len(nodes))]
    for name, net, sub, label, keep, reverse in [
        ("mcgrew", "mcgrew_preschool", "mcgrew_preschool", lambda n: [v["name"] for v in n],
         lambda e: True, True),
        ("hens", "hens", "hens", numbers, lambda e: True, True),
        ("krackhardt", "high_tech_company", "high_tech_company", numbers,
         lambda e: e["layer"] == "1", False),
        ("moreno2", "moreno_sociograms", "grade_2", lambda n: [v["name"] for v in n],
         lambda e: True, False),
        ("galesburg", "galesburg_physicians", "discussion", lambda n: [v["name"] for v in n],
         lambda e: True, False),
        ("madoff", "madoff_feeders", "madoff_feeders",
         lambda n: unique([initials(v["name"].replace("_", " ")) for v in n]), lambda e: True, False),
        ("worldcup", "world_cup_players", "world_cup_players", lambda n: [v["name"] for v in n],
         lambda e: True, False),
    ]:
        nodes, edges = netzschleuder(net, sub)
        arcs = [(int(e["source"]), int(e["target"])) for e in edges if keep(e)]
        if reverse:
            arcs = [(t, s) for s, t in arcs]
        write(name, True, unique(label(nodes)), arcs, pos=[position(v["_pos"]) for v in nodes])

    sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
    import bott_table2 as bott
    for k, (name, least) in enumerate([("talking", 2), ("interference", 2), ("watching", 2),
                                        ("imitation", 1), ("cooperation", 1)]):
        index = {c: i for i, c in enumerate(bott.KIDS)}
        arcs = [(index[r], index[c]) for r in bott.KIDS for c, v in bott.T[r].items()
                if v[k] >= least]
        write(f"bott_{name}", True, list(bott.KIDS), arcs)

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
