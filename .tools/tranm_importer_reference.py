#!/usr/bin/env python3
"""Execute pinned importer record helpers in Blender; game-valued traces PRIVATE.

AST-selected functions/constants and generated *T constructors execute unchanged.
No fake FlatBuffers runtime, pose application, interpolation or bpy scene mutation.
"""
import argparse
import ast
from collections import Counter
import hashlib
import json
import math
from pathlib import Path
import struct
import sys

import bpy
from mathutils import Vector, Quaternion

sys.path.insert(0, str(Path(__file__).resolve().parent))
from tranm_census import Animation

PIN = 'b0c98d9fcaab85a04ad35e2d111bae4cad6c1e04'
IMPORTER_HASH = 'b836fe617f3123a7ff5158a4f5ebab7e44186905ea3b698e19de136a3a1899fd'
CLASSES = ['FixedVectorTrackT', 'DynamicVectorTrackT', 'Framed16VectorTrackT',
           'Framed8VectorTrackT', 'FixedRotationTrackT', 'DynamicRotationTrackT',
           'Framed16RotationTrackT', 'Framed8RotationTrackT', 'Vec3T', 'sVec3T']
FUNCTIONS = ['expand_float', 'unpack_48bit_quaternion', 'get_quaternion_from_packed', 'get_track_transforms']


def reference_namespace(root, expected_hashes):
    hashes = {}
    ns = dict(math=math, Vector=Vector, Quaternion=Quaternion)
    for name in CLASSES:
        path = root / 'GFLib/Anim' / (name.removesuffix('T') + '.py')
        relative = path.relative_to(root).as_posix()
        hashes[relative] = hashlib.sha256(path.read_bytes()).hexdigest()
        if hashes[relative] != expected_hashes[relative]:
            raise ValueError('pinned generated constructor hash mismatch')
        source = ast.parse(path.read_text())
        cls = next(n for n in source.body if isinstance(n, ast.ClassDef) and n.name == name)
        # Only unchanged constructor: deserialization is independently checked elsewhere.
        cls.body = [n for n in cls.body if isinstance(n, ast.FunctionDef) and n.name == '__init__']
        exec(compile(ast.Module(body=[cls], type_ignores=[]), str(path), 'exec'), ns)
    path = root / 'gfbanm_importer.py'
    hashes['gfbanm_importer.py'] = hashlib.sha256(path.read_bytes()).hexdigest()
    if hashes['gfbanm_importer.py'] != IMPORTER_HASH:
        raise ValueError('pinned importer hash mismatch')
    source = ast.parse(path.read_text())
    names = {'TransformType', 'VectorTrackType', 'RotationTrackType', 'SCALE', 'PI_QUARTER', 'PI_HALF'}
    nodes = [n for n in source.body if (isinstance(n, ast.FunctionDef) and n.name in FUNCTIONS)
             or (isinstance(n, ast.Assign) and any(isinstance(t, ast.Name) and t.id in names for t in n.targets))]
    exec(compile(ast.Module(body=nodes, type_ignores=[]), str(path), 'exec'), ns)
    return ns, hashes


