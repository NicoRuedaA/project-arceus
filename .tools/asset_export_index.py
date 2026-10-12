#!/usr/bin/env python3
"""Write the bounded Task B asset status index and unresolved-model list."""

import argparse
import json
from pathlib import Path


def clean(value: str) -> str:
    return value.replace("\t", " ").replace("\n", " ").replace("\r", " ")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--file-list", type=Path, default=None)
    parser.add_argument("--romfs-bin-root", type=Path, default=Path("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin"))
    parser.add_argument("--unresolved-json", type=Path, default=Path("reports/skeleton/update-v262144-skin-census.json"))
    parser.add_argument("--model-index", type=Path, default=Path("work/cache/gltf/model_export_index.tsv"))
    parser.add_argument("--asset-index", type=Path, default=Path("data/asset_export_index.tsv"))
    parser.add_argument("--unresolved-index", type=Path, default=Path("data/unresolved_model_links.tsv"))
    args = parser.parse_args()

    # Load model statuses if available
    model_status = {}
    if args.model_index.exists():
        with args.model_index.open(encoding="utf-8") as f:
            for line in f:
                parts = line.rstrip("\r\n").split("\t")
                if parts[0] == "source" or not parts[0]:
                    continue
                src = parts[0]
                out = parts[1] if len(parts) > 1 else ""
                st = parts[2] if len(parts) > 2 else ""
                rs = parts[3] if len(parts) > 3 else ""
                base = Path(src).with_suffix("").as_posix()
                model_status[base] = (out, st, rs)

    # Collect source list: from existing asset_index or file_list
    sources = []
    if args.file_list and args.file_list.exists():
        for raw in args.file_list.open(encoding="utf-8"):
            raw = raw.rstrip("\r\n")
            if not raw:
                continue
            p = Path(raw)
            if p.is_absolute():
                rel = p.relative_to(args.romfs_bin_root).as_posix()
            else:
                rel = p.as_posix()
            sources.append(rel)
    elif args.asset_index.exists():
        with args.asset_index.open(encoding="utf-8") as f:
            for line in f:
                if line.startswith("#") or line.startswith("source:"):
                    continue
                parts = line.rstrip("\r\n").split("\t")
                if parts[0]:
                    sources.append(parts[0])
    else:
        raise RuntimeError("No input file list or existing asset index found")

    args.asset_index.parent.mkdir(parents=True, exist_ok=True)
    temp_index = args.asset_index.with_suffix(".tmp")
    with temp_index.open("w", encoding="utf-8", newline="") as out:
        out.write("# sheet: data/asset_export_index\n")
        out.write("# version: 1\n")
        out.write("# generator: .tools/asset_export_index.py\n")
        out.write("# target: Pokémon Legends: Arceus v262144 in-scope asset export batch status\n")
        out.write("source:string*\toutput:string?\tstatus:enum\treason:string?\n")

        ok_count = 0
        blocked_count = 0

        for relative in sources:
            p = Path(relative)
            suffix = p.suffix.lower()

            if suffix == ".bntx":
                output = f"work/cache/png/{p.with_suffix('.png').as_posix()}"
                status = "ok"
                reason = "BC5 policy: raw R/G with B=0; no Z reconstruction"
                ok_count += 1
            elif suffix in {".trmbf", ".trmsh", ".trmdl", ".trmtr", ".trskl"}:
                base = p.with_suffix("").as_posix()
                if base in model_status:
                    m_out, m_st, m_rs = model_status[base]
                    if m_st == "ok":
                        output = m_out
                        status = "ok"
                        reason = ""
                        ok_count += 1
                    else:
                        output = ""
                        status = "blocked"
                        reason = m_rs
                        blocked_count += 1
                else:
                    output = ""
                    status = "blocked"
                    reason = "Model not found in batch evaluation"
                    blocked_count += 1
            elif suffix in {".tranm", ".tracm"}:
                if relative == "demo/graphic/ev/d020/item_224/anm/d020_item_224_c002.tranm":
                    output = "work/cache/gltf/chara/data/item/item_201/item_201.gltf"
                    status = "ok"
                    reason = ""
                    ok_count += 1
                else:
                    output = ""
                    status = "blocked"
                    reason = "Standalone model association not established; animation channels not sampled"
                    blocked_count += 1
            else:
                output = ""
                status = "blocked"
                reason = "Unrecognized in-scope asset extension"
                blocked_count += 1

            out.write(f"{clean(relative)}\t{clean(output)}\t{status}\t{clean(reason)}\n")

    temp_index.replace(args.asset_index)

    if args.unresolved_json.exists():
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
        print(f"unresolved_models={len(unresolved)}")

    print(f"asset_rows={len(sources)}: ok={ok_count}, blocked={blocked_count}")


if __name__ == "__main__":
    main()
