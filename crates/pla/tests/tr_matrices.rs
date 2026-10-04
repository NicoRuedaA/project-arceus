//! Author-created buffers and transforms; opted-in game/reference inputs stay private.
use pla::assets::tr::{TrSkl, TrSklBindRecord, TrSklMatrix, TrSklNode, TrSklTransform};
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

fn synthetic_buffer() -> Vec<u8> {
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
        for (j, value) in if i == 0 {
            [1.0f32, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
        } else {
            [1.0f32, 1.0, 1.0, 0.0, 0.0, 0.0, 2.0, 3.0, 4.0]
        }
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
    let bind = table(&mut bytes, 12, &[4, 5, 8]);
    offset(&mut bytes, binds + 4, bind);
    bytes[bind + 4] = 1;
    bytes[bind + 5] = 1;
    let matrix = table(&mut bytes, 52, &[4, 16, 28, 40]);
    offset(&mut bytes, bind + 8, matrix);
    for (j, value) in [
        1.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, -2.0, -3.0, -4.0,
    ]
    .iter()
    .enumerate()
    {
        word(&mut bytes, matrix + 4 + j * 4, value.to_bits());
    }
    bytes
}

fn node(local: TrSklTransform, parent: Option<usize>, rig_index: Option<usize>) -> TrSklNode {
    TrSklNode {
        name: "synthetic".into(),
        local,
        parent,
        rig_index,
        scale_pivot: [0.0; 3],
        rotate_pivot: [0.0; 3],
    }
}

fn skeleton(nodes: Vec<TrSklNode>, bind_count: usize) -> TrSkl {
    TrSkl {
        bones: nodes.iter().map(|n| n.name.clone()).collect(),
        offsets: vec![],
        nodes,
        bind_count,
        root_flag: 0,
    }
}

fn identity_srt() -> TrSklTransform {
    TrSklTransform {
        scale: [1.0; 3],
        rotation: [0.0; 3],
        translation: [0.0; 3],
    }
}

#[test]
fn reference_srt_matches_independent_axis_product_and_parent_order() {
    let srt = TrSklTransform {
        scale: [1.2, 0.7, 1.8],
        rotation: [0.23, -0.51, 0.87],
        translation: [2.0, -3.0, 4.0],
    };
    // Independent generic matrix products, not the expanded evaluator formula.
    let mut axes = vec![];
    for (axis, &angle) in srt.rotation.iter().enumerate() {
        let (s, c) = f64::from(angle).sin_cos();
        let mut m = TrSklMatrix::IDENTITY;
        let a = (axis + 1) % 3;
        let b = (axis + 2) % 3;
        m.rows[a][a] = c;
        m.rows[a][b] = -s;
        m.rows[b][a] = s;
        m.rows[b][b] = c;
        axes.push(m);
    }
    let mut scale = TrSklMatrix::IDENTITY;
    let mut translation = TrSklMatrix::IDENTITY;
    for (i, &value) in srt.scale.iter().enumerate() {
        scale.rows[i][i] = f64::from(value);
        translation.rows[i][3] = f64::from(srt.translation[i]);
    }
    let expected = translation
        .multiply(&axes[2])
        .unwrap()
        .multiply(&axes[1])
        .unwrap()
        .multiply(&axes[0])
        .unwrap()
        .multiply(&scale)
        .unwrap();
    assert!(
        srt.reference_matrix()
            .unwrap()
            .max_abs_difference(&expected)
            .unwrap()
            < 1e-14
    );
    let mut child = identity_srt();
    child.translation = [1.0, 2.0, -1.0];
    let sk = skeleton(vec![node(srt, None, None), node(child, Some(0), None)], 0);
    let pose = sk.reference_pose().unwrap();
    assert_eq!(
        pose.global[1],
        pose.local[0].multiply(&pose.local[1]).unwrap()
    );
    assert!(
        pose.global[1]
            .max_abs_difference(&pose.local[1].multiply(&pose.local[0]).unwrap())
            .unwrap()
            > 0.1
    );
}

#[test]
fn bind_reader_checks_affine_columns_and_malformed_buffers() {
    let source = synthetic_buffer();
    let sk = TrSkl::parse(&source).unwrap();
    let records = TrSkl::read_bind_records(&source).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].flags, [1, 1]);
    assert_eq!(records[0].matrix.rows[0], [1.0, 0.0, 0.0, -2.0]);
    assert_eq!(records[0].matrix.rows[3], [0.0, 0.0, 0.0, 1.0]);
    sk.validate_bind_rest_pose(&records, 1e-12).unwrap();
    let root = u32::from_le_bytes(source[..4].try_into().unwrap()) as usize;
    let target = |at| at + u32::from_le_bytes(source[at..at + 4].try_into().unwrap()) as usize;
    let bind = target(target(root + 8) + 4);
    let matrix = target(bind + 8);
    for (at, value) in [
        (bind + 8, u32::MAX),
        (matrix + 4, f32::NAN.to_bits()),
        (matrix, i32::MIN as u32),
    ] {
        let mut bytes = source.clone();
        word(&mut bytes, at, value);
        assert!(TrSkl::read_bind_records(&bytes).is_err());
    }
    let mut bytes = source.clone();
    bytes[bind + 4] = 0;
    assert!(TrSkl::read_bind_records(&bytes).is_err());
    for length in 0..source.len() {
        let _ = TrSkl::read_bind_records(&source[..length]);
    }
    for at in 0..source.len() {
        let mut bytes = source.clone();
        bytes[at] ^= 0xff;
        let _ = TrSkl::read_bind_records(&bytes);
    }
}

