#!/usr/bin/env python3
"""Write the bounded Task B asset status index and unresolved-model list."""

import argparse
import json
from pathlib import Path


def clean(value: str) -> str:
    return value.replace("\t", " ").replace("\n", " ").replace("\r", " ")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("file_list", type=Path)
    parser.add_argument("romfs_bin_root", type=Path)
    parser.add_argument("unresolved_json", type=Path)
    parser.add_argument("asset_index", type=Path)
    parser.add_argument("unresolved_index", type=Path)
    args = parser.parse_args()

    args.asset_index.parent.mkdir(parents=True, exist_ok=True)
    with args.asset_index.open("w", encoding="utf-8", newline="") as out:
        out.write("# sheet: data/asset_export_index\n")
        out.write("# version: 1\n")
        out.write("# generator: .tools/asset_export_index.py\n")
        out.write("# target: Pokémon Legends: Arceus v262144 in-scope asset export batch status\n")
        out.write("source:string*\toutput:string?\tstatus:enum\treason:string?\n")
        for raw in args.file_list.open(encoding="utf-8"):
            raw = raw.rstrip("\r\n")
            if not raw:
                continue
            source = Path(raw)
            relative = source.relative_to(args.romfs_bin_root).as_posix()
            if source.suffix.lower() == ".bntx":
                output = f"work/cache/png/{Path(relative).with_suffix('.png').as_posix()}"
                reason = "BC5 policy: raw R/G with B=0; no Z reconstruction"
                status = "ok"
            else:
                output = ""
                status = "blocked"
                if source.suffix.lower() in {".trmdl", ".trmsh", ".trmbf", ".trmtr"}:
                    reason = "No glTF exporter; complete model-to-mesh/material binding is not established"
                elif source.suffix.lower() in {".trskl", ".tranm", ".tracm"}:
                    reason = "No glTF skin/animation exporter; animation channels were not sampled"
                else:
                    reason = "Unrecognized in-scope asset extension"
            out.write(
                f"{clean(relative)}\t{clean(output)}\t{status}\t{clean(reason)}\n"
            )

    report = json.loads(args.unresolved_json.read_text(encoding="utf-8"))
    unresolved = report.get("unresolved_models", [])
    args.unresolved_index.parent.mkdir(parents=True, exist_ok=True)
    with args.unresolved_index.open("w", encoding="utf-8", newline="") as out:
        out.write("# sheet: data/unresolved_model_links\n")
        out.write("# version: 1\n")
        out.write("# generator: .tools/asset_export_index.py\n")
        out.write("# target: bounded skin-census model association failures\n")
        out.write("model:string*\tstatus:enum\treason:string\n")
        for row in unresolved:
            out.write(
                f"{clean(str(row.get('model', '')))}\t"
                f"{clean(str(row.get('status', 'unknown')))}\t"
                f"{clean(str(row.get('reason', 'unknown')))}\n"
            )
    print(f"asset_rows={sum(1 for _ in args.asset_index.open(encoding='utf-8')) - 5}")
    print(f"unresolved_models={len(unresolved)}")


if __name__ == "__main__":
    main()
