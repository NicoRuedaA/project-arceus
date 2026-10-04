#!/usr/bin/env python3
"""Model-linked, metadata-only census of local TR skin channels.

Independent FlatBuffer traversal, not the Rust layout heuristic. Never emits
vertex payloads, matrices or game pseudocode. Unknown lanes remain explicit.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import struct
from trskl_census import Skeleton


def load(path, root):
    path.resolve(strict=True).relative_to(root)
    if path.stat().st_size > 32 * 1024 * 1024:
        raise ValueError('input exceeds 32 MiB cap')
    return Skeleton(path.read_bytes())


def scalar(reader, table, field):
    at = reader.field(table, field, 4, required=False)
    return 0 if at is None else reader.u32(at)


def text(reader, table, field):
    at = reader.target(reader.field(table, field, 4))
    size = reader.u32(at)
    if size > 1024:
        raise ValueError('reference exceeds name cap')
    data = reader.block(at + 4, size + 1)
    if data[-1] or 0 in data[:-1]:
        raise ValueError('invalid reference termination')
    return data[:-1].decode('utf-8')


def sibling(parent, name):
    path = Path(name)
    if path.is_absolute() or '..' in path.parts or not name:
        raise ValueError('invalid sibling reference')
    return parent / path


def identity(path, reader, root):
    return {'path': path.relative_to(root).as_posix(), 'sha256': hashlib.sha256(reader.data).hexdigest()}


def payload(reader, table, field):
    at = reader.target(reader.field(table, field, 4))
    return reader.block(at + 4, reader.u32(at))


def model_rows(path, root):
    model = load(path, root)
    mt = model.table(model.target(0), 7)
    skeleton_ref = model.table(model.target(model.field(mt, 2, 4)), 1)
    skeleton_path = sibling(path.parent, text(model, skeleton_ref, 0))
    skeleton = load(skeleton_path, root)
    skeleton_summary = skeleton.summary()
    model_id, skeleton_id = identity(path, model, root), identity(skeleton_path, skeleton, root)
    rows = []
    for mesh_ref in model.vector(mt, 1):
        mesh_path = sibling(path.parent, text(model, model.table(mesh_ref, 1), 0))
        mesh = load(mesh_path, root)
        mesh_root = mesh.table(mesh.target(0), 3)
        buffer_path = sibling(mesh_path.parent, text(mesh, mesh_root, 2))
        buffer = load(buffer_path, root)
        buffer_root = buffer.table(buffer.target(0), 2)
        shapes, buffers = mesh.vector(mesh_root, 1), buffer.vector(buffer_root, 1)
        if len(shapes) != len(buffers):
            raise ValueError('shape/buffer association unresolved')
        for shape_index, (shape_at, buffer_at) in enumerate(zip(shapes, buffers)):
            row = {'model': model_id, 'skeleton': skeleton_id, 'mesh': identity(mesh_path, mesh, root),
                   'buffer': identity(buffer_path, buffer, root), 'shape': shape_index, **skeleton_summary}
            shape = mesh.table(shape_at, 13)
            layouts = mesh.vector(shape, 3)
            if len(layouts) != 1:
                row['status'] = 'unsupported_layout_count'
                rows.append(row)
                continue
            layout = mesh.table(layouts[0], 2)
            sizes = mesh.vector(layout, 1)
            if len(sizes) != 1:
                row['status'] = 'unsupported_vertex_stream_count'
                rows.append(row)
                continue
            stride = scalar(mesh, mesh.table(sizes[0], 1), 0)
            attributes = []
            for attr in mesh.vector(layout, 0):
                table = mesh.table(attr, 5)
                attributes.append({'slot': scalar(mesh, table, 0), 'id': scalar(mesh, table, 1),
                                   'layer': scalar(mesh, table, 2), 'code': scalar(mesh, table, 3),
                                   'offset': scalar(mesh, table, 4)})
            row['stride'], row['attributes'] = stride, attributes
            ids = {a['id']: a for a in attributes}
            if 7 not in ids and 8 not in ids:
                row['status'] = 'no_skin_channels'
                rows.append(row)
                continue
            if len(ids) != len(attributes) or 7 not in ids or 8 not in ids:
                row['status'] = 'unsupported_channel_set'
                rows.append(row)
                continue
            joints, weights = ids[7], ids[8]
            if joints['code'] != 22 or weights['code'] != 39 or any(a['slot'] or a['layer'] for a in attributes):
                row['status'] = 'unsupported_skin_format'
                rows.append(row)
                continue
            bt = buffer.table(buffer_at, 2)
            vertices, indices = buffer.vector(bt, 1), buffer.vector(bt, 0)
            if len(vertices) != 1 or len(indices) != 1 or scalar(mesh, shape, 2) != 1:
                row['status'] = 'unsupported_buffer_or_index_format'
                rows.append(row)
                continue
            raw = payload(buffer, buffer.table(vertices[0], 1), 0)
            index_data = payload(buffer, buffer.table(indices[0], 1), 0)
            if not stride or len(raw) % stride or len(index_data) % 2 or joints['offset'] + 4 > stride or weights['offset'] + 8 > stride:
                raise ValueError('invalid payload/channel ranges')
            count = len(raw) // stride
            if count > 1_000_000 or not index_data or max(struct.unpack('<' + 'H' * (len(index_data) // 2), index_data)) >= count:
                raise ValueError('invalid vertex/index count')
            influence_counts, sums, active, inactive = Counter(), Counter(), set(), set()
            active_zero = invalid_bind = invalid_node = 0
            for i in range(count):
                at = i * stride
                js = raw[at + joints['offset']:at + joints['offset'] + 4]
                ws = struct.unpack_from('<4H', raw, at + weights['offset'])
                influence_counts[sum(w > 0 for w in ws)] += 1
                sums[sum(ws)] += 1
                for joint, weight in zip(js, ws):
                    if weight:
                        active.add(joint)
                        active_zero += joint == 0
                        invalid_bind += joint >= row['binds']
                        invalid_node += joint >= row['nodes']
                    else:
                        inactive.add(joint)
            metadata_rigs = sorted(scalar(mesh, mesh.table(e, 2), 0) for e in mesh.vector(shape, 10))
            widths = {20: 4, 22: 4, 39: 8, 43: 8, 48: 8, 51: 12}
            covered = 0
            intervals_valid = True
            for attribute in sorted(attributes, key=lambda a: a['offset']):
                width = widths.get(attribute['code'])
                if width is None or attribute['offset'] != covered:
                    intervals_valid = False
                    break
                covered += width
            qualified = (not invalid_bind and sorted(active) == metadata_rigs
                         and set(sums).issubset({65534, 65535, 65536})
                         and intervals_valid and covered == stride and 12 <= stride <= 64
                         and len(index_data) % 6 == 0)
            row.update(decoder_supported=qualified, status='observed_rgba8_uint_rgba16_unorm', vertex_count=count,
                       influence_counts=dict(sorted(influence_counts.items())), raw_weight_sums=dict(sorted(sums.items())),
                       active_joint_indices=sorted(active), inactive_joint_indices=sorted(inactive),
                       active_joint_zero_lanes=active_zero, invalid_active_bind_lanes=invalid_bind,
                       invalid_active_node_lanes=invalid_node, mesh_metadata_rig_indices=metadata_rigs,
                       active_indices_equal_mesh_metadata_rigs=sorted(active) == metadata_rigs)
            rows.append(row)
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('romfs', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    root = args.romfs.resolve(strict=True)
    rows, unresolved, models = [], [], 0
    for path in sorted(root.rglob('*.trmdl')):
        models += 1
        try:
            rows.extend(model_rows(path, root))
        except (ValueError, OSError, UnicodeError, struct.error) as error:
            unresolved.append({'model': path.relative_to(root).as_posix(), 'status': 'unresolved', 'reason': str(error) if isinstance(error, ValueError) and '/' not in str(error) and '\\' not in str(error) else type(error).__name__})
    if not models:
        raise ValueError('no model descriptors found')
    counts = dict(sorted(Counter(r['status'] for r in rows).items()))
    report = {'format': 'trm-model-linked-skin-census/v1', 'scope': 'authorized local RomFS; exact inputs identified by hashes',
              'proof_scope': 'structural channel observations only; not runtime/native-function/binary-match proof',
              'model_descriptors': models, 'linked_models': len({r['model']['path'] for r in rows}),
              'mesh_shapes': len(rows), 'status_counts': counts,
              'unique_skin_buffer_hashes': len({r['buffer']['sha256'] for r in rows if r.get('decoder_supported')}),
              'counts_include_repeated_buffers_per_model_reference': True,
              'unresolved_models': unresolved, 'references': rows}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    columns = ['model', 'mesh', 'buffer', 'skeleton', 'shape', 'vertices', 'one', 'two', 'three', 'four', 'active_zero']
    lines = ['\t'.join(columns)]
    for row in rows:
        if not row.get('decoder_supported'):
            continue
        counts = row['influence_counts']
        values = [row[k]['path'] for k in columns[:4]] + [row['shape'], row['vertex_count']] + [counts.get(i, 0) for i in range(1, 5)] + [row['active_joint_zero_lanes']]
        lines.append('\t'.join(map(str, values)))
    args.output.with_suffix('.tsv').write_text('\n'.join(lines) + '\n', encoding='utf-8')
    print(json.dumps({k: v for k, v in report.items() if k not in ['references', 'unresolved_models']}, sort_keys=True))
    print('Unresolved model count:', len(unresolved))


if __name__ == '__main__':
    main()
