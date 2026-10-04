//! Container parser tests: SARC / GFLXPACK / AHTB / BNTX / VFXB.
//!
//! Fixtures are game content and live outside the repository: see
//! `tests/common/mod.rs` and `REPRODUCE.md`. Missing fixtures skip the test.

mod common;

use common::{fixture, fixture_text};
use pla::assets::{
    ahtb::Ahtb,
    bntx::{Bntx, BntxFormat},
    event_list::EventList,
    gfpak::GfPak,
    message::{MessageEntry, MessageFile, MessageStore},
    sarc::Sarc,
    tr::TrMbf,
    vfxb::Vfxb,
};

/// Minimal SHA-256 (no external dependency) for parity checks.
fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    h.iter().map(|v| format!("{v:08x}")).collect()
}

/// Compare every SARC entry against the Python extractor's expectation file.
fn check_sarc(name: &str, expected_name: &str) -> Option<usize> {
    let bytes = fixture(name)?;
    let expected = fixture_text(expected_name)?;
    let expected = expected.trim();
    let sarc = Sarc::parse(&bytes).expect("SARC parses");
    assert_eq!(sarc.version, 0x0100);
    let mut count = 0;
    for entry in &sarc.entries {
        let entry_name = sarc.name(entry);
        let data = sarc.data(&bytes, entry).expect("entry data");
        let line = format!("{}\t{}\t{}", entry_name, data.len(), sha256_hex(data));
        assert!(
            expected.lines().any(|l| l == line),
            "entry not in python expectation: {line}"
        );
        count += 1;
    }
    assert_eq!(count, expected.lines().count(), "entry count mismatch");
    Some(count)
}

#[test]
fn sarc_matches_python_extraction() {
    let Some(n) = check_sarc("boot_bg.arc", "boot_bg.arc.expected.tsv") else {
        return;
    };
    assert_eq!(n, 1);
    let Some(n) = check_sarc("opapp_01.arc", "opapp_01.arc.expected.tsv") else {
        return;
    };
    assert_eq!(n, 1);
    let Some(n) = check_sarc("titlemenu_00.arc", "titlemenu_00.arc.expected.tsv") else {
        return;
    };
    assert!(n >= 3, "expected several entries, got {n}");
}

#[test]
fn sarc_entry_lookup_by_name() {
    let Some(bytes) = fixture("boot_bg.arc") else {
        return;
    };
    let sarc = Sarc::parse(&bytes).unwrap();
    let entry = sarc.find("blyt/boot_bg.bflyt").expect("named entry");
    let data = sarc.data(&bytes, entry).unwrap();
    assert_eq!(data.len(), 496);
    // .bflyt files start with the FLYT magic
    assert_eq!(&data[0..4], b"FLYT");
}

#[test]
fn gfpak_parses_small_packs() {
    for (name, expected_count) in [("locators.gfpak", 1u32), ("ha_area00_s06.gfpak", 1u32)] {
        let Some(bytes) = fixture(name) else { continue };
        let pak = GfPak::parse(&bytes).expect("gfpak parses");
        assert_eq!(pak.entry_count, expected_count);
        assert!(!pak.entries.is_empty());
        // invariant: data starts right after the entry table
        let table_end = pak.entry_table_offset + pak.entry_count as u64 * 24;
        assert_eq!(pak.entries[0].data_offset as u64, table_end);
        let data = pak.data(&bytes, &pak.entries[0]).expect("payload");
        assert_eq!(data.len(), pak.entries[0].size as usize);
    }
}

#[test]
fn gfpak_locators_payload_has_names() {
    let Some(bytes) = fixture("locators.gfpak") else {
        return;
    };
    let pak = GfPak::parse(&bytes).unwrap();
    let data = pak.data(&bytes, &pak.entries[0]).unwrap();
    let text = String::from_utf8_lossy(data);
    assert!(
        text.contains("lower_head"),
        "expected locator names in payload"
    );
}

