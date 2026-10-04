//! Authored synthetic values only; actual importer oracles remain opted-in/private.
use pla::assets::tr::{reference_unpack_rotation, TrAnm, TrAnmChannel, TrAnmEncoding, TrAnmKey};
use std::io::{Cursor, Read};

fn pack(values: [u16; 3], selector: u8, negative: bool) -> [u16; 3] {
    let p = (u64::from(values[0]) << 3)
        | (u64::from(values[1]) << 18)
        | (u64::from(values[2]) << 33)
        | u64::from(selector)
        | (u64::from(negative) << 2);
    [p as u16, (p >> 16) as u16, (p >> 32) as u16]
}

#[test]
fn reconstruction_checks_component_order_sign_extents_and_no_normalization() {
    // Distinct authored components make ordering errors visible in each selector.
    let words = [17000, 23000, 12000];
    let v = words
        .map(|n| n as f64 * (std::f64::consts::FRAC_PI_2 / 32767.0) - std::f64::consts::FRAC_PI_4);
    let omitted = (1.0 - v[0] * v[0] - v[1] * v[1] - v[2] * v[2]).sqrt() as f32;
    for selector in 0..4 {
        let q = reference_unpack_rotation(pack(words, selector, false));
        assert_eq!(q[selector as usize], omitted);
        let rest: Vec<_> = q
            .iter()
            .enumerate()
            .filter_map(|(i, &a)| (i != selector as usize).then_some(a))
            .collect();
        assert_eq!(rest, v.map(|a| a as f32));
        assert_eq!(
            reference_unpack_rotation(pack(words, selector, true)),
            q.map(|a| -a)
        );
        for extremum in [0, 32767] {
            let clamped = reference_unpack_rotation(pack([extremum; 3], selector, false));
            assert_eq!(clamped[selector as usize], 0.0);
            assert!(clamped.iter().map(|v| v * v).sum::<f32>() > 1.8);
            // Reference clamps invalid radicands but does NOT repair/normalize words.
        }
    }
}

fn channel(encoding: TrAnmEncoding, frames: &[u32]) -> TrAnmChannel<usize> {
    TrAnmChannel {
        encoding,
        keys: frames
            .iter()
            .enumerate()
            .map(|(i, &frame)| TrAnmKey { frame, value: i })
            .collect(),
    }
}

#[test]
fn integer_records_preserve_first_duplicates_and_absence_without_hold_or_loop() {
    for encoding in [TrAnmEncoding::Framed8, TrAnmEncoding::Framed16] {
        let ch = channel(encoding, &[0, 0, 3, 5, 5]);
        let view = ch.reference_records(6).unwrap();
        assert_eq!(view.record_at(0).unwrap(), Some(&0));
        assert_eq!(view.record_at(3).unwrap(), Some(&2));
        assert_eq!(view.record_at(5).unwrap(), Some(&3));
        assert_eq!(view.record_at(1).unwrap(), None);
        assert!(view.record_at(6).is_err());
        assert_eq!(ch.keys.len(), 5);
    }
    let fixed = channel(TrAnmEncoding::Fixed, &[0]);
    let view = fixed.reference_records(10).unwrap();
    assert_eq!(view.record_at(0).unwrap(), Some(&0));
    assert_eq!(view.record_at(1).unwrap(), None);
    let dense = channel(TrAnmEncoding::Dense, &[0, 1, 2]);
    let view = dense.reference_records(3).unwrap();
    assert_eq!(view.record_at(2).unwrap(), Some(&2));
    let high = channel(TrAnmEncoding::Framed16, &[0, 300, 65535]);
    assert_eq!(
        high.reference_records(65536)
            .unwrap()
            .record_at(300)
            .unwrap(),
        Some(&1)
    );
}

#[test]
fn public_channels_are_revalidated_before_indexing() {
    for (encoding, frames, count) in [
        (TrAnmEncoding::Fixed, vec![], 3),
        (TrAnmEncoding::Fixed, vec![1], 3),
        (TrAnmEncoding::Fixed, vec![0, 1], 3),
        (TrAnmEncoding::Dense, vec![0, 2], 3),
        (TrAnmEncoding::Dense, vec![0, 0, 2], 3),
        (TrAnmEncoding::Framed8, vec![2, 1], 3),
        (TrAnmEncoding::Framed8, vec![0, 3], 3),
        (TrAnmEncoding::Framed8, vec![0, 256], 300),
        (TrAnmEncoding::Framed16, vec![0, 65536], 70000),
        (TrAnmEncoding::Fixed, vec![0], 0),
        (TrAnmEncoding::Fixed, vec![0], 1000001),
    ] {
        assert!(channel(encoding, &frames).reference_records(count).is_err());
    }
    assert!(channel(TrAnmEncoding::Framed16, &vec![0; 1000001])
        .reference_records(2)
        .is_err());
}

