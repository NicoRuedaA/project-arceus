//! Lossless skin-lane decoding. Synthetic fixtures contain no game bytes.
use pla::assets::tr::{TrMbf, TrSkl, TrSklNode, TrSklTransform};

fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn skeleton() -> TrSkl {
    let nodes: Vec<_> = (0..6)
        .map(|i| TrSklNode {
            name: format!("node_{i}"),
            local: TrSklTransform {
                scale: [1.0; 3],
                rotation: [0.0; 3],
                translation: [0.0; 3],
            },
            scale_pivot: [0.0; 3],
            rotate_pivot: [0.0; 3],
            parent: (i != 0).then_some(0),
            rig_index: (i >= 2).then(|| i - 2),
        })
        .collect();
    TrSkl {
        bones: nodes.iter().map(|n| n.name.clone()).collect(),
        offsets: vec![],
        nodes,
        bind_count: 4,
        root_flag: 0,
    }
}

fn mesh() -> (Vec<u8>, Vec<u8>) {
    let mut layout = vec![0; 0x240];
    let table = 0x40;
    let declarations = [
        (8, 39, 40),
        (7, 22, 36),
        (6, 48, 28),
        (3, 43, 20),
        (2, 43, 12),
        (1, 51, 0),
    ];
    put(&mut layout, table, 6);
    put(&mut layout, table + 4, 48);
    put(&mut layout, table + 8, 6);
    for (i, (id, code, offset)) in declarations.iter().enumerate() {
        let delta = 4 + (declarations.len() - 1 - i) * 16;
        put(&mut layout, table + 12 + i * 4, delta as u32);
        let record = table + delta + 20 + i * 4;
        put(&mut layout, record, *id);
        put(&mut layout, record + 4, *code);
        put(
            &mut layout,
            if *id == 1 { record - 4 } else { record + 8 },
            *offset,
        );
    }
    let index_at = 0x44 + 96 + 28;
    let mut bytes = vec![0; index_at + 6];
    put(&mut bytes, 0, 12);
    put(&mut bytes, 0x28, 96 + 28);
    put(&mut bytes, 0x40, 96);
    for i in 0..2 {
        let at = 0x44 + i * 48;
        bytes[at + 36..at + 40].copy_from_slice(&[0, 1, 2, 3]);
        for (j, weight) in [32768u16, 16384, 8192, 8191].iter().enumerate() {
            bytes[at + 40 + 2 * j..at + 42 + 2 * j].copy_from_slice(&weight.to_le_bytes());
        }
    }
    for (i, index) in [0u16, 1, 1].iter().enumerate() {
        bytes[index_at + 2 * i..index_at + 2 * i + 2].copy_from_slice(&index.to_le_bytes());
    }
    (bytes, layout)
}

#[test]
fn skin_channels_preserve_four_weights_and_map_rigs_not_nodes() {
    let (bytes, layout) = mesh();
    let mesh = TrMbf::parse_with_layout(&bytes, &layout).unwrap();
    let lane = mesh.skin_influences(&bytes, 0, &skeleton()).unwrap();
    assert_eq!(lane.rig_indices, [0, 1, 2, 3]);
    assert_eq!(lane.weights_u16, [32768, 16384, 8192, 8191]);
    assert_eq!(lane.node_indices, [Some(2), Some(3), Some(4), Some(5)]);
    assert_eq!(lane.unorm_weights()[0], 32768.0 / 65535.0);
}

#[test]
fn skin_zero_weight_disables_lane_but_joint_zero_can_be_active() {
    let (mut bytes, layout) = mesh();
    bytes[0x44 + 36..0x44 + 40].copy_from_slice(&[0, 255, 255, 255]);
    bytes[0x44 + 40..0x44 + 48].copy_from_slice(&[255, 255, 0, 0, 0, 0, 0, 0]);
    let mesh = TrMbf::parse_with_layout(&bytes, &layout).unwrap();
    let lane = mesh.skin_influences(&bytes, 0, &skeleton()).unwrap();
    assert_eq!(lane.node_indices, [Some(2), None, None, None]);
    // Inactive 255 is a synthetic robustness case, not an observed sentinel.
    bytes[0x44 + 36] = 255;
    assert!(mesh.skin_influences(&bytes, 0, &skeleton()).is_err());
}

#[test]
fn skin_quantization_accepts_only_observed_sum_error_without_renormalizing() {
    let (bytes, layout) = mesh();
    let mesh = TrMbf::parse_with_layout(&bytes, &layout).unwrap();
    for sum in [65534u32, 65535, 65536] {
        let mut bytes = bytes.clone();
        bytes[0x44 + 40..0x44 + 42].copy_from_slice(&32768u16.to_le_bytes());
        bytes[0x44 + 42..0x44 + 44].copy_from_slice(&((sum - 32768) as u16).to_le_bytes());
        bytes[0x44 + 44..0x44 + 48].fill(0);
        let lane = mesh.skin_influences(&bytes, 0, &skeleton()).unwrap();
        assert_eq!(
            lane.weights_u16.iter().map(|&v| u32::from(v)).sum::<u32>(),
            sum
        );
        assert_eq!(lane.unorm_weights()[1], (sum - 32768) as f32 / 65535.0);
    }
    let mut bytes = bytes;
    bytes[0x44 + 40..0x44 + 48].fill(0);
    assert!(mesh.skin_influences(&bytes, 0, &skeleton()).is_err());
}