#[test]
fn ahtb_flagwork_tables_match_python_parse() {
    // Every `bin/flagwork` AHTB table: Rust parser vs the Python reference
    // (fixtures come from the update build; see REPRODUCE.md).
    for table in [
        "event_flags",
        "event_works",
        "map_flags",
        "map_works",
        "phase_works",
        "system_flags",
        "system_works",
    ] {
        let Some(bytes) = fixture(&format!("{table}.tbl")) else {
            continue;
        };
        let Some(expected) = fixture_text(&format!("{table}.tbl.expected.tsv")) else {
            continue;
        };
        let parsed = Ahtb::parse(&bytes).expect("AHTB parses");
        let expected: Vec<&str> = expected.trim().lines().collect();
        assert_eq!(parsed.len(), expected.len(), "{table}: row count");
        for (entry, line) in parsed.entries.iter().zip(expected.iter()) {
            let (name, id) = line.split_once('\t').expect("name\tid");
            assert_eq!(entry.name, name, "{table}: name");
            assert_eq!(format!("{:016x}", entry.id), id, "{table}: id");
        }
    }
}

#[test]
fn ahtb_rejects_trailing_bytes() {
    let Some(bytes) = fixture("event_flags.tbl") else {
        return;
    };
    let mut bytes = bytes;
    bytes.push(0);
    assert!(Ahtb::parse(&bytes).is_err());
}

#[test]
fn message_file_parses_its_table() {
    let Some(dat) = fixture("bag_pocket.dat") else {
        return;
    };
    let file = MessageFile::parse(&dat).expect("bag_pocket.dat parses");
    assert_eq!(file.version, 1);
    assert_eq!(file.base, 0x10);
    assert_eq!(file.len(), 11, "11 pocket messages");
    // First entry: offset 0x5c relative to the base, 5 UTF-16 units, no flags.
    assert_eq!(
        file.entries[0],
        MessageEntry {
            offset: 0x5c,
            length: 5,
            flags: 0
        }
    );
    let payload = file.payload(&dat, 0).expect("payload");
    assert_eq!(payload.len(), 10);
    // `parse` already enforced that every entry is contiguous and the last one
    // ends exactly at the file end (the parity check over 322 game files).
    let last = file.entries.last().unwrap();
    let end = file.base as usize + last.offset as usize + last.length as usize * 2;
    assert!(end <= dat.len() && dat.len() - end < 4);
}

#[test]
fn message_store_resolves_keys_to_payloads() {
    let (Some(tbl), Some(dat)) = (fixture("bag_pocket.tbl"), fixture("bag_pocket.dat")) else {
        return;
    };
    let store = MessageStore::parse(&tbl, &dat).expect("store");
    assert_eq!(store.count(), 11);
    assert_eq!(store.resolve("str_pocketname_001"), Some(0));
    assert_eq!(store.resolve("str_pocketname_011"), Some(10));
    // The trailing sentinel key has no message.
    assert_eq!(store.resolve("msg_bag_pocket_max"), None);
    assert_eq!(store.resolve("does_not_exist"), None);
    assert_eq!(store.raw(&dat, "str_pocketname_001").unwrap().len(), 10);
}

#[test]
fn bntx_parses_textures() {
    let Some(ball_bytes) = fixture("icon_ball.bntx") else {
        return;
    };
    let ball = Bntx::parse(&ball_bytes).expect("icon_ball parses");
    assert_eq!(ball.texture_count, 1);
    assert_eq!(ball.declared_size as usize, ball_bytes.len());
    let t = ball.texture().expect("texture");
    assert_eq!(t.name, "icon_ball_0000");
    assert_eq!((t.width, t.height), (2, 2));
    assert_eq!(t.format, BntxFormat::Bc7);
    assert_eq!(t.mip_count, 1);
    assert_eq!(t.tile_mode, 0, "optimal (swizzled)");
    // A 2x2 BC7 texture is one 16-byte block, padded to the 0x200 alignment.
    assert_eq!(t.image_size, 0x200);
    let data = ball.data(&ball_bytes);
    assert!(data.len() >= 16, "payload {}", data.len());

    let Some(poke_bytes) = fixture("pokeicon.bntx") else {
        return;
    };
    let poke = Bntx::parse(&poke_bytes).expect("pokeicon parses");
    let t = poke.texture().expect("texture");
    assert_eq!((t.width, t.height), (512, 512));
    assert_eq!(t.format, BntxFormat::Bc7);
    assert_eq!(t.image_size, 512 * 512);
    assert!(poke.data(&poke_bytes).len() > 100_000);
}

