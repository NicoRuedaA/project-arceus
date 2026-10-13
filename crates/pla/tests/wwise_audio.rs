//! Real-data tests for the Wwise readers and the Vorbis path. They skip (`[skip]`) when the
//! game files are not present. No audio is ever played; checks use decoded sample statistics.

use pla::assets::wwise::{opus, vorbis, vorbis_dec, wav, Bank, CodebookLibrary, Codec, Wem};
use std::path::PathBuf;

const GAME: &str = "/home/nico/work/decompilacion/work/pla/pk2";

fn game_file(rel: &str) -> Option<PathBuf> {
    let p = PathBuf::from(GAME).join(rel);
    p.exists().then_some(p)
}

fn library() -> Option<CodebookLibrary> {
    let p = game_file("main.elf")?;
    Some(CodebookLibrary::from_elf_file(p.to_str().unwrap()).expect("codebook library"))
}

fn bank(name: &str) -> Option<(Vec<u8>, Bank)> {
    let p = game_file(&format!(
        "romfs_patched/bin/sound/NX64/wwise/bank/{name}.bnk"
    ))?;
    let bytes = std::fs::read(p).ok()?;
    let b = Bank::parse(&bytes).expect("bank parses");
    Some((bytes, b))
}

#[test]
fn codebook_library_has_598_books_that_all_parse() {
    let Some(lib) = library() else {
        eprintln!("[skip] main.elf not available");
        return;
    };
    assert_eq!(lib.books.len(), 598);
    for (i, b) in lib.books.iter().enumerate() {
        let n = vorbis::packed_book_len(b).unwrap_or_else(|| panic!("book {i} does not parse"));
        assert_eq!(n, b.len(), "book {i}: grammar length vs pointer distance");
    }
}

#[test]
fn ui_bank_census_and_first_vorbis_decode() {
    let (Some(lib), Some((bytes, b))) = (library(), bank("UI")) else {
        eprintln!("[skip] UI.bnk or main.elf not available");
        return;
    };
    assert_eq!(b.version, 135);
    assert_eq!(b.didx.len(), 163);
    let w0 = b.media(&bytes, 0).unwrap();
    let wem = Wem::parse(w0).unwrap();
    assert_eq!(wem.codec, Codec::WwiseVorbis);
    let v = vorbis::convert(w0, &lib).unwrap();
    let pcm = vorbis_dec::decode_all(&v).unwrap();
    // 4 s of 48 kHz stereo, measured declared sample count 192000.
    assert_eq!(pcm.frames(), 192000);
    let st = wav::stats(&pcm.channels, pcm.sample_rate);
    assert_eq!(st.non_finite, 0);
    assert!(st.rms > 0.001 && st.peak < 1.0, "{st:?}");
    // The standard Ogg rewrap starts with an Ogg page and the identification header.
    let ogg = vorbis::to_ogg(&v);
    assert_eq!(&ogg[..4], b"OggS");
    assert!(ogg.windows(7).any(|w| w == b"\x01vorbis"));
}

#[test]
fn twenty_vorbis_files_decode_to_declared_length_without_nan() {
    let (Some(lib), Some((bytes, b))) = (library(), bank("UI")) else {
        eprintln!("[skip] UI.bnk or main.elf not available");
        return;
    };
    let mut ok = 0;
    for i in 0..b.didx.len().min(40) {
        let w = b.media(&bytes, i).unwrap();
        let v = vorbis::convert(w, &lib).unwrap();
        let pcm = vorbis_dec::decode_all(&v).unwrap();
        let st = wav::stats(&pcm.channels, pcm.sample_rate);
        assert_eq!(pcm.frames() as u32, v.sample_count, "media {i}");
        assert_eq!(st.non_finite, 0, "media {i}");
        assert!(st.peak <= 1.0 && st.rms > 0.0, "media {i}: {st:?}");
        ok += 1;
    }
    assert!(ok >= 20);
}

#[test]
fn single_mode_streams_use_one_wire_mode_bit() {
    // Common bank media 1066566035 is a 6 kHz mono stream with a single mode: decoding it
    // yields a real low-frequency signal only when the constant wire mode bit is skipped.
    let (Some(lib), Some((bytes, b))) = (library(), bank("Common")) else {
        eprintln!("[skip] Common.bnk or main.elf not available");
        return;
    };
    let Some(i) = b.didx.iter().position(|e| e.id == 1066566035) else {
        eprintln!("[skip] media not found");
        return;
    };
    let v = vorbis::convert(b.media(&bytes, i).unwrap(), &lib).unwrap();
    assert_eq!(v.mode_blockflags.len(), 1);
    let pcm = vorbis_dec::decode_all(&v).unwrap();
    let st = wav::stats(&pcm.channels, pcm.sample_rate);
    assert!(st.rms > 0.005, "{st:?}");
}

#[test]
fn opus_wem_packets_match_the_seek_table_and_declared_length() {
    let Some((bytes, b)) = bank("Common") else {
        eprintln!("[skip] Common.bnk not available");
        return;
    };
    let mut checked = 0;
    for i in 0..b.didx.len() {
        let w = b.media(&bytes, i).unwrap();
        let Ok(wem) = Wem::parse(w) else { continue };
        if !matches!(wem.codec, Codec::WwiseOpus(_)) {
            continue;
        }
        let o = opus::convert(w).unwrap();
        assert_eq!(o.packets.len(), o.seek_entries, "media {i}");
        let total: u64 = o
            .packets
            .iter()
            .map(|p| opus::packet_samples(p).unwrap() as u64)
            .sum();
        let at_rate = total * o.sample_rate as u64 / 48000;
        assert!(
            at_rate >= o.sample_count as u64,
            "media {i}: {at_rate} < {}",
            o.sample_count
        );
        assert!(at_rate - (o.sample_count as u64) < 2 * 960, "media {i}");
        let ogg = opus::to_ogg(&o);
        if o.channels <= 2 {
            assert_eq!(&ogg.unwrap()[..4], b"OggS");
        }
        checked += 1;
        if checked >= 30 {
            break;
        }
    }
    assert!(checked > 0);
}
