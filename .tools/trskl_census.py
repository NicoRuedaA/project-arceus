#!/usr/bin/env python3
"""Independent metadata-only oracle for the demonstrated update skeleton lane.

Reads authorized local-private RomFS input; reports contain relative paths,
hashes and counts, never payload bytes or transforms. Unsupported data fails.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import struct


class Skeleton:
    def __init__(self, data):
        if len(data) > 32 * 1024 * 1024:
            raise ValueError("skeleton exceeds input cap")
        self.data = data

    def block(self, at, size):
        if at < 0 or size < 0 or at + size > len(self.data):
            raise ValueError("range outside skeleton")
        return self.data[at:at + size]

    def u32(self, at):
        return struct.unpack('<I', self.block(at, 4))[0]

    def table(self, at, maximum):
        delta = struct.unpack('<i', self.block(at, 4))[0]
        vt = at - delta
        size, extent = struct.unpack('<HH', self.block(vt, 4))
        if not delta or at % 4 or vt % 2 or size < 4 or size % 2 or size > 4 + 2 * maximum or extent < 4:
            raise ValueError("invalid or unsupported table")
        self.block(at, extent)
        fields = struct.unpack('<' + 'H' * ((size - 4) // 2), self.block(vt + 4, size - 4))
        return at, extent, fields

    def field(self, table, index, width, required=True):
        at, extent, fields = table
        offset = fields[index] if index < len(fields) else 0
        if not offset:
            if required:
                raise ValueError("missing field")
            return None
        if offset < 4 or offset + width > extent or (width >= 4 and (at + offset) % 4):
            raise ValueError("invalid field extent/alignment")
        return at + offset

    def target(self, at):
        delta = self.u32(at)
        if delta < 4:
            raise ValueError("invalid relative offset")
        self.block(at + delta, 4)
        return at + delta

    def vector(self, table, index):
        at = self.target(self.field(table, index, 4))
        count = self.u32(at)
        if count > 8192:
            raise ValueError("vector exceeds supported cap")
        self.block(at + 4, 4 * count)
        return [self.target(at + 4 + 4 * i) for i in range(count)]

    def index(self, table, index):
        at = self.field(table, index, 4, required=False)
        value = -1 if at is None else struct.unpack('<i', self.block(at, 4))[0]
        if value < -1:
            raise ValueError("invalid signed index")
        return value

    def vec3(self, table, index):
        if not all(math.isfinite(v) for v in struct.unpack('<3f', self.block(self.field(table, index, 12), 12))):
            raise ValueError("non-finite local transform/pivot")

    def summary(self):
        root = self.table(self.target(0), 5)
        at = self.field(root, 0, 4, required=False)
        flag = 0 if at is None else self.u32(at)
        at = self.field(root, 4, 4, required=False)
        if flag > 1 or (at is not None and self.u32(at)) or self.vector(root, 3):
            raise ValueError("unsupported root/rig offset/IK")
        nodes, binds = self.vector(root, 1), self.vector(root, 2)
        for bind in binds:
            self.table(bind, 3)
        roots, rigs = 0, []
        for i, node in enumerate(nodes):
            table = self.table(node, 8)
            at = self.field(table, 7, 4, required=False)
            if at is not None and self.u32(at):
                raise ValueError("unsupported node type")
            text = self.target(self.field(table, 0, 4))
            size = self.u32(text)
            if size > 1024:
                raise ValueError("name exceeds supported size limit")
            name = self.block(text + 4, size + 1)
            if not size or name[-1] or 0 in name[:-1]:
                raise ValueError("invalid name")
            name[:-1].decode('utf-8', errors='strict')
            locator = self.target(self.field(table, 6, 4))
            if self.u32(locator) or self.block(locator + 4, 1) != b'\0':
                raise ValueError("unsupported locator attachment")
            transform = self.table(self.target(self.field(table, 1, 4)), 3)
            for j in range(3):
                self.vec3(transform, j)
            self.vec3(table, 2)
            self.vec3(table, 3)
            parent, rig = self.index(table, 4), self.index(table, 5)
            if parent >= i:
                raise ValueError("unsupported parent order/cycle")
            roots += parent == -1
            if rig >= 0:
                rigs.append(rig)
        if not nodes or roots != 1 or sorted(rigs) != list(range(len(binds))):
            raise ValueError("unsupported root/rig mapping")
        return {"nodes": len(nodes), "binds": len(binds), "roots": roots, "root_flag": flag}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('romfs', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    root = args.romfs.resolve(strict=True)
    rows = []
    for path in sorted(root.rglob('*.trskl')):
        # Refuse symlinks escaping the authorized input boundary.
        path.resolve(strict=True).relative_to(root)
        if path.stat().st_size > 32 * 1024 * 1024:
            raise ValueError("skeleton exceeds input cap")
        data = path.read_bytes()
        rows.append({"path": path.relative_to(root).as_posix(), "sha256": hashlib.sha256(data).hexdigest(),
                     "bytes": len(data), **Skeleton(data).summary()})
    if not rows:
        raise ValueError("no skeleton inputs found")
    report = {"format": "trskl-structural-census/v1", "input_scope": "authorized local RomFS; identity requires matching input hashes",
              "verification": "structural serialization only; no runtime/binary-match proof",
              "files": len(rows), "nodes": sum(r['nodes'] for r in rows),
              "binds": sum(r['binds'] for r in rows), "empty_binds": sum(r['binds'] == 0 for r in rows),
              "root_flag_one": sum(r['root_flag'] == 1 for r in rows), "references": rows}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({key: value for key, value in report.items() if key != 'references'}, sort_keys=True))


if __name__ == '__main__':
    main()