#[test]
fn bntx_decodes_a_bc7_icon() {
    let Some(bytes) = fixture("pokeicon.bntx") else {
        return;
    };
    let bntx = Bntx::parse(&bytes).unwrap();
    let data = bntx.decode(&bytes, 0).expect("decode");
    assert_eq!((data.width, data.height), (512, 512));
    assert_eq!(data.rgba.len(), 512 * 512 * 4);

    // The egg icon sits on a transparent background: the dominant pixel is
    // transparent and covers most of the image, while the opaque egg body is a
    // significant minority.
    let mut counts: std::collections::HashMap<[u8; 4], usize> = std::collections::HashMap::new();
    let mut opaque = 0usize;
    for px in data.rgba.chunks(4) {
        *counts.entry([px[0], px[1], px[2], px[3]]).or_default() += 1;
        if px[3] == 255 {
            opaque += 1;
        }
    }
    let total = 512 * 512;
    let dominant = counts.values().copied().max().unwrap();
    assert!(
        dominant * 100 / total > 60,
        "dominant pixel covers only {}%",
        dominant * 100 / total
    );
    let opaque_pct = opaque * 100 / total;
    assert!(
        (10..=40).contains(&opaque_pct),
        "opaque coverage {opaque_pct}% outside the expected 10-40%"
    );
}

#[test]
fn bntx_decodes_the_two_pixel_ball_placeholder() {
    let Some(bytes) = fixture("icon_ball.bntx") else {
        return;
    };
    let bntx = Bntx::parse(&bytes).unwrap();
    let data = bntx.decode(&bytes, 0).expect("decode");
    assert_eq!((data.width, data.height), (2, 2));
    assert_eq!(data.rgba.len(), 16);
    // The placeholder is fully transparent white on all 24 icon_ball files.
    assert_eq!(
        data.rgba,
        vec![255, 255, 255, 0, 255, 255, 255, 0, 255, 255, 255, 0, 255, 255, 255, 0]
    );
}

#[test]
fn bntx_rejects_size_mismatch() {
    let Some(mut bytes) = fixture("icon_ball.bntx") else {
        return;
    };
    bytes.pop();
    assert!(Bntx::parse(&bytes).is_err());
}

#[test]
fn vfxb_embeds_a_parsable_bntx() {
    let Some(bytes) = fixture("particle.ptcl") else {
        return;
    };
    let vfxb = Vfxb::parse(&bytes).expect("VFXB parses");
    assert!(
        vfxb.texture_offset >= 0x1000,
        "offset {}",
        vfxb.texture_offset
    );
    assert_eq!(vfxb.texture.declared_size, vfxb.texture_size);
    let t = vfxb.texture.texture().expect("embedded texture");
    assert!(t.width > 0);
    let (start, end) = vfxb.payload;
    assert!(end > start, "particle payload {start}..{end}");
}

#[test]
fn vfxb_rejects_bad_bom() {
    let Some(mut bytes) = fixture("particle.ptcl") else {
        return;
    };
    bytes[0x0C] = 0;
    assert!(Vfxb::parse(&bytes).is_err());
}