#[test]
fn skin_rejects_missing_layout_wrong_format_bounds_and_unresolved_rigs() {
    let (bytes, layout) = mesh();
    let mesh = TrMbf::parse_with_layout(&bytes, &layout).unwrap();
    assert!(TrMbf::parse(&bytes)
        .unwrap()
        .skin_influences(&bytes, 0, &skeleton())
        .is_err());
    assert!(mesh.skin_influences(&bytes, 2, &skeleton()).is_err());
    assert!(mesh
        .skin_influences(&bytes[..0x44 + 47], 0, &skeleton())
        .is_err());
    let mut bad = mesh.clone();
    bad.layout
        .as_mut()
        .unwrap()
        .attributes
        .iter_mut()
        .find(|a| a.id == 7)
        .unwrap()
        .format_code = 20;
    assert!(bad.skin_influences(&bytes, 0, &skeleton()).is_err());
    let mut bad = mesh.clone();
    bad.vertex_offset = 0x60;
    assert!(bad.skin_influences(&bytes, 0, &skeleton()).is_err());
    let mut skel = skeleton();
    skel.bind_count = 3;
    assert!(mesh.skin_influences(&bytes, 0, &skel).is_err());
    let mut skel = skeleton();
    skel.nodes[2].rig_index = None;
    assert!(mesh.skin_influences(&bytes, 0, &skel).is_err());
    skel.nodes[2].rig_index = Some(1);
    assert!(mesh.skin_influences(&bytes, 0, &skel).is_err());
}

#[test]
fn skin_multi_influence_reference_models_when_explicitly_available() {
    let Ok(root) = std::env::var("PLA_SKIN_CORPUS") else {
        eprintln!("[skip] set PLA_SKIN_CORPUS to the authorized local-private update RomFS");
        return;
    };
    for (relative, expected, positive_zero) in [
        (
            "bin/chara/data/item/item_255/item_255",
            [303, 500, 233, 123],
            528,
        ),
        (
            "bin/demo/graphic/sd/sd9150/sd9150_mysterygift/mdl/sd9150_mysterygift",
            [831, 624, 0, 0],
            0,
        ),
    ] {
        let path = std::path::Path::new(&root).join(relative);
        let bytes = std::fs::read(path.with_extension("trmbf")).unwrap();
        let layout = std::fs::read(path.with_extension("trmsh")).unwrap();
        let skeleton = TrSkl::parse(&std::fs::read(path.with_extension("trskl")).unwrap()).unwrap();
        let mesh = TrMbf::parse_skin_shape(&bytes, &layout, 0).unwrap();
        let mut counts = [0; 4];
        let mut zero = 0;
        for i in 0..mesh.vertex_count {
            let lane = mesh.skin_influences(&bytes, i, &skeleton).unwrap();
            let active = lane.weights_u16.iter().filter(|&&w| w > 0).count();
            counts[active - 1] += 1;
            for j in 0..4 {
                if lane.weights_u16[j] > 0 {
                    zero += usize::from(lane.rig_indices[j] == 0);
                    let node = &skeleton.nodes[lane.node_indices[j].unwrap()];
                    assert_eq!(node.rig_index, Some(usize::from(lane.rig_indices[j])));
                }
            }
        }
        assert_eq!(counts, expected, "{relative}");
        assert_eq!(zero, positive_zero, "{relative}");
    }
}

#[test]
fn skin_model_linked_census_when_explicitly_available() {
    let Ok(root) = std::env::var("PLA_SKIN_CORPUS") else {
        eprintln!("[skip] explicit PLA_SKIN_CORPUS is required for model-linked checks");
        return;
    };
    let report = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reports/skeleton/update-v262144-skin-census.tsv");
    let report = std::fs::read_to_string(report).unwrap();
    let mut shapes = 0;
    let mut total_vertices = 0;
    for line in report.lines().skip(1) {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 11);
        let read = |index| std::fs::read(std::path::Path::new(&root).join(fields[index])).unwrap();
        let bytes = read(2);
        let layout = read(1);
        let skeleton = TrSkl::parse(&read(3)).unwrap();
        let shape = fields[4].parse().unwrap();
        let mesh = TrMbf::parse_skin_shape(&bytes, &layout, shape)
            .unwrap_or_else(|e| panic!("{} shape {shape}: {e}", fields[0]));
        assert_eq!(mesh.vertex_count, fields[5].parse::<usize>().unwrap());
        let mut counts = [0usize; 4];
        let mut zero = 0;
        for i in 0..mesh.vertex_count {
            let lane = mesh.skin_influences(&bytes, i, &skeleton).unwrap();
            let active = lane.weights_u16.iter().filter(|&&w| w > 0).count();
            counts[active - 1] += 1;
            for j in 0..4 {
                if lane.weights_u16[j] > 0 {
                    zero += usize::from(lane.rig_indices[j] == 0);
                }
            }
        }
        for i in 0..4 {
            assert_eq!(counts[i], fields[i + 6].parse::<usize>().unwrap());
        }
        assert_eq!(zero, fields[10].parse::<usize>().unwrap());
        shapes += 1;
        total_vertices += mesh.vertex_count;
    }
    assert_eq!((shapes, total_vertices), (177, 128303));
}
