//! Authored inputs; original native outputs are opt-in and remain private.
use pla::assets::tr::{
    TrAnmChannel, TrAnmEncoding, TrAnmKey, TrAnmNativeContext, TrAnmNativeError,
    TrAnmNativeFramedRotation,
};

fn pack(parts: [u16; 3], selector: u16, negative: bool) -> [u16; 3] {
    let word = (u64::from(parts[0]) << 3)
        | (u64::from(parts[1]) << 18)
        | (u64::from(parts[2]) << 33)
        | u64::from(selector)
        | (u64::from(negative) << 2);
    [word as u16, (word >> 16) as u16, (word >> 32) as u16]
}

fn channel(encoding: TrAnmEncoding) -> TrAnmChannel<[u16; 3]> {
    TrAnmChannel {
        encoding,
        keys: [0, 0, 10, 20, 20]
            .into_iter()
            .zip([16384, 16384, 19000, 21000, 21000])
            .map(|(frame, part)| TrAnmKey {
                frame,
                value: pack([part; 3], 3, false),
            })
            .collect(),
    }
}

#[test]
fn explicit_context_rejects_unknown_fpcr_and_epsilon() {
    assert_eq!(
        TrAnmNativeContext::new(1, 1e-12).unwrap_err(),
        TrAnmNativeError::UnsupportedFpcr
    );
    for epsilon in [0.0, -0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(
            TrAnmNativeContext::new(0, epsilon).unwrap_err(),
            TrAnmNativeError::InvalidEpsilon
        );
    }
}

#[test]
fn checked_guard_structure_and_position_domain() {
    let mut data = channel(TrAnmEncoding::Framed8);
    let context = TrAnmNativeContext::new(0, 1e-12).unwrap();
    let view = TrAnmNativeFramedRotation::new(&data).unwrap();
    for position in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            view.evaluate(position, context),
            Err(TrAnmNativeError::NonfinitePosition)
        );
    }
    for position in [-0.25, 20.25] {
        assert_eq!(
            view.evaluate(position, context),
            Err(TrAnmNativeError::PositionOutsideGuards)
        );
    }
    data.keys[0].frame = 1;
    assert_eq!(
        TrAnmNativeFramedRotation::new(&data).unwrap_err(),
        TrAnmNativeError::InvalidGuardRecords
    );
    data.keys[0].frame = 0;
    data.keys[2].frame = 0;
    assert_eq!(
        TrAnmNativeFramedRotation::new(&data).unwrap_err(),
        TrAnmNativeError::InvalidFrameOrder
    );
    data.keys[2].frame = 256;
    assert_eq!(
        TrAnmNativeFramedRotation::new(&data).unwrap_err(),
        TrAnmNativeError::FrameWidthExceeded
    );
    data.encoding = TrAnmEncoding::Dense;
    assert_eq!(
        TrAnmNativeFramedRotation::new(&data).unwrap_err(),
        TrAnmNativeError::UnsupportedEncoding
    );
    data.encoding = TrAnmEncoding::Framed16;
    data.keys.truncate(3);
    assert_eq!(
        TrAnmNativeFramedRotation::new(&data).unwrap_err(),
        TrAnmNativeError::InvalidGuardRecords
    );
}