#[test]
fn event_list_parses_the_name_table() {
    let Some(bytes) = fixture("event_list.bin") else {
        return;
    };
    let list = EventList::parse(&bytes).expect("event_list parses");
    assert_eq!(list.version, 12);
    assert_eq!(list.count, 739, "record offsets");
    assert!(
        list.offsets.windows(2).all(|w| w[0] > w[1]),
        "offsets descend"
    );
    assert!(
        list.len() > 1_800,
        "expected the full name pool, got {}",
        list.len()
    );
    for name in [
        "TutorialNPC_area03_001",
        "NsRematchWinExtraAct",
        "DoorEvent",
    ] {
        assert!(list.find(name).is_some(), "missing {name}");
    }
    // The pool is the port's event registry: hashes resolve back to names.
    let hash = pla::save::fnv1a64_str("TutorialNPC_area03_001");
    assert_eq!(list.find_by_hash(hash), Some("TutorialNPC_area03_001"));
}

#[test]
fn bntx_decodes_an_hdr_probe_cubemap() {
    let Some(bytes) = fixture("probe_rg32f.bntx") else {
        return;
    };
    let bntx = Bntx::parse(&bytes).expect("probe parses");
    let t = bntx.texture().expect("texture");
    assert_eq!(t.format, BntxFormat::Rg32Float);
    assert_eq!((t.width, t.height), (32, 32));
    assert_eq!(t.array_count, 6, "a probe cubemap has six faces");
    // Decoding returns face 0, mip 0, tone-mapped to RGBA8.
    let data = bntx.decode(&bytes, 0).expect("decode");
    assert_eq!(data.rgba.len(), 32 * 32 * 4);
    // Face 0, mip 0, clamped-linear tone map. The probe carries tiny irradiance
    // values (0.0011, 0.0078, ...), so the first pixel is (0, 1, 0, 255).
    assert_eq!(&data.rgba[0..4], &[0, 1, 0, 255]);
    assert!(
        data.rgba.chunks(4).all(|p| p[3] == 255),
        "alpha stays opaque"
    );
}

#[test]
fn decodes_real_message_text_through_the_host() {
    let (Some(tbl), Some(dat), Some(ks)) = (
        fixture("bag_pocket.tbl"),
        fixture("bag_pocket.dat"),
        fixture_text("messages.keystream"),
    ) else {
        return;
    };
    let lua = pla::script::new_state();
    pla::host::install(&lua).unwrap();
    let loaded: usize = lua
        .load(
            r#"
            local tbl, dat, ks = ...
            return Global.GetFieldMessageWindowManager()
              :__port_load_messages(tbl, dat, ks)
            "#,
        )
        .call((
            lua.create_string(&tbl).unwrap(),
            lua.create_string(&dat).unwrap(),
            lua.create_string(&ks).unwrap(),
        ))
        .unwrap();
    assert_eq!(loaded, 11);

    let text: Option<String> = lua
        .load(
            r#"
            local m = Global.GetFieldMessageWindowManager()
            return m:GetMessage("str_pocketname_005")
            "#,
        )
        .eval()
        .unwrap();
    assert_eq!(text.as_deref(), Some("Everyday Items"));

    // The window follows the message: pending while one is set, closed after
    // `CloseMessageWindow`, and `SetMessage` returns the decoded text.
    let (shown, open_before, open_after): (Option<String>, bool, bool) = lua
        .load(
            r#"
            local m = Global.GetFieldMessageWindowManager()
            local text = m:SetMessage("str_pocketname_005")
            local before = not m:IsClosedMessageWindow()
            m:CloseMessageWindow()
            return text, before, not m:IsClosedMessageWindow()
            "#,
        )
        .eval()
        .unwrap();
    assert_eq!(shown.as_deref(), Some("Everyday Items"));
    assert!(open_before, "a set message leaves the window open");
    assert!(!open_after, "closing clears it");

    // An unknown key leaves the window alone.
    let unknown: Option<String> = lua
        .load(
            r#"
            local m = Global.GetFieldMessageWindowManager()
            return m:SetMessage("not_a_key")
            "#,
        )
        .eval()
        .unwrap();
    assert_eq!(unknown, None);
}