#[test]
fn reference_math_rejects_unknown_pivots_indices_nonfinite_and_bad_binds() {
    let mut sk = skeleton(vec![node(identity_srt(), None, Some(0))], 1);
    let record = TrSklBindRecord {
        flags: [1, 1],
        matrix: TrSklMatrix::IDENTITY,
    };
    sk.validate_bind_rest_pose(std::slice::from_ref(&record), 1e-4)
        .unwrap();
    for epsilon in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(sk
            .validate_bind_rest_pose(std::slice::from_ref(&record), epsilon)
            .is_err());
    }
    for pivot in [0.01, f32::NAN] {
        sk.nodes[0].scale_pivot[0] = pivot;
        assert!(sk.reference_pose().is_err());
    }
    sk.nodes[0].scale_pivot[0] = 0.0;
    sk.nodes[0].parent = Some(0);
    assert!(sk.reference_pose().is_err());
    sk.nodes[0].parent = None;
    sk.nodes[0].local.rotation[0] = f32::INFINITY;
    assert!(sk.reference_pose().is_err());
    sk.nodes[0].local = identity_srt();
    sk.nodes[0].rig_index = Some(1);
    assert!(sk
        .bind_rest_residuals(std::slice::from_ref(&record))
        .is_err());
    sk.nodes[0].rig_index = Some(0);
    assert!(sk.bind_rest_residuals(&[]).is_err());
    let mut bad = record.clone();
    bad.flags = [0, 1];
    assert!(sk.bind_rest_residuals(&[bad]).is_err());
    let mut bad = record.clone();
    bad.matrix.rows[0][0] = 1.5;
    assert!(sk.validate_bind_rest_pose(&[bad], 1e-4).is_err());
    let mut huge = TrSklMatrix::IDENTITY;
    huge.rows[0][0] = f64::MAX;
    assert!(huge.multiply(&huge).is_err());
    let mut negative = huge.clone();
    negative.rows[0][0] = -f64::MAX;
    assert!(huge.max_abs_difference(&negative).is_err());
    sk.nodes.push(node(identity_srt(), Some(0), Some(0)));
    assert!(sk
        .bind_rest_residuals(std::slice::from_ref(&record))
        .is_err());
    sk.nodes[1].rig_index = None;
    sk.nodes[1].parent = None;
    assert!(sk.reference_pose().is_err());
}

fn matrix(values: &[f64]) -> TrSklMatrix {
    assert_eq!(values.len(), 16);
    TrSklMatrix {
        rows: std::array::from_fn(|r| std::array::from_fn(|c| values[4 * r + c])),
    }
}

