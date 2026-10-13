#!/usr/bin/env python3
"""Decode the bounded Pokémon rare/size app-config tables.

The files are FlatBuffers with a root vector of parameter rows.  This decoder
preserves the raw strings and the opaque 12-byte field; it does not assign a
native probability or claim that the display-oriented scale keys are model
scale categories.
"""

from __future__ import annotations

import argparse
import struct
from pathlib import Path


def fields(data: bytes, table: int) -> dict[int, int]:
    vtable = table - struct.unpack_from("<i", data, table)[0]
    vtable_size, _object_size = struct.unpack_from("<HH", data, vtable)
    count = (vtable_size - 4) // 2
    return {
        index: table + struct.unpack_from("<H", data, vtable + 4 + 2 * index)[0]
        for index in range(count)
        if struct.unpack_from("<H", data, vtable + 4 + 2 * index)[0]
    }


def root_rows(data: bytes) -> list[dict[int, int]]:
    root = struct.unpack_from("<I", data, 0)[0]
    root_fields = fields(data, root)
    vector_field = root_fields[0]
    vector = vector_field + struct.unpack_from("<I", data, vector_field)[0]
    count = struct.unpack_from("<I", data, vector)[0]
    rows = []
    for index in range(count):
        element = vector + 4 + index * 4
        rows.append(fields(data, element + struct.unpack_from("<I", data, element)[0]))
    return rows


def string_at(data: bytes, field: int) -> str:
    string = field + struct.unpack_from("<I", data, field)[0]
    length = struct.unpack_from("<I", data, string)[0]
    end = string + 4 + length
    if end >= len(data) or data[end] != 0:
        raise ValueError(f"invalid string at 0x{field:x}")
    return data[string + 4 : end].decode("ascii")


def string_vector(data: bytes, field: int) -> list[str]:
    vector = field + struct.unpack_from("<I", data, field)[0]
    count = struct.unpack_from("<I", data, vector)[0]
    return [string_at(data, vector + 4 + index * 4) for index in range(count)]


def decode_file(path: Path, source_name: str, kind: str) -> list[dict[str, str]]:
    data = path.read_bytes()
    rows = []
    for row in root_rows(data):
        key = string_at(data, row[0])
        values = string_vector(data, row[5])
        opaque = data[row[1] : row[1] + 12].hex()
        type_code = str(data[row[2]]) if 2 in row else ""
        rows.append(
            {
                "source_file": source_name,
                "kind": kind,
                "key": key,
                "value": values[0] if values else "",
                "value_min": string_at(data, row[3]),
                "value_max": string_at(data, row[4]),
                "type_code": type_code,
                "opaque_id_hex": opaque,
                "raw_confidence": "VERIFIED",
                "meaning_confidence": "UNKNOWN" if kind == "rare" else "PROBABLE",
                "note": "raw FlatBuffer strings; parameter meaning is not claimed",
            }
        )
    return rows


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("romfs_bin", type=Path, help="the overlay bin directory")
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    config = args.romfs_bin / "misc" / "app_config"
    rows = decode_file(config / "pokemon_rare.bin", "pokemon_rare.bin", "rare")
    rows += decode_file(
        config / "pokemon_size_category_adjust_scale_config.bin",
        "pokemon_size_category_adjust_scale_config.bin",
        "size",
    )
    rows.sort(key=lambda row: (row["source_file"], row["key"]))
    with args.output.open("w", encoding="utf-8", newline="") as stream:
        stream.write("# sheet: data/pokemon_variants\n")
        stream.write("# version: 1\n")
        stream.write("# generator: .tools/decode_pokemon_variants.py\n")
        stream.write("# target: Pokémon Legends: Arceus v262144 app-config raw rows\n")
        stream.write(
            "source_file:string*\tkind:enum\tkey:string*\tvalue:string?\tvalue_min:string?\tvalue_max:string?\t"
            "type_code:u8?\topaque_id_hex:string*\traw_confidence:enum\tmeaning_confidence:enum\tnote:string?\n"
        )
        for row in rows:
            stream.write("\t".join(row[field] for field in (
                "source_file", "kind", "key", "value", "value_min", "value_max", "type_code",
                "opaque_id_hex", "raw_confidence", "meaning_confidence", "note",
            )) + "\n")
    print(f"decoded {len(rows)} rows: rare=5 size=56 -> {args.output}")


if __name__ == "__main__":
    main()
