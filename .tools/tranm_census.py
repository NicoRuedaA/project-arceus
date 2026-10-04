#!/usr/bin/env python3
"""Bounded metadata-only animation census; payload/key values remain private.

Independent Python traversal corroborated by pinned generated animation readers.
Reports are not native interpolation, quaternion or playback parity evidence.
"""
import argparse
from collections import Counter
import hashlib
import json
import math
from pathlib import Path
import struct

from trskl_census import Skeleton

MAX_KEYS = 1_000_000
NAMES = ['scale', 'rotation', 'translation']
ENCODINGS = {1: 'fixed', 2: 'dense', 3: 'framed16', 4: 'framed8'}


class Animation(Skeleton):
    def vector_data(self, table, index, width):
        at = self.target(self.field(table, index, 4))
        if at % 4:
            raise ValueError("unaligned animation vector")
        count = self.u32(at)
        if not count or count > MAX_KEYS:
            raise ValueError('unsupported animation vector count')
        return count, self.block(at + 4, count * width)

    def text(self, table, index):
        at = self.target(self.field(table, index, 4))
        size = self.u32(at)
        if not 0 < size <= 1024:
            raise ValueError('unsupported track name size')
        text = self.block(at + 4, size + 1)
        if text[-1] or 0 in text[:-1]:
            raise ValueError('invalid track name termination')
        return text[:-1].decode('utf-8', errors='strict')

    def channel(self, track, index, count, rotation):
        at = self.field(track, index, 1, required=False)
        tag = self.block(at, 1)[0] if at else 0
        if tag not in ENCODINGS:
            raise ValueError('unsupported animation channel encoding')
        width = 6 if rotation else 12
        table = self.table(self.target(self.field(track, index + 1, 4)), 1 if tag < 3 else 2)
        if tag == 1:
            # Packed rotation structs are two-byte, NOT four-byte aligned.
            at = self.field(table, 0, 2 if rotation else 12)
            if at + width > table[0] + table[1] or at % (2 if rotation else 4):
                raise ValueError('invalid fixed channel extent/alignment')
            raw = self.block(at, width)
            frames = [0]
        else:
            keys, raw = self.vector_data(table, 0 if tag == 2 else 1, width)
            if tag == 2:
                if keys != count:
                    raise ValueError('dense channel length differs from frame count')
                frames = list(range(keys))
            else:
                keys2, indices = self.vector_data(table, 0, 2 if tag == 3 else 1)
                if keys2 != keys:
                    raise ValueError('framed channel length mismatch')
                frames = list(struct.unpack('<' + ('H' if tag == 3 else 'B') * keys, indices))
        if frames[-1] >= count or any(a > b for a, b in zip(frames, frames[1:])):
            raise ValueError('unordered or out-of-range animation frames')
        values = list(struct.iter_unpack('<3H' if rotation else '<3f', raw))
        if not rotation and not all(math.isfinite(v) for value in values for v in value):
            raise ValueError('non-finite animation vector')
        return {'encoding': tag, 'frames': frames, 'values': values, 'raw': raw}

    def decode(self):
        root = self.table(self.target(0), 5)
        if any(self.field(root, i, 4, False) is not None for i in range(2, 5)):
            raise ValueError('unsupported material/visibility/event animation chunk')
        info = self.table(self.target(self.field(root, 0, 4)), 3)
        at = self.field(info, 0, 4, False)
        loop = self.u32(at) if at else 0
        count = self.u32(self.field(info, 1, 4))
        rate = self.u32(self.field(info, 2, 4))
        if loop > 1 or not 0 < count <= MAX_KEYS or not 0 < rate <= 1000:
            raise ValueError('unsupported animation timing')
        skeleton = self.table(self.target(self.field(root, 1, 4)), 2)
        if self.field(skeleton, 1, 4, False) is not None:
            raise ValueError('unsupported animation InitData')
        tracks = []
        names = set()
        keys = 0
        for at in self.vector(skeleton, 0):
            track = self.table(at, 7)
            name = self.text(track, 0)
            if name in names:
                raise ValueError('duplicate animation track name')
            names.add(name)
            channels = [self.channel(track, i, count, j == 1) for j, i in enumerate([1, 3, 5])]
            keys += sum(len(ch['frames']) for ch in channels)
            if keys > MAX_KEYS:
                raise ValueError('animation aggregate key cap exceeded')
            tracks.append({'name': name, 'channels': channels})
        if not tracks:
            raise ValueError('empty skeletal animation')
        return {'loops': loop, 'frame_count': count, 'frame_rate': rate, 'tracks': tracks}


