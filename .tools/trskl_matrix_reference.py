#!/usr/bin/env python3
"""Blender mathutils reference; metadata report public, matrix traces PRIVATE.

Run with blender --background --factory-startup --disable-autoexec --python
.tools/trskl_matrix_reference.py -- ROMFS --output REPORT [--private-reference DIR].
No bpy operators, rendering, scene mutation or automatic script loading is used.
"""
import argparse
from collections import Counter
import hashlib
import itertools
import json
import math
from pathlib import Path
import struct
import sys

# Blender does not put the script directory on sys.path automatically.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from trskl_census import Skeleton
from mathutils import Euler, Matrix
import bpy


def vec3(reader, table, field):
    values = struct.unpack('<3f', reader.block(reader.field(table, field, 12), 12))
    if not all(math.isfinite(v) for v in values):
        raise ValueError('non-finite packed vector')
    return values


def read(path):
    if path.stat().st_size > 32 * 1024 * 1024:
        raise ValueError('skeleton exceeds supported cap')
    reader = Skeleton(path.read_bytes())
    reader.summary()
    root = reader.table(reader.target(0), 5)
    nodes = []
    for at in reader.vector(root, 1):
        table = reader.table(at, 8)
        transform = reader.table(reader.target(reader.field(table, 1, 4)), 3)
        nodes.append({'scale': vec3(reader, transform, 0), 'rotation': vec3(reader, transform, 1),
                      'translation': vec3(reader, transform, 2), 'parent': reader.index(table, 4),
                      'rig': reader.index(table, 5), 'scale_pivot': vec3(reader, table, 2),
                      'rotate_pivot': vec3(reader, table, 3)})
    binds = []
    for at in reader.vector(root, 2):
        table = reader.table(at, 3)
        flags = [reader.block(reader.field(table, i, 1), 1)[0] for i in range(2)]
        if flags != [1, 1]:
            raise ValueError('unsupported bind flags')
        matrix = reader.table(reader.target(reader.field(table, 2, 4)), 4)
        binds.append([vec3(reader, matrix, i) for i in range(4)])
    return reader, nodes, binds


def difference(left, right):
    return max(abs(left[i][j] - right[i][j]) for i in range(4) for j in range(4))


def identity_residual(left, right):
    return difference(left @ right, Matrix.Identity(4))


def evaluate(nodes, order='XYZ'):
    local, global_matrices = [], []
    for node in nodes:
        if any(node['scale_pivot']) or any(node['rotate_pivot']):
            raise ValueError('nonzero pivots remain unsupported')
        matrix = Matrix.LocRotScale(node['translation'], Euler(node['rotation'], order), node['scale'])
        local.append(matrix)
        global_matrices.append(matrix if node['parent'] < 0 else global_matrices[node['parent']] @ matrix)
    return local, global_matrices


def expand_bind(columns, transpose_basis=False):
    matrix = Matrix.Identity(4)
    for column in range(4):
        for row in range(3):
            matrix[row][column] = columns[column][row]
    if transpose_basis:
        for row in range(3):
            for column in range(3):
                matrix[row][column] = columns[row][column]
    return matrix


def synthetic_nodes():
    # Deterministic author-created transforms, not source-game values.
    values = [([1.2, 0.7, 1.8], [0.23, -0.51, 0.87], [2.0, -3.0, 4.0], -1),
              ([0.8, 1.4, 0.9], [-0.43, 0.67, -0.12], [-1.0, 2.0, 0.5], 0),
              ([1.0, 1.0, 1.0], [0.35, 0.15, -0.75], [0.2, -0.4, 1.5], 1)]
    def f32(v):
        return struct.unpack('<f', struct.pack('<f', v))[0]
    return [dict(scale=tuple(map(f32, s)), rotation=tuple(map(f32, r)),
                 translation=tuple(map(f32, t)), parent=p, rig=-1,
                 scale_pivot=(0, 0, 0), rotate_pivot=(0, 0, 0)) for s, r, t, p in values]