def track_object(ns, channel, rotation):
    kind = ['Fixed', 'Dynamic', 'Framed16', 'Framed8'][channel['encoding'] - 1]
    obj = ns[kind + ('Rotation' if rotation else 'Vector') + 'TrackT']()
    values = []
    for v in channel['values']:
        value = ns['sVec3T' if rotation else 'Vec3T']()
        value.x, value.y, value.z = v
        values.append(value)
    obj.co = values[0] if channel['encoding'] == 1 else values
    if channel['encoding'] >= 3:
        obj.frames = channel['frames']
    return obj


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('romfs', type=Path)
    parser.add_argument('--reference-source', type=Path, required=True)
    parser.add_argument('--private-reference', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args(sys.argv[sys.argv.index('--') + 1:])
    repo = Path(__file__).resolve().parent.parent
    root = args.romfs.resolve(strict=True)
    target = args.private_reference.resolve()
    if target.is_relative_to(repo):
        raise ValueError('game-valued reference traces must remain outside repository')
    target.mkdir(parents=True, exist_ok=True)
    census = json.loads((repo / 'reports/skeleton/update-v262144-animation-census.json').read_text())
    ns, hashes = reference_namespace(args.reference_source.resolve(strict=True), census['reference_source_hashes'])
    synthetic = bytearray()
    for values in [(17000, 23000, 12000), (0, 0, 0), (32767, 32767, 32767), (0, 32767, 16384), (16383, 16384, 16385)]:
        for selector in range(4):
            for negative in [False, True]:
                packed = sum(v << s for v, s in zip(values, [3, 18, 33])) | selector | (negative << 2)
                words = [packed & 65535, (packed >> 16) & 65535, (packed >> 32) & 65535]
                q = ns['unpack_48bit_quaternion'](*words)
                synthetic += struct.pack('<3H4f', *words, q.x, q.y, q.z, q.w)
    (target / 'synthetic.bin').write_bytes(synthetic)
    rows, counts = [], Counter()
    for entry in census['references']:
        path = root / entry['path']
        path.resolve(strict=True).relative_to(root)
        if path.stat().st_size > 32 * 1024 * 1024:
            raise ValueError('animation exceeds input cap')
        data = path.read_bytes()
        if hashlib.sha256(data).hexdigest() != entry['sha256']:
            raise ValueError('census input hash mismatch')
        clip = Animation(data).decode()
        oracle_key = hashlib.sha256(entry['path'].encode()).hexdigest()
        trace = bytearray(struct.pack('<II', len(clip['tracks']), clip['frame_count']))
        for track in clip['tracks']:
            channels = track['channels']
            rotation = channels[1]
            trace += struct.pack('<I', len(rotation['values']))
            for words in rotation['values']:
                q = ns['unpack_48bit_quaternion'](*words)
                trace += struct.pack('<4f', q.x, q.y, q.z, q.w)
                packed = words[0] | (words[1] << 16) | (words[2] << 32)
                counts['selector/' + str(packed & 3)] += 1
                counts['negative/' + str(bool(packed & 4)).lower()] += 1
                counts['clamped_records'] += sum(ns['expand_float']((packed >> s) & 32767)**2 for s in [3, 18, 33]) > 1
                counts['rotation_records'] += 1
            evaluated = [ns['get_track_transforms'](track_object(ns, ch, i == 1), clip['frame_count'])
                         for i, ch in enumerate(channels)]
            for i, ch in enumerate(channels):
                label = ['scale', 'rotation', 'translation'][i] + '/' + str(ch['encoding'])
                counts['channels/' + label] += 1
                counts['present/' + label] += sum(v is not None for v in evaluated[i])
                counts['absent/' + label] += sum(v is None for v in evaluated[i])
            for frame in range(clip['frame_count']):
                values = [e[frame] for e in evaluated]
                trace += bytes([sum((v is not None) << i for i, v in enumerate(values))])
                for i, value in enumerate(values):
                    if value is not None:
                        components = (value.x, value.y, value.z, value.w) if i == 1 else tuple(value)
                        trace += struct.pack('<' + str(len(components)) + 'f', *components)
                counts['track_frame_slots'] += 1
            counts['tracks'] += 1
        (target / (oracle_key + '.bin')).write_bytes(trace)
        rows.append(dict(path=entry['path'], sha256=entry['sha256'], oracle_key=oracle_key,
                         trace_sha256=hashlib.sha256(trace).hexdigest(), trace_bytes=len(trace)))
    report = dict(format='tranm-selected-importer-reference/v1', game_build=census['game_build'],
                  archive_sha256=census['archive_sha256'], reference_commit=PIN,
                  reference_version=bpy.app.version_string, reference_build_hash=bpy.app.build_hash.decode(),
                  reference_source_hashes=hashes, executed_functions=FUNCTIONS,
                  script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                  scope='selected unchanged importer helpers and mathutils constructors; not deserializer/full importer/native playback',
                  counts=dict(counts), files=len(rows), references=rows, synthetic_rotation_cases=40,
                  unsupported=['continuous interpolation', 'loop/extrapolation', 'pose application', 'native semantics'])
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    args.output.with_suffix('.tsv').write_text('path\toracle_key\ttrace_sha256\ttrace_bytes\n' + ''.join(
        '\t'.join(str(r[k]) for k in ['path', 'oracle_key', 'trace_sha256', 'trace_bytes']) + '\n' for r in rows))
    print('SELECTED IMPORTER COUNTS', json.dumps(dict(counts), sort_keys=True))


if __name__ == '__main__':
    main()