#[test]
fn endpoint_decode_sign_and_strict_epsilon_mask() {
    let context = TrAnmNativeContext::new(0, 1e-12).unwrap();
    for selector in 0..4 {
        let positive = context
            .unpack_rotation(pack([12000, 19000, 22000], selector, false))
            .unwrap();
        let negative = context
            .unpack_rotation(pack([12000, 19000, 22000], selector, true))
            .unwrap();
        assert_eq!(negative.map(f32::to_bits), positive.map(|x| (-x).to_bits()));
    }
    assert_eq!(
        context.unpack_rotation(pack([32767; 3], 3, false)).unwrap()[3].to_bits(),
        0x7fc0_0000
    );
    let data = channel(TrAnmEncoding::Framed16);
    let view = TrAnmNativeFramedRotation::new(&data).unwrap();
    assert_eq!(
        view.evaluate(0.0, context).unwrap(),
        context.unpack_rotation(data.keys[1].value).unwrap()
    );
    assert_eq!(
        view.evaluate(20.0, context).unwrap(),
        context.unpack_rotation(data.keys[3].value).unwrap()
    );
    let mask = TrAnmNativeContext::new(0, 2.0).unwrap();
    assert_eq!(view.evaluate(5.0, mask).unwrap().map(f32::to_bits), [0; 4]);
    assert_ne!(view.evaluate(0.0, mask).unwrap().map(f32::to_bits), [0; 4]);
    let mut identity = data.clone();
    for key in &mut identity.keys {
        key.value = pack([16384; 3], 3, false);
    }
    let identity = TrAnmNativeFramedRotation::new(&identity).unwrap();
    let equal = TrAnmNativeContext::new(0, 1.0).unwrap();
    let above = TrAnmNativeContext::new(0, f32::from_bits(1.0_f32.to_bits() + 1)).unwrap();
    assert_ne!(
        identity.evaluate(5.0, equal).unwrap().map(f32::to_bits),
        [0; 4]
    );
    assert_eq!(
        identity.evaluate(5.0, above).unwrap().map(f32::to_bits),
        [0; 4]
    );
}

#[test]
fn stateless_widths_backward_queries_and_antipodal_midpoint() {
    let mut a = channel(TrAnmEncoding::Framed16);
    let b = channel(TrAnmEncoding::Framed8);
    let context = TrAnmNativeContext::new(0, 1e-12).unwrap();
    let va = TrAnmNativeFramedRotation::new(&a).unwrap();
    let vb = TrAnmNativeFramedRotation::new(&b).unwrap();
    for t in [0.0, 7.5, 2.5, 20.0, 10.0, 0.25] {
        assert_eq!(
            va.evaluate(t, context).unwrap().map(f32::to_bits),
            vb.evaluate(t, context).unwrap().map(f32::to_bits)
        );
    }
    for (i, key) in a.keys.iter_mut().enumerate() {
        key.value = pack([16384; 3], 3, i >= 2);
    }
    assert_eq!(
        TrAnmNativeFramedRotation::new(&a)
            .unwrap()
            .evaluate(5.0, context)
            .unwrap()
            .map(f32::to_bits),
        [0; 4]
    );
}

#[test]
fn native_nan_words_signs_lazy_controls_and_unordered_mask() {
    let context = TrAnmNativeContext::new(0, 1e-12).unwrap();
    // Authored exact-zero radicand and immediate quantized neighbors. This is
    // reachable packed data, not an arbitrary injected IEEE NaN.
    let zero_parts = [29019_u16, 19921, 32600].map(|part| {
        (part as f32).mul_add(f32::from_bits(0x3849_116d), f32::from_bits(0xbf49_0fdb))
    });
    let squares = zero_parts.map(|part| part * part);
    assert_eq!(1.0 - ((squares[0] + squares[1]) + (squares[2] + 0.0)), 0.0);
    for parts in [[32767; 3], [29019, 19921, 32600], [29019, 19921, 32601]] {
        for selector in 0..4 {
            for negative in [false, true] {
                let q = context
                    .unpack_rotation(pack(parts, selector, negative))
                    .unwrap();
                assert_eq!(
                    q[selector as usize].to_bits(),
                    if negative { 0xffc0_0000 } else { 0x7fc0_0000 }
                );
                assert_eq!(q.iter().filter(|x| x.is_nan()).count(), 1);
            }
        }
    }
    assert!(context
        .unpack_rotation(pack([29019, 19921, 32599], 3, false))
        .unwrap()
        .iter()
        .all(|x| x.is_finite()));
    let mut data = channel(TrAnmEncoding::Framed16);
    data.keys[0].value = pack([32767; 3], 0, true);
    let view = TrAnmNativeFramedRotation::new(&data).unwrap();
    assert!(view
        .evaluate(0.0, context)
        .unwrap()
        .iter()
        .all(|x| x.is_finite()));
    assert!(view
        .evaluate(20.0, context)
        .unwrap()
        .iter()
        .all(|x| x.is_finite()));
    let nan = view.evaluate(5.0, context).unwrap();
    assert!(nan.iter().all(|x| x.is_nan()));
    let high_epsilon = TrAnmNativeContext::new(0, 65535.0).unwrap();
    assert_eq!(
        view.evaluate(5.0, high_epsilon).unwrap().map(f32::to_bits),
        nan.map(f32::to_bits)
    );
    // NaN norm is unordered: a high positive epsilon must NOT mask it to zero.
}