def main():
    argv = sys.argv[sys.argv.index('--') + 1:]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('romfs', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--private-reference', type=Path)
    args = parser.parse_args(argv)
    root = args.romfs.resolve(strict=True)
    repo = Path(__file__).resolve().parent.parent
    if args.private_reference:
        target = args.private_reference.resolve()
        if target.is_relative_to(repo):
            raise ValueError('matrix reference traces must remain outside repository')
        target.mkdir(parents=True, exist_ok=True)
    rows, scores, totals = [], {}, Counter()
    for order in [''.join(p) for p in itertools.permutations('XYZ')]:
        for storage in ['columns', 'transposed_basis']:
            for direction in ['inverse_global', 'global']:
                scores[(order, storage, direction)] = []
    for path in sorted(root.rglob('*.trskl')):
        path.resolve(strict=True).relative_to(root)
        reader, nodes, binds = read(path)
        local, world = evaluate(nodes)
        multi_axis = sum(sum(abs(v) > 1e-5 for v in node['rotation']) >= 2 for node in nodes)
        nonidentity_scale = sum(any(abs(v - 1) > 1e-6 for v in node['scale']) for node in nodes)
        residuals = []
        for i, node in enumerate(nodes):
            if node['rig'] >= 0:
                bind = expand_bind(binds[node['rig']])
                residuals.append({'node': i, 'rig': node['rig'],
                                  'global_times_bind': identity_residual(world[i], bind),
                                  'bind_times_global': identity_residual(bind, world[i]),
                                  'inverse_global_difference': difference(world[i].inverted(), bind)})
        for (order, storage, direction), values in scores.items():
            _, alternative = evaluate(nodes, order)
            for i, node in enumerate(nodes):
                if node['rig'] >= 0:
                    target = alternative[i].inverted() if direction == 'inverse_global' else alternative[i]
                    values.append(difference(target, expand_bind(binds[node['rig']], storage != 'columns')))
        relative = path.relative_to(root).as_posix()
        row = {'path': relative, 'sha256': hashlib.sha256(reader.data).hexdigest(),
               'oracle_key': hashlib.sha256(relative.encode()).hexdigest(),
               'nodes': len(nodes), 'binds': len(binds), 'multi_axis_rotation_nodes': multi_axis,
               'nonidentity_scale_nodes': nonidentity_scale, 'nonzero_pivot_nodes': 0,
               'bind_checks': residuals,
               'bind_rest_status': 'agree' if all(max(b['global_times_bind'], b['bind_times_global']) <= 1e-4 for b in residuals) else 'disagree'}
        rows.append(row)
        totals.update(files=1, nodes=len(nodes), binds=len(binds), multi_axis_rotation_nodes=multi_axis,
                      nonidentity_scale_nodes=nonidentity_scale)
        if args.private_reference:
            # Matrices are game-derived and written ONLY into the opted-in private directory.
            lines = ['node\trig\tlocal_row_major_16\tglobal_row_major_16']
            for i, node in enumerate(nodes):
                values = [i, node['rig']] + [float(v) for matrix in [local[i], world[i]] for line in matrix for v in line]
                lines.append('\t'.join(map(str, values)))
            key = hashlib.sha256(relative.encode()).hexdigest() + '.tsv'
            (args.private_reference / key).write_text('\n'.join(lines) + '\n', encoding='utf-8')
    if not rows:
        raise ValueError('no skeleton references')
    ranked = [{'euler_order': k[0], 'storage': k[1], 'direction': k[2], 'bind_count': len(v),
               'maximum_component_error': max(v), 'agree_at_1e_4': sum(e <= 1e-4 for e in v)}
              for k, v in scores.items()]
    ranked.sort(key=lambda r: r['maximum_component_error'])
    report = {'format': 'trskl-mathutils-reference/v1', 'reference_version': bpy.app.version_string,
              'reference_build_hash': bpy.app.build_hash.decode(),
              'reference_build_date': bpy.app.build_date.decode(),
              'synthetic_reference_nodes': 3,
              'convention': 'column-vector T * Rz * Ry * Rx * S; global=parent*local; serialized XYZW affine columns',
              'scope': 'zero pivots; direct mathutils operation agreement, not full armature importer or native runtime parity',
              'game_build': 'update-v262144',
              'archive_sha256': 'f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446',
              'evaluator_script_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              'scaled_files': sum(r['nonidentity_scale_nodes'] > 0 for r in rows),
              'scaled_files_with_binds': [r['path'] for r in rows if r['nonidentity_scale_nodes'] and r['binds']],
              'nonzero_pivot_nodes': 0,
              'bind_identity_abs_epsilon': 1e-4, 'local_global_reference_abs_epsilon': 2e-5,
              'counts': dict(totals), 'convention_candidates': ranked, 'references': rows}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    header = ['path', 'oracle_key', 'nodes', 'binds', 'bind_rest_status',
              'global_times_bind_max', 'bind_times_global_max']
    lines = ['\t'.join(header)]
    for row in rows:
        checks = row['bind_checks']
        lines.append('\t'.join(map(str, [row['path'], row['oracle_key'], row['nodes'], row['binds'],
                                        row['bind_rest_status'],
                                        max((v['global_times_bind'] for v in checks), default=0),
                                        max((v['bind_times_global'] for v in checks), default=0)])))
    args.output.with_suffix('.tsv').write_text('\n'.join(lines) + '\n', encoding='utf-8')
    if args.private_reference:
        nodes = synthetic_nodes()
        local, world = evaluate(nodes)
        lines = ['node\tparent\tSRT_9\tlocal_row_major_16\tglobal_row_major_16']
        for i, node in enumerate(nodes):
            values = [i, node['parent']] + list(node['scale'] + node['rotation'] + node['translation'])
            values += [float(v) for matrix in [local[i], world[i]] for line in matrix for v in line]
            lines.append('\t'.join(map(str, values)))
        (args.private_reference / 'synthetic.tsv').write_text('\n'.join(lines) + '\n', encoding='utf-8')
    print('REFERENCE COUNTS', json.dumps(dict(totals), sort_keys=True))
    print('BEST CONVENTION', json.dumps(ranked[0], sort_keys=True))
    print('DISAGREEING FILES', [r['path'] for r in rows if r['bind_rest_status'] != 'agree'])


if __name__ == '__main__':
    main()
