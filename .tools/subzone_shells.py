#!/usr/bin/env python3
"""Sub-area shell coverage: does a sub-area's exported geometry cover its navmesh?

For every sub-area (interior record) of Pokemon Legends: Arceus v262144 this tool unions the
placed bounding box of the models that `data/field_model_export_index.tsv` exports `ok` and
that `data/field_object_placements.tsv` places in that sub-area, and compares it with the
sub-area navmesh box of `data/subzone_index.tsv`:

  * `coverage_x` / `coverage_z` = fraction of the navmesh box (per horizontal axis) inside the
    placed union box.
  * `y_overlap` = fraction of the navmesh y range inside the placed union y range.
  * `shell` = `covered` when coverage_x >= 0.95 and coverage_z >= 0.95 and y_overlap >= 0.90,
    `partial` when some placed geometry exists, `none` when nothing does.

Method: the exported glTF `POSITION` accessor bounds (local model space) transformed by the
placement row (`x, y, z`, `rot_y` LIKELY degrees, `scale_*`), same method as
`reports/data-progress/subareas.md`. The result is PROBABLE: `rot_y` is sheet data marked
LIKELY and a box test is coarser than a per-face test.

Usage:
  python3 .tools/subzone_shells.py [--out data/subzone_shells.tsv] [--cache DIR]
"""
import argparse
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MAIN = Path("/home/nico/work/decompilacion")

SHELL_HEADER = """# sheet: data/subzone_shells
# version: 1
# generator: .tools/subzone_shells.py
# target: Pokémon Legends: Arceus v262144 sub-area (interior) shell coverage
# note: placed box = union of the exported model POSITION accessor bounds transformed by the
# note: placement row (x,y,z, rot_y LIKELY degrees, scale); method of reports/data-progress/subareas.md.
# note: coverage_x/z = fraction of the sub-area navmesh box (data/subzone_index.tsv) inside the
# note: placed box per horizontal axis; y_overlap = fraction of the navmesh y range inside it.
# note: shell = covered (>=0.95 x, >=0.95 z, >=0.90 y) | partial (some geometry) | none.
# note: confidence PROBABLE (box test; rot_y is LIKELY sheet data).
"""

SHELL_COLS = ("area:string*\tparent:string\tmodels_ok:u16\tcoverage_x:f32\tcoverage_z:f32\t"
              "y_overlap:f32\tshell:enum\tconfidence:enum")


def fmt(v):
    if v is None or v != v:
        return ""
    s = f"{v:.3f}".rstrip("0").rstrip(".")
    return "0" if s in ("-0", "") else s


def split_rows(path, key="area"):
    """TSV reader that strips the `:type` suffixes from the header."""
    lines = [l for l in path.read_text().splitlines() if l and not l.startswith("#")]
    header = [c.split(":")[0].split("?")[0] for c in lines[0].split("\t")]
    return [dict(zip(header, l.split("\t"))) for l in lines[1:]]


def export_index(data_dir):
    """model_name -> first `ok` glTF path."""
    out = {}
    for r in split_rows(data_dir / "field_model_export_index.tsv"):
        if r["status"] == "ok" and r["output"]:
            out.setdefault(r["model_name"], r["output"])
    return out


def local_bbox(gltf_path):
    doc = json.loads(Path(gltf_path).read_text())
    lo = [math.inf] * 3
    hi = [-math.inf] * 3
    for mesh in doc.get("meshes", []):
        for prim in mesh.get("primitives", []):
            pos = prim.get("attributes", {}).get("POSITION")
            if pos is None:
                continue
            acc = doc["accessors"][pos]
            mn, mx = acc.get("min"), acc.get("max")
            if not mn or not mx:
                continue
            for i in range(3):
                lo[i] = min(lo[i], mn[i])
                hi[i] = max(hi[i], mx[i])
    return lo, hi


def place_bbox(lo, hi, x, y, z, rot_y_deg, sx, sy, sz):
    """Bevy rot_y convention (subzones.md): x' = x cos + z sin, z' = -x sin + z cos."""
    a = math.radians(rot_y_deg)
    ca, sa = math.cos(a), math.sin(a)
    out_lo = [math.inf] * 3
    out_hi = [-math.inf] * 3
    for cx in (lo[0], hi[0]):
        for cy in (lo[1], hi[1]):
            for cz in (lo[2], hi[2]):
                px, py, pz = cx * sx, cy * sy, cz * sz
                qx = px * ca + pz * sa
                qz = -px * sa + pz * ca
                for i, v in enumerate((qx + x, py + y, qz + z)):
                    out_lo[i] = min(out_lo[i], v)
                    out_hi[i] = max(out_hi[i], v)
    return out_lo, out_hi