/// Private binary schema: magic, count, then width/epsilon/position/key-count,
/// authored frame+3u16 keys, four native output words, FPCR and FPSR. No native
/// outputs are committed. Width zero selects direct packed decode only. The
/// compatible dec102 schema and dec103 runs/hashes/conditions are documented.
#[test]
fn optional_original_byte_oracle() {
    let Some(path) = std::env::var_os("PLA_NATIVE_ROTATION_ORACLE") else {
        eprintln!("[skip] PLA_NATIVE_ROTATION_ORACLE not supplied; no native proof ran");
        return;
    };
    use std::io::Read;
    let file = std::fs::File::open(path).expect("open private oracle");
    let mut bytes = Vec::new();
    file.take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .expect("read private oracle");
    assert!(bytes.len() <= 8 * 1024 * 1024);
    let mut reader = OracleReader {
        bytes: &bytes,
        offset: 0,
    };
    let magic = reader.take(8);
    let sticky = magic == b"DEC104\0\x01";
    assert!(sticky || magic == b"DEC102\0\x01");
    let count = reader.u32();
    assert!(count > 0 && count <= 10_000);
    let mut compared = 0;
    let mut nonfinite = 0;
    for case in 0..count {
        let encoding = match reader.u32() {
            0 => TrAnmEncoding::Fixed, // Direct native packed decode, not a fixed player.
            16 => TrAnmEncoding::Framed16,
            8 => TrAnmEncoding::Framed8,
            _ => panic!("unknown fixture width"),
        };
        let epsilon = f32::from_bits(reader.u32());
        let position = f32::from_bits(reader.u32());
        let initial_fpsr = if sticky { reader.u32() } else { 0 };
        let length = reader.u32();
        assert!((1..=1024).contains(&length));
        let keys = (0..length)
            .map(|_| TrAnmKey {
                frame: reader.u32(),
                value: [reader.u16(), reader.u16(), reader.u16()],
            })
            .collect();
        let expected = [reader.u32(), reader.u32(), reader.u32(), reader.u32()];
        let fpcr = reader.u64();
        let native_fpsr = reader.u64();
        assert_eq!(fpcr, 0);
        let data = TrAnmChannel { encoding, keys };
        let context = TrAnmNativeContext::new(fpcr, epsilon).unwrap();
        let actual = if encoding == TrAnmEncoding::Fixed {
            assert_eq!(data.keys.len(), 1);
            context.unpack_rotation_with_fpsr(data.keys[0].value, initial_fpsr)
        } else {
            TrAnmNativeFramedRotation::new(&data)
                .unwrap()
                .evaluate_with_fpsr(position, context, initial_fpsr)
        };
        let actual = actual.unwrap();
        assert_eq!(
            u64::from(actual.fpsr),
            native_fpsr,
            "native FPSR case {case}"
        );
        assert_eq!(
            actual.rotation.map(f32::to_bits),
            expected,
            "native case {case}"
        );
        nonfinite += usize::from(
            expected
                .iter()
                .any(|&bits| !f32::from_bits(bits).is_finite()),
        );
        compared += 1;
    }
    assert_eq!(reader.offset, bytes.len(), "trailing fixture data");
    eprintln!("native output-bit comparisons={compared}; nonfinite output vectors={nonfinite}; FPSR comparisons={compared}");
}

struct OracleReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl OracleReader<'_> {
    fn take(&mut self, count: usize) -> &[u8] {
        let end = self.offset.checked_add(count).unwrap();
        let data = self.bytes.get(self.offset..end).expect("truncated oracle");
        self.offset = end;
        data
    }
    fn u16(&mut self) -> u16 {
        u16::from_le_bytes(self.take(2).try_into().unwrap())
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.take(4).try_into().unwrap())
    }
    fn u64(&mut self) -> u64 {
        u64::from_le_bytes(self.take(8).try_into().unwrap())
    }
}

