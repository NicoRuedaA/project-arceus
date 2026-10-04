//! Synthetic FlatBuffer cases contain no game bytes. Optional corpus input is
//! local-private and is never copied into the repository.
use pla::assets::tr::TrSkl;

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

fn synthetic() -> Vec<u8> {
    let mut bytes = vec![0; 4];
    let root = table(&mut bytes, 20, &[16, 4, 8, 12]);
    offset(&mut bytes, 0, root);
    let nodes = vector(&mut bytes, 2);
    offset(&mut bytes, root + 4, nodes);
    let binds = vector(&mut bytes, 1);
    offset(&mut bytes, root + 8, binds);
    let iks = vector(&mut bytes, 0);
    offset(&mut bytes, root + 12, iks);
    for i in 0..2 {
        let node = table(&mut bytes, 48, &[4, 8, 12, 24, 36, 40, 44]);
        offset(&mut bytes, nodes + 4 + 4 * i, node);
        word(&mut bytes, node + 36, if i == 0 { u32::MAX } else { 0 });
        word(&mut bytes, node + 40, if i == 0 { u32::MAX } else { 0 });
        let transform = table(&mut bytes, 40, &[4, 16, 28]);
        offset(&mut bytes, node + 8, transform);
        for (j, value) in [2.0f32, 3.0, 4.0, 0.1, 0.2, 0.3, 5.0, 6.0, 7.0]
            .iter()
            .enumerate()
        {
            word(&mut bytes, transform + 4 + j * 4, value.to_bits());
        }
        let name = if i == 0 { "root" } else { "child" };
        let text = allocate(&mut bytes, 4 + name.len() + 1);
        word(&mut bytes, text, name.len() as u32);
        bytes[text + 4..text + 4 + name.len()].copy_from_slice(name.as_bytes());
        offset(&mut bytes, node + 4, text);
        let locator = vector(&mut bytes, 0);
        bytes.push(0);
        offset(&mut bytes, node + 44, locator);
    }
    let bind = table(&mut bytes, 4, &[]);
    offset(&mut bytes, binds + 4, bind);
    bytes
}

#[test]
fn trskl_resolves_relative_nodes_and_reads_local_srt() {
    let skeleton = TrSkl::parse(&synthetic()).unwrap();
    assert_eq!(skeleton.bones, ["root", "child"]);
    assert_eq!(skeleton.bind_count, 1);
    assert_eq!(skeleton.nodes[0].parent, None);
    assert_eq!(skeleton.nodes[1].parent, Some(0));
    assert_eq!(skeleton.nodes[1].rig_index, Some(0));
    assert_eq!(skeleton.nodes[1].local.scale, [2.0, 3.0, 4.0]);
    assert_eq!(skeleton.nodes[1].local.rotation, [0.1, 0.2, 0.3]);
    assert_eq!(skeleton.nodes[1].local.translation, [5.0, 6.0, 7.0]);
    assert_eq!(skeleton.nodes[1].scale_pivot, [0.0; 3]);
}

#[test]
fn trskl_rejects_malformed_ranges_names_indices_and_transforms() {
    let source = synthetic();
    let skeleton = TrSkl::parse(&source).unwrap();
    let child = skeleton.offsets[1];
    let transform_field = child + 8;
    let transform = transform_field
        + u32::from_le_bytes(
            source[transform_field..transform_field + 4]
                .try_into()
                .unwrap(),
        ) as usize;
    let root = u32::from_le_bytes(source[..4].try_into().unwrap()) as usize;
    let name_field = child + 4;
    let name = name_field
        + u32::from_le_bytes(source[name_field..name_field + 4].try_into().unwrap()) as usize;
    for (at, value) in [
        (0, u32::MAX),
        (root + 16, 2),  // unseen root flag
        (name_field, 4), // invalid string target
        (child, i32::MIN as u32),
        (child + 36, 1), // self-parent/cycle
        (child + 36, 2), // out of range
        (child + 36, (-2i32) as u32),
        (child + 40, 1), // out-of-range rig
        (child + 8, 0),  // null required transform
        (transform + 4, f32::NAN.to_bits()),
        (name, u32::MAX),
    ] {
        let mut bytes = source.clone();
        word(&mut bytes, at, value);
        assert!(TrSkl::parse(&bytes).is_err(), "mutation at {at}");
    }
    let mut bytes = source.clone();
    bytes[name + 4] = 0xff;
    assert!(TrSkl::parse(&bytes).is_err());
    // Truncations and a bounded byte-mutation sweep must never panic.
    for length in 0..source.len() {
        let _ = TrSkl::parse(&source[..length]);
    }
    for at in 0..source.len() {
        for byte in [0, 0x80, 0xff] {
            let mut bytes = source.clone();
            bytes[at] = byte;
            let _ = TrSkl::parse(&bytes);
        }
    }
}

#[test]
fn trskl_supports_omitted_root_indices_and_zero_bind_skeletons() {
    let mut bytes = synthetic();
    let skeleton = TrSkl::parse(&bytes).unwrap();
    let root_node = skeleton.offsets[0];
    let vt = (root_node as i64
        - i64::from(i32::from_le_bytes(
            bytes[root_node..root_node + 4].try_into().unwrap(),
        ))) as usize;
    // Absent parent/rig fields must NOT accidentally default to node/rig zero.
    bytes[vt + 12..vt + 16].fill(0);
    let child = skeleton.offsets[1];
    word(&mut bytes, child + 40, u32::MAX);
    let root = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
    let bind_field = root + 8;
    let binds = bind_field
        + u32::from_le_bytes(bytes[bind_field..bind_field + 4].try_into().unwrap()) as usize;
    word(&mut bytes, binds, 0);
    word(&mut bytes, root + 16, 1);
    let skeleton = TrSkl::parse(&bytes).unwrap();
    assert_eq!(skeleton.root_flag, 1);
    assert_eq!(skeleton.nodes[0].parent, None);
    assert_eq!(skeleton.nodes[0].rig_index, None);
    assert_eq!(skeleton.bind_count, 0);
    assert_eq!(skeleton.nodes.len(), 2);
}

#[test]
fn trskl_update_corpus_when_explicitly_available() {
    let Ok(root) = std::env::var("PLA_TRSKL_CORPUS") else {
        eprintln!("[skip] set PLA_TRSKL_CORPUS to a local-private update RomFS root");
        return;
    };
    let mut pending = vec![std::path::PathBuf::from(root)];
    let (mut files, mut nodes, mut binds) = (0, 0, 0);
    while let Some(path) = pending.pop() {
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                pending.push(entry.path());
            } else if entry.path().extension().is_some_and(|ext| ext == "trskl") {
                let bytes = std::fs::read(entry.path()).unwrap();
                let skeleton = TrSkl::parse(&bytes)
                    .unwrap_or_else(|e| panic!("{}: {e}", entry.path().display()));
                if entry.file_name() == "item_228.trskl" {
                    assert_eq!((skeleton.nodes.len(), skeleton.bind_count), (16, 12));
                    assert_eq!(skeleton.nodes[4].name, "parts_01");
                    assert_eq!(skeleton.nodes[4].parent, Some(3));
                    assert_eq!(skeleton.nodes[4].rig_index, Some(1));
                }
                if entry.file_name() == "item_001.trskl" {
                    assert_eq!((skeleton.nodes.len(), skeleton.bind_count), (3, 0));
                }
                files += 1;
                nodes += skeleton.nodes.len();
                binds += skeleton.bind_count;
            }
        }
    }
    assert_eq!((files, nodes, binds), (199, 1264, 289));
}