#[test]
fn trskl_lists_the_bones() {
    let Some(bytes) = fixture("item_228.trskl") else {
        return;
    };
    let skl = pla::assets::tr::TrSkl::parse(&bytes).expect("parse skeleton");
    assert!(
        skl.bones.iter().any(|b| b.starts_with("parts_")),
        "bone names come through: {:?}",
        skl.bones
    );
    assert!(!skl.offsets.is_empty(), "every bone has an offset");
}

#[test]
fn tranm_has_keyframes() {
    let Some(bytes) = fixture("item_228_anim.tranm") else {
        return;
    };
    let anm = pla::assets::tr::TrAnm::parse(&bytes).expect("parse animation");
    assert!(
        anm.keyframes.len() >= 2,
        "a track needs at least two frames"
    );
    assert!(anm
        .keyframes
        .iter()
        .all(|k| k.iter().all(|v| v.is_finite())));
}

#[test]
fn trmbf_parses_the_mesh_buffer() {
    let Some(bytes) = fixture("mesh.trmbf") else {
        return;
    };
    let mesh = TrMbf::parse(&bytes).expect("mesh parses");
    // Every vertex is referenced, so the count is max(index) + 1 = 33 and the
    // payload is 1,188 = 3 x 33 x 12. `arrays` remains a legacy chunk count;
    // without a sibling `.trmsh`, semantic attribute IDs are not attached.
    assert_eq!(mesh.vertex_count, 33);
    assert_eq!(mesh.arrays, 3);
    assert_eq!(
        mesh.vertex_offset, 0x44,
        "the payload starts at the header boundary"
    );
    assert_eq!(mesh.vertex_size, 3 * 33 * 12);
    let p0 = mesh.position(&bytes, 0).expect("position 0");
    assert!(p0.iter().all(|v| v.is_finite()));
    assert_eq!(mesh.triangle_count(), 24, "72 indices");
    assert_eq!(mesh.indices.iter().copied().max(), Some(32));
    assert_eq!(mesh.vertices(&bytes).len(), 3 * 33 * 12);
}

#[test]
fn trmsh_layout_decodes_reference_meshes_when_fixtures_exist() {
    for (stem, expected_count, expected_stride, expected_triangles) in [
        ("item_228", 2046usize, 48usize, 1188usize),
        ("d110_gimmick_rock02_lod1", 200, 36, 232),
        ("ground_area02_cliff01", 1155, 40, 2101),
    ] {
        let (Some(trmbf), Some(trmsh)) = (
            fixture(&format!("{stem}.trmbf")),
            fixture(&format!("{stem}.trmsh")),
        ) else {
            continue;
        };
        let mesh = TrMbf::parse_with_layout(&trmbf, &trmsh).expect("paired mesh layout parses");
        assert_eq!(mesh.vertex_offset, 0x44, "{stem}");
        assert_eq!(mesh.vertex_count, expected_count, "{stem}");
        assert_eq!(mesh.vertex_stride, expected_stride, "{stem}");
        assert_eq!(mesh.triangle_count(), expected_triangles, "{stem}");
        assert_eq!(mesh.attribute_offset(1), Some(0), "{stem} position");
        assert_eq!(mesh.attribute_offset(6), Some(28), "{stem} UV0");
        for vertex in 0..mesh.vertex_count {
            let position = mesh.position(&trmbf, vertex).expect("ID 1 position");
            assert!(position.iter().all(|value| value.is_finite()), "{stem}");
            let uv = mesh.uv(&trmbf, vertex).expect("ID 6 UV0");
            assert!(
                uv.iter()
                    .all(|value| value.is_finite() && (0.0..=1.0).contains(value)),
                "{stem}"
            );
        }
    }
}
