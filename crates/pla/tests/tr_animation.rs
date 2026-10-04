//! Author-created FlatBuffers only; corpus inputs are explicitly opted-in/private.
use pla::assets::tr::{TrAnm, TrAnmClip, TrAnmEncoding, TrSkl, TrSklNode, TrSklTransform};
fn word(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn offset(bytes: &mut [u8], at: usize, target: usize) {
    word(bytes, at, u32::try_from(target - at).unwrap());
}

fn allocate(bytes: &mut Vec<u8>, size: usize) -> usize {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(0);
    }
    let at = bytes.len();
    bytes.resize(at + size, 0);
    at
}

fn table(bytes: &mut Vec<u8>, size: usize, fields: &[u16]) -> usize {
    let at = allocate(bytes, size);
    // Deliberately place the vtable AFTER the table, exercising signed offsets.
    let vt = allocate(bytes, 4 + fields.len() * 2);
    word(bytes, at, (at as i32 - vt as i32) as u32);
    bytes[vt..vt + 2].copy_from_slice(&((4 + fields.len() * 2) as u16).to_le_bytes());
    bytes[vt + 2..vt + 4].copy_from_slice(&(size as u16).to_le_bytes());
    for (i, field) in fields.iter().enumerate() {
        bytes[vt + 4 + i * 2..vt + 6 + i * 2].copy_from_slice(&field.to_le_bytes());
    }
    at
}

fn vector(bytes: &mut Vec<u8>, count: usize) -> usize {
    let at = allocate(bytes, 4 + count * 4);
    word(bytes, at, count as u32);
    at
}

fn text(bytes: &mut Vec<u8>, name: &str) -> usize {
    let at = allocate(bytes, 4 + name.len() + 1);
    word(bytes, at, name.len() as u32);
    bytes[at + 4..at + 4 + name.len()].copy_from_slice(name.as_bytes());
    at
}

struct Fixture {
    bytes: Vec<u8>,
    root: usize,
    info: usize,
    skeleton: usize,
    track: usize,
    values: usize,
    frames: Option<usize>,
}

fn fixture(tag: u8, count: u32) -> Fixture {
    let mut bytes = vec![0; 4];
    let root = table(&mut bytes, 24, &[4, 8, 12, 16, 20]);
    offset(&mut bytes, 0, root);
    let info = table(&mut bytes, 16, &[4, 8, 12]);
    offset(&mut bytes, root + 4, info);
    word(&mut bytes, info + 4, 1);
    word(&mut bytes, info + 8, count);
    word(&mut bytes, info + 12, 60);
    let skeleton = table(&mut bytes, 12, &[4, 8]);
    offset(&mut bytes, root + 8, skeleton);
    // Absent fields, rather than present null uoffsets, denote missing chunks.
    let vt = (root as i64
        - i64::from(i32::from_le_bytes(
            bytes[root..root + 4].try_into().unwrap(),
        ))) as usize;
    bytes[vt + 8..vt + 14].fill(0);
    let vt = (skeleton as i64
        - i64::from(i32::from_le_bytes(
            bytes[skeleton..skeleton + 4].try_into().unwrap(),
        ))) as usize;
    bytes[vt + 6..vt + 8].fill(0);
    let tracks = vector(&mut bytes, 1);
    offset(&mut bytes, skeleton + 4, tracks);
    let track = table(&mut bytes, 32, &[4, 8, 12, 16, 20, 24, 28]);
    offset(&mut bytes, tracks + 4, track);
    let name = text(&mut bytes, "synthetic_root");
    offset(&mut bytes, track + 4, name);
    let mut values = 0;
    let mut frames = None;
    for (channel, type_field) in [8, 16, 24].iter().copied().enumerate() {
        bytes[track + type_field] = tag;
        let rotation = channel == 1;
        let width = if rotation { 6 } else { 12 };
        let tab = table(
            &mut bytes,
            if tag == 1 && rotation {
                12
            } else if tag < 3 {
                16
            } else {
                12
            },
            if tag == 1 && rotation {
                &[6]
            } else if tag < 3 {
                &[4]
            } else {
                &[4, 8]
            },
        );
        offset(&mut bytes, track + type_field + 4, tab);
        let keys = if tag == 1 {
            1
        } else if tag == 2 {
            count as usize
        } else {
            4
        };
        let at = if tag == 1 {
            tab + if rotation { 6 } else { 4 }
        } else {
            let at = allocate(&mut bytes, 4 + keys * width);
            word(&mut bytes, at, keys as u32);
            offset(&mut bytes, tab + if tag == 2 { 4 } else { 8 }, at);
            at + 4
        };
        for key in 0..keys {
            if rotation {
                for (j, value) in [0x1000u16, 0x2345, 0x6789].iter().enumerate() {
                    bytes[at + key * width + 2 * j..at + key * width + 2 * j + 2]
                        .copy_from_slice(&value.to_le_bytes());
                }
            } else {
                for j in 0..3 {
                    word(
                        &mut bytes,
                        at + key * width + j * 4,
                        (if tag >= 3 {
                            if key < 2 {
                                0.0f32
                            } else {
                                2.0
                            }
                        } else {
                            key as f32
                        })
                        .to_bits(),
                    );
                }
            }
        }
        if channel == 2 {
            values = at;
        }
        if tag >= 3 {
            let size = if tag == 3 { 2 } else { 1 };
            let f = allocate(&mut bytes, 4 + keys * size);
            word(&mut bytes, f, keys as u32);
            offset(&mut bytes, tab + 4, f);
            for (j, value) in [0u16, 0, (count - 1) as u16, (count - 1) as u16]
                .iter()
                .enumerate()
            {
                if size == 2 {
                    bytes[f + 4 + j * 2..f + 6 + j * 2].copy_from_slice(&value.to_le_bytes());
                } else {
                    bytes[f + 4 + j] = *value as u8;
                }
            }
            if channel == 2 {
                frames = Some(f + 4);
            }
        }
    }
    Fixture {
        bytes,
        root,
        info,
        skeleton,
        track,
        values,
        frames,
    }
}

