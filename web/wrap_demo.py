#!/usr/bin/env python3
"""Wraps the wasm-bindgen output into one self-contained HTML page that runs
as an Eigendeck demo (or in any browser): the wasm module inlined as base64
and the JavaScript glue inlined as a module script.

Usage: web/wrap_demo.py <bindgen dir> <wasm file> <output.html>
"""
import base64
import re
import sys

bindgen, wasm, out = sys.argv[1:4]
# The glue is an ES module; turn it into plain script code with the exports
# removed. This depends on the shape of the wasm-bindgen output, so re-check it
# when upgrading wasm-bindgen (the assert below fails if an export is left).
glue = open(f"{bindgen}/centrality_explorer.js").read()
glue = re.sub(r"^export \{[^}]*\};?\s*$", "", glue, flags=re.M)
glue = re.sub(r"^export (function|async function|class|const|let)", r"\1", glue, flags=re.M)
assert not re.search(r"^export ", glue, re.M), "unexpected export left in the glue"
b64 = base64.b64encode(open(wasm, "rb").read()).decode()

page = f"""<!DOCTYPE html>
<!--eigendeck-demo-v1-->
<html>
<head>
<meta charset="utf-8">
<title>Centrality Explorer</title>
<!-- Centrality Explorer by Sebastiano Vigna (github.com/vigna/centrality-explorer-rs,
     Apache-2.0 OR LGPL-2.1-or-later), web build from github.com/dgleich/centrality-explorer-rs. -->
<style>
  html, body {{ margin: 0; width: 100%; height: 100%; overflow: hidden; background: #fff; }}
  #c {{ display: block; width: 100%; height: 100%; outline: none; }}
  #msg {{ position: absolute; inset: 0; display: flex; align-items: center; justify-content: center;
          font: 20px sans-serif; color: #555; pointer-events: none; }}
</style>
</head>
<body>
<canvas id="c"></canvas>
<div id="msg">Loading Centrality Explorer…</div>
<script id="wasm" type="application/octet-stream">{b64}</script>
<script type="module">
{glue}
try {{
  const bin = atob(document.getElementById('wasm').textContent);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  await __wbg_init({{ module_or_path: bytes }});
  await start(document.getElementById('c'));
  document.getElementById('msg').remove();
}} catch (e) {{
  document.getElementById('msg').textContent = 'Centrality Explorer failed to start: ' + e;
  console.error(e);
}}
</script>
</body>
</html>
"""
open(out, "w").write(page)
print(f"wrote {out} ({len(page) / 1e6:.1f} MB)")