def canonical_fingerprint(clip):
    # Non-cryptographic metadata checksum over typed records, not playback proof.
    value = 0xcbf29ce484222325
    def add(data):
        nonlocal value
        for byte in data:
            value = ((value ^ byte) * 0x100000001b3) & 0xffffffffffffffff
    add(struct.pack('<3I', clip['loops'], clip['frame_count'], clip['frame_rate']))
    add(struct.pack('<I', len(clip['tracks'])))
    for track in clip['tracks']:
        name = track['name'].encode()
        add(struct.pack('<I', len(name)) + name)
        for j, channel in enumerate(track['channels']):
            add(struct.pack('<BI', channel['encoding'], len(channel['frames'])))
            width = 6 if j == 1 else 12
            for i, frame in enumerate(channel['frames']):
                add(struct.pack('<I', frame) + channel['raw'][i * width:(i + 1) * width])
    return f'{value:016x}'


def skeleton_names(root):
    result = []
    for path in sorted(root.rglob('*.trskl')):
        path.resolve(strict=True).relative_to(root)
        reader = Animation(path.read_bytes())
        Skeleton.summary(reader)
        table = reader.table(reader.target(0), 5)
        names = [reader.text(reader.table(at, 8), 0) for at in reader.vector(table, 1)]
        result.append({'path': path.relative_to(root).as_posix(),
                       'sha256': hashlib.sha256(reader.data).hexdigest(), 'names': names})
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('romfs', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    root = args.romfs.resolve(strict=True)
    skeletons = skeleton_names(root)
    rows, totals, errors = [], Counter(), Counter()
    for path in sorted(root.rglob('*.tranm')):
        path.resolve(strict=True).relative_to(root)
        if path.stat().st_size > 32 * 1024 * 1024:
            raise ValueError('animation exceeds input cap')
        data = path.read_bytes()
        row = {'path': path.relative_to(root).as_posix(), 'sha256': hashlib.sha256(data).hexdigest(), 'bytes': len(data)}
        try:
            clip = Animation(data).decode()
            counts, keys, varying = Counter(), Counter(), Counter()
            duplicate_channels = 0
            for track in clip['tracks']:
                for name, channel in zip(NAMES, track['channels']):
                    encoding = f'{name}/{ENCODINGS[channel["encoding"]]}'
                    counts[encoding] += 1
                    keys[encoding] += len(channel['frames'])
                    varying[encoding] += len(set(channel['values'])) > 1
                    duplicate_channels += any(a == b for a, b in zip(channel['frames'], channel['frames'][1:]))
            names = {track['name'] for track in clip['tracks']}
            candidates = [sk for sk in skeletons if names.issubset(sk['names'])]
            mapping = {'candidate_count': len(candidates),
                       'status': 'unique_within_census' if len(candidates) == 1 else 'ambiguous' if candidates else 'unresolved'}
            if len(candidates) == 1:
                sk = candidates[0]
                mapping.update(skeleton_path=sk['path'], skeleton_sha256=sk['sha256'], matched_tracks=len(names))
            row.update(status='records_decoded', loops=clip['loops'], frame_count=clip['frame_count'], frame_rate=clip['frame_rate'],
                       tracks=len(clip['tracks']), encodings=dict(counts), keys=dict(keys), varying_channels=dict(varying),
                       duplicate_frame_channels=duplicate_channels, record_fingerprint=canonical_fingerprint(clip), name_mapping=mapping)
            totals.update(files_with_varying_records=int(sum(varying.values()) > 0))
            for track in clip['tracks']:
                for channel in track['channels']:
                    if channel['encoding'] >= 3:
                        frames, values = channel['frames'], channel['values']
                        same_boundary = (len(frames) >= 4 and frames[0] == frames[1] == 0 and
                                         frames[-1] == frames[-2] == clip['frame_count'] - 1 and
                                         values[0] == values[1] and values[-1] == values[-2] and
                                         sum(a == b for a, b in zip(frames, frames[1:])) == 2)
                        totals.update(identical_duplicate_boundary_channels=int(same_boundary))
            totals.update(record_decoded_files=1, tracks=len(clip['tracks']), key_records=sum(keys.values()), duplicate_frame_channels=duplicate_channels)
            totals.update({f'encoding/{k}': v for k, v in counts.items()})
            totals.update({f'keys/{k}': v for k, v in keys.items()})
            totals.update({f'varying/{k}': v for k, v in varying.items()})
            totals.update({f'mapping/{mapping["status"]}': 1})
        except (ValueError, UnicodeError, struct.error) as error:
            row.update(status='unsupported', reason=str(error))
            errors[str(error)] += 1
        rows.append(row)
    if not rows:
        raise ValueError('no animation inputs found')
    report = {'format': 'tranm-typed-record-census/v1', 'game_build': 'update-v262144',
              'archive_sha256': 'f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446',
              'scope': 'standalone authorized corpus; typed records only, no interpolation/quaternion/native playback proof',
              'reference_commit': 'b0c98d9fcaab85a04ad35e2d111bae4cad6c1e04',
              'reference_source_hashes': {'GFLib/Anim/Animation.py': '324ea935e9dbef290eb217acc12c37d1f34ff322c75ab4b36aa7d9a7be94a575', 'GFLib/Anim/Info.py': '6429e64f365b6ec1a0e32d93fe55fde522742bf6842017285b947feba6d6608e', 'GFLib/Anim/BoneAnimation.py': 'be59f2e76a43f107aa9412883ad7c53c05c33e72a2818f423de7b87b4d123656', 'GFLib/Anim/BoneTrack.py': '9ee0c0a91ed235a74e2d7b512a5af6850e7f50556bfe3d9a7a5c28399fb23e1a', 'GFLib/Anim/VectorTrack.py': '6fb9a2f37632c7d7dc6a99510b86e084ecbad42b0a799948018bfd8b53c09f48', 'GFLib/Anim/RotationTrack.py': '20a75d388fc934d75bf461aa528a19c5255c46bb5ebe7865e81d8fbda6f3908f', 'GFLib/Anim/FixedVectorTrack.py': 'a54c9577e43311bd5ae9491418532ddf85cfab90b737d1229dd722d046a54199', 'GFLib/Anim/DynamicVectorTrack.py': '5b3b033b6a0663d0bb23ab88b7d184b91cfe90d6d67b3ee098d5e043e181756a', 'GFLib/Anim/Framed16VectorTrack.py': 'aec32c5a1504cf9d3f4a5c2ab3233145cd954a904a1d3c4dbd7573dcb51f0b78', 'GFLib/Anim/Framed8VectorTrack.py': 'f2117f3110df3254232fbd19992ea12b0a80f0f161e9fda230cdc2ea4664a764', 'GFLib/Anim/FixedRotationTrack.py': 'e1d83c57e546be919c9a3831fe374cd5ac1e16c819f56693c99eb1e3fffcb974', 'GFLib/Anim/DynamicRotationTrack.py': 'beab641dd2bc22f82b5c1b3a4fcc08b933beac0c0bbc58115011bb6ab78d8bf8', 'GFLib/Anim/Framed16RotationTrack.py': '72a1648604f2099e62321452d27bba6614714c613e4b270377313997a2ee89cc', 'GFLib/Anim/Framed8RotationTrack.py': '5d357cfd70320f8405e07791ae0688d72d6d29c948fddf483c7d046c16d3d7e0', 'GFLib/Anim/Vec3.py': '35443c6d6196db282ff79c4d4604761204a15855fad1ada38918274a0cd2dbf2', 'GFLib/Anim/sVec3.py': '413543ba20953c237d516a9b82a98042b4a3003db4e5ff90f81e48b7ba6ffb96', 'gfbanm_importer.py': 'b836fe617f3123a7ff5158a4f5ebab7e44186905ea3b698e19de136a3a1899fd', 'gfbanm_exporter.py': 'f70892f2158ac8d6a46380dd15179ce5396fbb17ed6710305b834c4436ceefa2'},
              'census_script_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              'files': len(rows), 'skeleton_references': len(skeletons), 'counts': dict(totals),
              'unsupported_semantics': ['packed quaternion reconstruction', 'interpolation and duplicate-record evaluation',
                                        'loop boundary evaluation', 'native skeleton association', 'playback/deformation'],
              'name_mapping_scope': 'unique name-subset candidate within 199-file skeleton census, not native asset association',
              'timing': {'frame_count_min': min((r['frame_count'] for r in rows if r['status'] == 'records_decoded'), default=None),
                         'frame_count_max': max((r['frame_count'] for r in rows if r['status'] == 'records_decoded'), default=None),
                         'frame_rates': sorted({r['frame_rate'] for r in rows if r['status'] == 'records_decoded'}),
                         'loop_counts': dict(Counter(r['loops'] for r in rows if r['status'] == 'records_decoded'))},
              'unsupported_reasons': dict(errors), 'references': rows}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    columns = ['path', 'status', 'frame_count', 'frame_rate', 'loops', 'tracks', 'key_records', 'duplicate_frame_channels', 'record_fingerprint', 'skeleton_path', 'skeleton_sha256']
    lines = ['\t'.join(columns)]
    for row in rows:
        mapping = row.get('name_mapping', {})
        values = [row['path'], row['status'], row.get('frame_count', ''), row.get('frame_rate', ''), row.get('loops', ''),
                  row.get('tracks', ''), sum(row.get('keys', {}).values()), row.get('duplicate_frame_channels', ''), row.get('record_fingerprint', ''),
                  mapping.get('skeleton_path', ''), mapping.get('skeleton_sha256', '')]
        lines.append('\t'.join(map(str, values)))
    args.output.with_suffix('.tsv').write_text('\n'.join(lines) + '\n', encoding='utf-8')
    print('ANIMATION COUNTS', json.dumps(dict(totals), sort_keys=True))
    print('UNSUPPORTED', dict(errors))


if __name__ == '__main__':
    main()