#[test]
fn animation_decodes_fixed_dense_and_both_framed_encodings_losslessly() {
    for (tag, encoding) in [
        (1, TrAnmEncoding::Fixed),
        (2, TrAnmEncoding::Dense),
        (3, TrAnmEncoding::Framed16),
        (4, TrAnmEncoding::Framed8),
    ] {
        let source = fixture(tag, 3);
        let clip = TrAnm::parse_tracks(&source.bytes).unwrap();
        assert!(clip.loops);
        assert_eq!(
            (clip.frame_count, clip.frame_rate, clip.tracks.len()),
            (3, 60, 1)
        );
        let track = &clip.tracks[0];
        assert_eq!(track.name, "synthetic_root");
        assert_eq!(track.translation.encoding, encoding);
        assert_eq!(track.rotation.keys[0].value, [0x1000, 0x2345, 0x6789]);
        assert_eq!(track.translation.keys[0].value, [0.0; 3]); // zero records are valid
        let frames: Vec<_> = track.translation.keys.iter().map(|key| key.frame).collect();
        assert_eq!(
            frames,
            if tag == 1 {
                vec![0]
            } else if tag == 2 {
                vec![0, 1, 2]
            } else {
                vec![0, 0, 2, 2]
            }
        );
        if tag >= 3 {
            assert_eq!(track.translation.keys[0], track.translation.keys[1]);
            assert_eq!(track.translation.keys[2], track.translation.keys[3]);
        }
    }
    let source = fixture(3, 301);
    let clip = TrAnm::parse_tracks(&source.bytes).unwrap();
    assert_eq!(clip.tracks[0].translation.keys[2].frame, 300);
}

#[test]
fn animation_rejects_bad_headers_tags_lengths_indices_and_payloads() {
    let source = fixture(4, 3);
    for (at, value) in [
        (0, u32::MAX),
        (source.info + 4, 2),
        (source.info + 8, 0),
        (source.info + 12, 0),
        (source.info + 12, 1001),
        (source.track + 12, 0),
        (source.values, f32::NAN.to_bits()),
    ] {
        let mut bytes = source.bytes.clone();
        word(&mut bytes, at, value);
        assert!(TrAnm::parse_tracks(&bytes).is_err(), "mutation at {at}");
    }
    for tag in [0, 5, 255] {
        let mut bytes = source.bytes.clone();
        bytes[source.track + 8] = tag;
        assert!(TrAnm::parse_tracks(&bytes).is_err());
    }
    let mut bytes = source.bytes.clone();
    bytes[source.frames.unwrap() + 2] = 3;
    assert!(TrAnm::parse_tracks(&bytes).is_err());
    let mut bytes = source.bytes.clone();
    bytes[source.frames.unwrap() + 1] = 2;
    bytes[source.frames.unwrap() + 2] = 1;
    assert!(TrAnm::parse_tracks(&bytes).is_err());
    let mut bytes = source.bytes.clone();
    word(&mut bytes, source.values - 4, 3);
    assert!(TrAnm::parse_tracks(&bytes).is_err());
    let source = fixture(2, 3);
    let mut bytes = source.bytes.clone();
    word(&mut bytes, source.values - 4, 2);
    assert!(TrAnm::parse_tracks(&bytes).is_err());
    for length in 0..source.bytes.len() {
        let _ = TrAnm::parse_tracks(&source.bytes[..length]);
    }
    for at in 0..source.bytes.len() {
        let mut bytes = source.bytes.clone();
        bytes[at] ^= 0xff;
        let _ = TrAnm::parse_tracks(&bytes);
    }
}