fn u32_from(cursor: &mut Cursor<&[u8]>) -> u32 {
    let mut b = [0; 4];
    cursor.read_exact(&mut b).unwrap();
    u32::from_le_bytes(b)
}

fn compare(
    cursor: &mut Cursor<&[u8]>,
    values: &[f32],
    max_error: &mut f32,
    path: &str,
    track: usize,
    frame: u32,
    lane: &str,
) {
    for (component, &actual) in values.iter().enumerate() {
        let expected = f32::from_bits(u32_from(cursor));
        let error = (actual - expected).abs();
        assert!(expected.is_finite() && error <= 1e-7,
            "{path}: track={track} frame/record={frame} lane={lane} component={component} reference mismatch");
        *max_error = max_error.max(error);
    }
}

#[test]
fn entire_actual_corpus_matches_executed_selected_importer_helpers() {
    let (Ok(root), Ok(oracles)) = (
        std::env::var("PLA_ANIMATION_CORPUS"),
        std::env::var("PLA_ANIMATION_REFERENCE"),
    ) else {
        eprintln!("[skip] private animation corpus/reference not configured");
        return;
    };
    let metadata = include_str!("../../../reports/skeleton/update-v262144-animation-reference.tsv");
    let (mut files, mut tracks, mut records, mut slots, mut compared) = (0, 0, 0, 0, 0);
    let mut max_error = 0.0;
    let synthetic = std::fs::read(std::path::Path::new(&oracles).join("synthetic.bin")).unwrap();
    assert_eq!(synthetic.len(), 40 * 22);
    let mut cursor = Cursor::new(synthetic.as_slice());
    for case in 0..40 {
        let mut words = [0; 3];
        for word in &mut words {
            let mut bytes = [0; 2];
            cursor.read_exact(&mut bytes).unwrap();
            *word = u16::from_le_bytes(bytes);
        }
        compare(
            &mut cursor,
            &reference_unpack_rotation(words),
            &mut max_error,
            "author-created synthetic",
            0,
            case,
            "rotation",
        );
    }
    for line in metadata.lines().skip(1) {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 4);
        let source = std::fs::read(std::path::Path::new(&root).join(fields[0])).unwrap();
        let clip = TrAnm::parse_tracks(&source).unwrap();
        let trace =
            std::fs::read(std::path::Path::new(&oracles).join(format!("{}.bin", fields[1])))
                .unwrap();
        assert_eq!(trace.len(), fields[3].parse::<usize>().unwrap());
        let mut cursor = Cursor::new(trace.as_slice());
        assert_eq!(u32_from(&mut cursor), clip.tracks.len() as u32);
        assert_eq!(u32_from(&mut cursor), clip.frame_count);
        for (i, track) in clip.tracks.iter().enumerate() {
            assert_eq!(u32_from(&mut cursor), track.rotation.keys.len() as u32);
            for (j, key) in track.rotation.keys.iter().enumerate() {
                compare(
                    &mut cursor,
                    &reference_unpack_rotation(key.value),
                    &mut max_error,
                    fields[0],
                    i,
                    j as u32,
                    "rotation record",
                );
                records += 1;
                compared += 4;
            }
            let scale = track.scale.reference_records(clip.frame_count).unwrap();
            let rotation = track.rotation.reference_records(clip.frame_count).unwrap();
            let translation = track
                .translation
                .reference_records(clip.frame_count)
                .unwrap();
            for frame in 0..clip.frame_count {
                let s = scale.record_at(frame).unwrap();
                let r = rotation
                    .record_at(frame)
                    .unwrap()
                    .copied()
                    .map(reference_unpack_rotation);
                let t = translation.record_at(frame).unwrap();
                let mut expected = [0];
                cursor.read_exact(&mut expected).unwrap();
                let mask = u8::from(s.is_some())
                    | (u8::from(r.is_some()) << 1)
                    | (u8::from(t.is_some()) << 2);
                assert_eq!(
                    mask, expected[0],
                    "{}: track={i} frame={frame} presence mismatch",
                    fields[0]
                );
                for (lane, values) in [
                    ("scale", s.map(|v| v.as_slice())),
                    ("rotation", r.as_ref().map(|v| v.as_slice())),
                    ("translation", t.map(|v| v.as_slice())),
                ] {
                    if let Some(values) = values {
                        compare(
                            &mut cursor,
                            values,
                            &mut max_error,
                            fields[0],
                            i,
                            frame,
                            lane,
                        );
                        compared += values.len();
                    }
                }
                slots += 1;
            }
            tracks += 1;
        }
        assert_eq!(
            cursor.position() as usize,
            trace.len(),
            "{}: unconsumed oracle",
            fields[0]
        );
        files += 1;
    }
    assert_eq!((files, tracks, records), (1522, 25394, 1264143));
    assert_eq!(slots, 8546341);
    eprintln!("[selected importer differential] files={files} tracks={tracks} rotation_records={records} track_frame_slots={slots} compared_components={compared} max_absolute_error={max_error}");
}