#[test]
fn cumulative_flags_include_discarded_decode_and_preserve_wrappers() {
    let context = TrAnmNativeContext::new(0, 1e-12).unwrap();
    assert_eq!(
        context
            .unpack_rotation_with_fpsr([0; 3], 0x100)
            .unwrap_err(),
        TrAnmNativeError::UnsupportedFpsrState
    );
    let mut data = channel(TrAnmEncoding::Framed16);
    data.keys[1].value = pack([32767; 3], 2, true);
    let view = TrAnmNativeFramedRotation::new(&data).unwrap();
    let evaluated = view.evaluate_with_fpsr(10.0, context, 0x0800_0084).unwrap();
    assert!(evaluated.rotation.iter().all(|x| x.is_finite()));
    assert_eq!(evaluated.fpsr, 0x0800_0097); // unused decode still IOC/DZC/IXC
    assert_eq!(view.evaluate(10.0, context).unwrap(), evaluated.rotation);
    for epsilon in [f32::from_bits(1), 1e-12, 65535.0] {
        let context = TrAnmNativeContext::new(0, epsilon).unwrap();
        let nan = context
            .unpack_rotation_with_fpsr(pack([32767; 3], 0, false), 0)
            .unwrap();
        assert_eq!(nan.fpsr, 0x13);
    }
}

#[test]
fn cache_constructor_reset_and_checked_rebind() {
    use pla::assets::tr::TrAnmNativeFramedRotationCache;
    let data = channel(TrAnmEncoding::Framed16);
    let alternate = channel(TrAnmEncoding::Framed8);
    let mut invalid = alternate.clone();
    invalid.keys[2].frame = 0;
    let mut cache = TrAnmNativeFramedRotationCache::new(&data).unwrap();
    let mut expected = [0; 176];
    expected[8..12].copy_from_slice(&f32::MAX.to_bits().to_le_bytes());
    expected[12..16].copy_from_slice(&1u32.to_le_bytes());
    assert_eq!(cache.snapshot(), expected);
    let context = TrAnmNativeContext::new(0, 1e-12).unwrap();
    cache.evaluate(7.5, context).unwrap();
    let before = cache.snapshot();
    assert!(cache.rebind(&invalid).is_err());
    assert_eq!(cache.snapshot(), before);
    cache.rebind(&alternate).unwrap();
    assert_eq!(cache.snapshot(), expected);
    cache.evaluate(0.0, context).unwrap();
    cache.reset();
    assert_eq!(cache.snapshot(), expected);
}

#[test]
fn cache_invalid_inputs_do_not_mutate_state() {
    use pla::assets::tr::TrAnmNativeFramedRotationCache;
    let data = channel(TrAnmEncoding::Framed8);
    let mut cache = TrAnmNativeFramedRotationCache::new(&data).unwrap();
    let context = TrAnmNativeContext::new(0, 1e-12).unwrap();
    let before = cache.snapshot();
    for position in [f32::NAN, f32::INFINITY, -1.0, 21.0] {
        assert!(cache.evaluate(position, context).is_err());
        assert_eq!(cache.snapshot(), before);
    }
    assert_eq!(
        cache.evaluate_with_fpsr(0.0, context, 0x100),
        Err(TrAnmNativeError::UnsupportedFpsrState)
    );
    assert_eq!(cache.snapshot(), before);
}

#[test]
fn cache_hit_does_not_replay_unused_decode_exceptions() {
    use pla::assets::tr::TrAnmNativeFramedRotationCache;
    let mut data = channel(TrAnmEncoding::Framed8);
    data.keys[3].value = pack([32767; 3], 2, true);
    let context = TrAnmNativeContext::new(0, 1e-12).unwrap();
    let mut cache = TrAnmNativeFramedRotationCache::new(&data).unwrap();
    let cold = cache.evaluate_with_fpsr(0.0, context, 0).unwrap();
    let hit = cache.evaluate_with_fpsr(0.0, context, 0).unwrap();
    assert_eq!(
        cold.rotation.map(f32::to_bits),
        hit.rotation.map(f32::to_bits)
    );
    assert_ne!(cold.fpsr & 3, 0);
    assert_eq!(hit.fpsr, 0);
    assert_eq!(
        cache
            .evaluate_with_fpsr(0.0, context, 0x08000084)
            .unwrap()
            .fpsr,
        0x08000084
    );
}