#[test]
fn blender_synthetic_differential_when_explicitly_available() {
    let Ok(root) = std::env::var("PLA_MATRIX_REFERENCE") else {
        eprintln!("[skip] private Blender oracle not configured");
        return;
    };
    let text = std::fs::read_to_string(std::path::Path::new(&root).join("synthetic.tsv")).unwrap();
    let mut nodes = vec![];
    let mut references = vec![];
    for line in text.lines().skip(1) {
        let v: Vec<f64> = line.split('\t').map(|v| v.parse().unwrap()).collect();
        assert_eq!(v.len(), 43);
        assert_eq!(v[0] as usize, nodes.len());
        let local = TrSklTransform {
            scale: std::array::from_fn(|i| v[2 + i] as f32),
            rotation: std::array::from_fn(|i| v[5 + i] as f32),
            translation: std::array::from_fn(|i| v[8 + i] as f32),
        };
        nodes.push(node(
            local,
            if v[1] < 0.0 {
                None
            } else {
                Some(v[1] as usize)
            },
            None,
        ));
        references.push((matrix(&v[11..27]), matrix(&v[27..43])));
    }
    assert_eq!(nodes.len(), 3);
    let pose = skeleton(nodes, 0).reference_pose().unwrap();
    let (mut local_max, mut global_max) = (0.0f64, 0.0f64);
    for (i, (local, global)) in references.iter().enumerate() {
        local_max = local_max.max(pose.local[i].max_abs_difference(local).unwrap());
        global_max = global_max.max(pose.global[i].max_abs_difference(global).unwrap());
        assert!(pose.local[i].max_abs_difference(local).unwrap() < 2e-5);
        assert!(pose.global[i].max_abs_difference(global).unwrap() < 2e-5);
    }
    eprintln!("[synthetic reference] nodes=3 local_max={local_max:e} global_max={global_max:e}");
}

#[test]
fn blender_update_corpus_differential_when_explicitly_available() {
    let (Ok(root), Ok(oracle)) = (
        std::env::var("PLA_TRSKL_CORPUS"),
        std::env::var("PLA_MATRIX_REFERENCE"),
    ) else {
        eprintln!("[skip] private corpus/Blender oracle not configured");
        return;
    };
    let metadata = include_str!("../../../reports/skeleton/update-v262144-matrix-reference.tsv");
    let (mut files, mut nodes, mut binds, mut disagreements) = (0, 0, 0, 0);
    let (mut local_max, mut global_max) = (0.0f64, 0.0f64);
    for line in metadata.lines().skip(1) {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 7);
        let bytes = std::fs::read(std::path::Path::new(&root).join(fields[0])).unwrap();
        let sk = TrSkl::parse(&bytes).unwrap();
        let pose = sk.reference_pose().unwrap();
        let records = TrSkl::read_bind_records(&bytes).unwrap();
        assert_eq!(sk.nodes.len(), fields[2].parse::<usize>().unwrap());
        assert_eq!(records.len(), fields[3].parse::<usize>().unwrap());
        let text = std::fs::read_to_string(
            std::path::Path::new(&oracle).join(format!("{}.tsv", fields[1])),
        )
        .unwrap();
        let mut seen = 0;
        for row in text.lines().skip(1) {
            let v: Vec<f64> = row.split('\t').map(|v| v.parse().unwrap()).collect();
            assert_eq!(v.len(), 34);
            let i = v[0] as usize;
            assert_eq!(i, seen);
            assert_eq!(
                if v[1] < 0.0 {
                    None
                } else {
                    Some(v[1] as usize)
                },
                sk.nodes[i].rig_index
            );
            local_max = local_max.max(
                pose.local[i]
                    .max_abs_difference(&matrix(&v[2..18]))
                    .unwrap(),
            );
            global_max = global_max.max(
                pose.global[i]
                    .max_abs_difference(&matrix(&v[18..34]))
                    .unwrap(),
            );
            seen += 1;
        }
        assert_eq!(seen, sk.nodes.len());
        assert!(
            local_max < 2e-5 && global_max < 2e-5,
            "{} reference mismatch",
            fields[0]
        );
        let residuals = sk.bind_rest_residuals(&records).unwrap();
        let a = residuals
            .iter()
            .map(|r| r.global_times_bind_error)
            .fold(0.0, f64::max);
        let b = residuals
            .iter()
            .map(|r| r.bind_times_global_error)
            .fold(0.0, f64::max);
        assert!((a - fields[5].parse::<f64>().unwrap()).abs() < 2e-5);
        assert!((b - fields[6].parse::<f64>().unwrap()).abs() < 2e-5);
        let result = sk.validate_bind_rest_pose(&records, 1e-4);
        if fields[4] == "disagree" {
            assert!(fields[0].ends_with("item_230.trskl"));
            assert!(result.is_err());
            disagreements += 1;
        } else {
            result.unwrap();
        }
        files += 1;
        nodes += sk.nodes.len();
        binds += records.len();
    }
    assert_eq!((files, nodes, binds, disagreements), (199, 1264, 289, 1));
    eprintln!("[reference] files={files} nodes={nodes} binds={binds} rejected_bind_files={disagreements} local_max={local_max:e} global_max={global_max:e}");
}