#[test]
fn animation_rejects_extra_chunks_init_data_name_errors_and_budget_exhaustion() {
    let source = fixture(1, 3);
    for (table_at, field) in [
        (source.root, 2),
        (source.root, 3),
        (source.root, 4),
        (source.skeleton, 1),
    ] {
        let mut bytes = source.bytes.clone();
        let vt = (table_at as i64
            - i64::from(i32::from_le_bytes(
                bytes[table_at..table_at + 4].try_into().unwrap(),
            ))) as usize;
        bytes[vt + 4 + field * 2..vt + 6 + field * 2]
            .copy_from_slice(&((4 + field * 4) as u16).to_le_bytes());
        assert!(TrAnm::parse_tracks(&bytes).is_err());
    }
    let mut bytes = source.bytes.clone();
    let at = source.track + 4;
    let name = at + u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
    bytes[name + 4] = 0xff;
    assert!(TrAnm::parse_tracks(&bytes).is_err());
    let mut bytes = source.bytes.clone();
    let duplicate = vector(&mut bytes, 2);
    offset(&mut bytes, source.skeleton + 4, duplicate);
    // New vector elements can use forward copies of the table, retaining names.
    let first = table(&mut bytes, 32, &[4, 8, 12, 16, 20, 24, 28]);
    let second = table(&mut bytes, 32, &[4, 8, 12, 16, 20, 24, 28]);
    offset(&mut bytes, duplicate + 4, first);
    offset(&mut bytes, duplicate + 8, second);
    let name = text(&mut bytes, "duplicate");
    offset(&mut bytes, first + 4, name);
    offset(&mut bytes, second + 4, name);
    for tr in [first, second] {
        for id in [8, 16, 24] {
            bytes[tr + id] = 1;
            let tab = table(&mut bytes, 16, &[4]);
            offset(&mut bytes, tr + id + 4, tab);
        }
    }
    assert!(TrAnm::parse_tracks(&bytes)
        .unwrap_err()
        .contains("duplicate"));
    let huge = fixture(2, 400_001);
    assert!(TrAnm::parse_tracks(&huge.bytes)
        .unwrap_err()
        .contains("aggregate key cap"));
    assert!(TrAnm::parse_tracks(&vec![0; 32 * 1024 * 1024 + 1]).is_err());
}

fn sk_node(name: &str) -> TrSklNode {
    TrSklNode {
        name: name.into(),
        local: TrSklTransform {
            scale: [1.0; 3],
            rotation: [0.0; 3],
            translation: [0.0; 3],
        },
        scale_pivot: [0.0; 3],
        rotate_pivot: [0.0; 3],
        parent: None,
        rig_index: None,
    }
}

#[test]
fn animation_name_mapping_requires_explicit_unique_skeleton_names() {
    let clip = TrAnm::parse_tracks(&fixture(1, 3).bytes).unwrap();
    let mut sk = TrSkl {
        bones: vec![],
        offsets: vec![],
        nodes: vec![sk_node("other"), sk_node("synthetic_root")],
        bind_count: 0,
        root_flag: 0,
    };
    assert_eq!(clip.skeleton_nodes(&sk).unwrap(), [1]);
    sk.nodes.push(sk_node("synthetic_root"));
    assert!(clip.skeleton_nodes(&sk).is_err());
    sk.nodes = vec![sk_node("unrelated")];
    assert!(clip.skeleton_nodes(&sk).is_err());
}