#[test]
fn optional_original_byte_cache_oracle() {
    use pla::assets::tr::TrAnmNativeFramedRotationCache;
    let Ok(path) = std::env::var("PLA_NATIVE_ROTATION_CACHE_ORACLE") else {
        eprintln!("[skip] native cache oracle not supplied");
        return;
    };
    let bytes = std::fs::read(path).unwrap();
    assert!(bytes.len() <= 8 * 1024 * 1024);
    assert_eq!(&bytes[..8], b"DEC106\0\x01");
    let mut offset = 8;
    fn word(bytes: &[u8], offset: &mut usize) -> u32 {
        let value = u32::from_le_bytes(bytes[*offset..*offset + 4].try_into().unwrap());
        *offset += 4;
        value
    }
    let sequences = word(&bytes, &mut offset);
    assert!(sequences <= 128);
    let mut calls = 0;
    let mut nonfinite = 0;
    for sequence in 0..sequences {
        let width = word(&bytes, &mut offset);
        let epsilon = f32::from_bits(word(&bytes, &mut offset));
        let count = word(&bytes, &mut offset) as usize;
        let call_count = word(&bytes, &mut offset);
        assert!(count <= 128 && call_count <= 256);
        let mut data = TrAnmChannel {
            encoding: match width {
                8 => TrAnmEncoding::Framed8,
                16 => TrAnmEncoding::Framed16,
                _ => panic!("invalid width"),
            },
            keys: Vec::with_capacity(count),
        };
        for _ in 0..count {
            let frame = word(&bytes, &mut offset);
            let mut value = [0; 3];
            for part in &mut value {
                *part = u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap());
                offset += 2;
            }
            data.keys.push(TrAnmKey { frame, value });
        }
        let context = TrAnmNativeContext::new(0, epsilon).unwrap();
        let mut cache = TrAnmNativeFramedRotationCache::new(&data).unwrap();
        for call in 0..call_count {
            let reset = word(&bytes, &mut offset);
            let position = f32::from_bits(word(&bytes, &mut offset));
            let initial = word(&bytes, &mut offset);
            if reset != 0 {
                cache.reset();
            }
            let expected = [0; 4].map(|_| word(&bytes, &mut offset));
            let fpcr = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
            offset += 8;
            let fpsr = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
            offset += 8;
            let state = &bytes[offset..offset + 176];
            offset += 176;
            assert_eq!(fpcr, 0);
            let result = cache
                .evaluate_with_fpsr(position, context, initial)
                .unwrap();
            assert_eq!(
                result.rotation.map(f32::to_bits),
                expected,
                "output sequence {sequence} call {call}"
            );
            assert_eq!(
                u64::from(result.fpsr),
                fpsr,
                "FPSR sequence {sequence} call {call}"
            );
            assert_eq!(
                cache.snapshot().as_slice(),
                state,
                "state sequence {sequence} call {call}"
            );
            calls += 1;
            nonfinite += result.rotation.iter().any(|x| !x.is_finite()) as usize;
        }
    }
    assert_eq!(offset, bytes.len());
    eprintln!("native cache oracle: {calls} outputs/FPSR/176-byte states, {nonfinite} nonfinite outputs, zero mismatches");
}

#[test]
fn cache_checked_third_probe_and_large_jump_paths() {
    use pla::assets::tr::TrAnmNativeFramedRotationCache;
    let frames = [0, 0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 110];
    let data = TrAnmChannel {
        encoding: TrAnmEncoding::Framed8,
        keys: frames
            .into_iter()
            .map(|frame| TrAnmKey {
                frame,
                value: pack([16384; 3], 3, false),
            })
            .collect(),
    };
    let context = TrAnmNativeContext::new(0, 1e-12).unwrap();
    let mut cache = TrAnmNativeFramedRotationCache::new(&data).unwrap();
    for jump in [55.0, 95.0] {
        cache.reset();
        cache.evaluate(25.0, context).unwrap();
        cache.evaluate(35.0, context).unwrap();
        let result = cache.evaluate(jump, context).unwrap();
        assert!(result.iter().all(|v| v.is_finite()));
        let state = cache.snapshot();
        let index = u32::from_le_bytes(state[12..16].try_into().unwrap());
        assert_eq!(index, if jump == 55.0 { 4 } else { 1 });
    }
}