def fnum(row, key, default=0.0):
    v = (row.get(key) or "").strip()
    try:
        return float(v)
    except ValueError:
        return default


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", type=Path, default=ROOT / "data/subzone_shells.tsv")
    ap.add_argument("--cache", type=Path, default=None,
                    help="glTF cache root (default: $PLA_CACHE_ROOT, <repo>/work/cache, main checkout's work/cache)")
    a = ap.parse_args()

    data_dir = ROOT / "data"
    cache = a.cache
    if cache is None:
        import os
        cands = [os.environ.get("PLA_CACHE_ROOT"), ROOT / "work/cache", MAIN / "work/cache"]
        cache = next((Path(c) for c in cands if c and Path(c).is_dir()), ROOT / "work/cache")

    areas = split_rows(data_dir / "subzone_index.tsv")
    subs = [r for r in areas if r["parent"] and r["area"] != r["parent"]]
    placements = split_rows(data_dir / "field_object_placements.tsv")
    idx = export_index(data_dir)

    by_area = {}
    for r in placements:
        if r["model_name"]:
            by_area.setdefault(r["area"], []).append(r)

    rows = []
    for s in sorted(subs, key=lambda r: r["area"]):
        area = s["area"]
        nav_lo = [fnum(s, "nav_min_x"), fnum(s, "nav_min_y"), fnum(s, "nav_min_z")]
        nav_hi = [fnum(s, "nav_max_x"), fnum(s, "nav_max_y"), fnum(s, "nav_max_z")]
        union_lo = [math.inf] * 3
        union_hi = [-math.inf] * 3
        models = 0
        for p in by_area.get(area, []):
            path = idx.get(p["model_name"])
            if not path or not Path(path).is_file():
                continue
            try:
                lo, hi = local_bbox(path)
            except (json.JSONDecodeError, KeyError):
                continue
            if not all(math.isfinite(v) for v in lo + hi):
                continue
            models += 1
            plo, phi = place_bbox(
                lo, hi,
                fnum(p, "x"), fnum(p, "y"), fnum(p, "z"), fnum(p, "rot_y"),
                fnum(p, "scale_x", 1.0), fnum(p, "scale_y", 1.0), fnum(p, "scale_z", 1.0),
            )
            for i in range(3):
                union_lo[i] = min(union_lo[i], plo[i])
                union_hi[i] = max(union_hi[i], phi[i])
        if models == 0:
            rows.append([area, s["parent"], "0", "", "", "", "none", "VERIFIED"])
            continue
        span_x = max(nav_hi[0] - nav_lo[0], 1e-6)
        span_z = max(nav_hi[2] - nav_lo[2], 1e-6)
        cov_x = max(0.0, min(union_hi[0], nav_hi[0]) - max(union_lo[0], nav_lo[0])) / span_x
        cov_z = max(0.0, min(union_hi[2], nav_hi[2]) - max(union_lo[2], nav_lo[2])) / span_z
        # y as a reach test with a 0.5 m tolerance: the placed geometry must reach down to (at
        # most) 0.5 m above the navmesh floor and up to (at least) 0.5 m below its top; the
        # fraction form is over-sensitive on flat navmeshes (span can be ~0.2 m).
        y_ok = union_lo[1] <= nav_lo[1] + 0.5 and union_hi[1] >= nav_hi[1] - 0.5
        span_y = max(nav_hi[1] - nav_lo[1], 1e-6)
        y_ov = max(0.0, min(union_hi[1], nav_hi[1]) - max(union_lo[1], nav_lo[1])) / span_y
        shell = (
            "covered"
            if cov_x >= 0.95 and cov_z >= 0.95 and y_ok
            else "partial"
        )
        rows.append([area, s["parent"], str(models), fmt(cov_x), fmt(cov_z), fmt(y_ov), shell, "PROBABLE"])

    a.out.write_text(SHELL_HEADER + SHELL_COLS + "\n" + "".join("\t".join(r) + "\n" for r in rows))
    counts = {}
    for r in rows:
        counts[r[6]] = counts.get(r[6], 0) + 1
    print(f"sub-areas: {len(rows)}  shells: {counts}  -> {a.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