fn fingerprint(clip: &TrAnmClip) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    let mut add = |bytes: &[u8]| {
        for &b in bytes {
            hash = (hash ^ u64::from(b)).wrapping_mul(0x100000001b3);
        }
    };
    for v in [
        u32::from(clip.loops),
        clip.frame_count,
        clip.frame_rate,
        clip.tracks.len() as u32,
    ] {
        add(&v.to_le_bytes());
    }
    for track in &clip.tracks {
        add(&(track.name.len() as u32).to_le_bytes());
        add(track.name.as_bytes());
        add(&[track.scale.encoding as u8]);
        add(&(track.scale.keys.len() as u32).to_le_bytes());
        for key in &track.scale.keys {
            add(&key.frame.to_le_bytes());
            for value in key.value {
                add(&value.to_bits().to_le_bytes());
            }
        }
        add(&[track.rotation.encoding as u8]);
        add(&(track.rotation.keys.len() as u32).to_le_bytes());
        for key in &track.rotation.keys {
            add(&key.frame.to_le_bytes());
            for value in key.value {
                add(&value.to_le_bytes());
            }
        }
        add(&[track.translation.encoding as u8]);
        add(&(track.translation.keys.len() as u32).to_le_bytes());
        for key in &track.translation.keys {
            add(&key.frame.to_le_bytes());
            for value in key.value {
                add(&value.to_bits().to_le_bytes());
            }
        }
    }
    format!("{hash:016x}")
}

#[test]
fn animation_entire_listed_corpus_matches_independent_typed_record_census() {
    let Ok(root) = std::env::var("PLA_ANIMATION_CORPUS") else {
        eprintln!("[skip] private animation corpus not configured");
        return;
    };
    let metadata = include_str!("../../../reports/skeleton/update-v262144-animation-census.tsv");
    let (mut files, mut tracks, mut keys, mut duplicates, mut mapped) = (0, 0, 0, 0, 0);
    let mut encodings = [0usize; 12];
    for line in metadata.lines().skip(1) {
        let f: Vec<_> = line.split('\t').collect();
        assert_eq!(f.len(), 11);
        assert_eq!(f[1], "records_decoded");
        let source = std::fs::read(std::path::Path::new(&root).join(f[0])).unwrap();
        let clip = TrAnm::parse_tracks(&source).unwrap_or_else(|e| panic!("{}: {e}", f[0]));
        assert_eq!(clip.frame_count, f[2].parse::<u32>().unwrap());
        assert_eq!(clip.frame_rate, f[3].parse::<u32>().unwrap());
        assert_eq!(u32::from(clip.loops), f[4].parse::<u32>().unwrap());
        assert_eq!(clip.tracks.len(), f[5].parse::<usize>().unwrap());
        assert_eq!(fingerprint(&clip), f[8]);
        let mut file_keys = 0;
        let mut file_duplicates = 0;
        for track in &clip.tracks {
            let sizes = [
                track.scale.keys.len(),
                track.rotation.keys.len(),
                track.translation.keys.len(),
            ];
            let tags = [
                track.scale.encoding,
                track.rotation.encoding,
                track.translation.encoding,
            ];
            for (i, tag) in tags.iter().enumerate() {
                encodings[4 * i + *tag as usize - 1] += 1;
                file_keys += sizes[i];
            }
            file_duplicates += usize::from(
                track
                    .scale
                    .keys
                    .windows(2)
                    .any(|k| k[0].frame == k[1].frame),
            ) + usize::from(
                track
                    .rotation
                    .keys
                    .windows(2)
                    .any(|k| k[0].frame == k[1].frame),
            ) + usize::from(
                track
                    .translation
                    .keys
                    .windows(2)
                    .any(|k| k[0].frame == k[1].frame),
            );
        }
        assert_eq!(file_keys, f[6].parse::<usize>().unwrap());
        assert_eq!(file_duplicates, f[7].parse::<usize>().unwrap());
        if !f[9].is_empty() {
            let bytes = std::fs::read(std::path::Path::new(&root).join(f[9])).unwrap();
            let sk = TrSkl::parse(&bytes).unwrap();
            let mapping = clip.skeleton_nodes(&sk).unwrap();
            assert_eq!(mapping.len(), clip.tracks.len());
            for (track, &i) in clip.tracks.iter().zip(&mapping) {
                assert_eq!(track.name, sk.nodes[i].name);
            }
            mapped += 1;
        }
        files += 1;
        tracks += clip.tracks.len();
        keys += file_keys;
        duplicates += file_duplicates;
    }
    assert_eq!(
        (files, tracks, keys, duplicates, mapped),
        (1522, 25394, 1868293, 21677, 6)
    );
    assert_eq!(
        encodings,
        [25195, 0, 157, 42, 10897, 1200, 6674, 6623, 16848, 365, 3756, 4425]
    );
    eprintln!("[animation records] files={files} tracks={tracks} records={keys} duplicate_channels={duplicates} unique_census_name_mappings={mapped}");
}
