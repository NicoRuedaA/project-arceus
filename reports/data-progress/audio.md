# Audio: Wwise banks, packages and decoding

Tools: `crates/pla/src/assets/wwise.rs` (+ `wwise/*.rs`) and `crates/pla/examples/audio_export.rs` (headless, never opens an audio device or plays anything). Index: [`data/audio_index.tsv`](../../data/audio_index.tsv) (11,970 rows, `sheetty check data` = 0 errors). Decoded output (git-ignored): `work/cache/audio/<bank>/<wem_id>.wav` (16-bit PCM, 5.1 GB) plus `<wem_id>.ogg` (standard Ogg Vorbis rewrap, Vorbis-source media only, 9,805 files, 220 MB). Full run: about 40 s for decoding and WAV writing, about 20 s more for name matching.

```
cargo run --offline --example audio_export --config 'profile.dev.package.pla.opt-level=3' -- \
    export --names --ogg --max-gb 6 --crosscheck 200 --index data/audio_index.tsv
```

Options: `census`, `probe <bank> <wem_id>`, `export [--limit N] [--max-gb G] [--only BANK|Default.pck] [--no-wav] [--ogg] [--threads N] [--crosscheck N] [--names]`. Opus decoding calls the system `libopus` through its C ABI (no crate); the Vorbis decoder is our own pure Rust (`wwise/vorbis_dec.rs`). `ffmpeg` is used only by `--crosscheck`, as an independent decoder.

## Census (Brief 34 numbers reproduced)

| Item | Count |
| --- | --- |
| `.bnk` banks (version 135) | 31 (7 `ENV_BASE_AREA00..06` are 32-byte shells; `Init`, `Dialog`, `ADEMO`, `PM_SKILLS`, `PM_VOICE` carry objects but no media) |
| Media embedded in banks (`DIDX`) | 6,499: 6,332 Vorbis (tag 0xFFFF), 160 Opus (0x3039), 7 PCM (0xFFFE) |
| `Default.pck` streams (`AKPK` v1) | 5,471: 4,653 Vorbis (85.1 %), 566 Opus (10.3 %), 195 tag 0x8311 (3.6 %), 57 PCM (1.0 %) |
| `LooseMedia.pck` | 0 entries (56-byte header only) |
| Index rows / distinct WEM ids | 11,970 / 10,790 (1,180 rows are identical copies of the same id in several banks) |
| HIRC objects, by type | 2: 17,585; 3: 6,976; 4: 5,192; 5: 3,448; 6: 2,167; 7: 513; 8: 81; 9: 22; 10: 389; 11: 381; 12: 64; 13: 193; 14: 185; 16: 58; 17: 52; 18: 4; 21: 2; 1: 575 |

Differences from Brief 34: none in the counts (the sample percentages for the `.pck` were 85 / 11 / 3 / 1; the full table gives 85.1 / 10.3 / 3.6 / 1.0). Additions: only one bank entry is a *prefetch prefix* of a `.pck` file (`BGM.bnk` media 875740768 holds the first 13,008 of 91,088 bytes; the full file is in `Default.pck` under the same id), so `BGM.bnk` really carries almost no music; the music tracks stream from `Default.pck`. The HIRC type numbers were checked against object layouts (see Verified).

## Formats worked out from the data

* **Wwise Vorbis (0xFFFF).** The `fmt` chunk is always 0x42 bytes; the Vorbis tail holds the sample count (+0x18), setup packet offset (+0x28), first audio packet offset (+0x2c) and block-size exponents (+0x40/+0x41). Packets are a `u16` size plus payload. The setup header has no codebooks of its own: it holds 10-bit ids into a **library of packed codebooks that lives in the game executable** (`main.elf`): a table of 599 relative-relocation pointers reaches 598 consecutive packed books (the 599th pointer leads to a non-book byte table). Each book is stored as `bits / 8 + 1` bytes. The packed grammar (4-bit dimensions, 14-bit entry count, 3-bit length width, 1-bit lookup type, ...) was derived from the data and checked on all 598 books (the grammar length equals the pointer distance for every book). The library is game data: it is read from `main.elf` at run time and never stored in git.
* The audio packets lose the packet-type bit and, for long blocks, the previous/next window flags (rebuilt from the following packet's mode). The mode number occupies **at least one bit even when the stream has a single mode** (such streams carry a constant 0 bit there). Without that rule, 319 files (mono 6 kHz and other single-mode streams) decode to silence or to 16x full-scale noise; with it all 10,985 Vorbis files decode cleanly.
* **Wwise Opus (0x3039).** `fmt` is 40 bytes (sample count at +24, seek-table size at +36, 960 samples per frame at +18). The `data` chunk is a seek table (one `u32` per packet) followed by packets stored as `u32` big-endian size, `u32` big-endian Opus final range, and a plain Opus packet (CELT 20 ms, TOC 0xFC, stereo/mono, constant bitrate). The field at +32 is *not* the packet-area size in every file, so it is ignored (the packets run to the end of the chunk). Some files are 16, 24 or 36 kHz Opus and must be decoded at their own rate.
* **PCM (0xFFFE).** Plain 16-bit; copied.
* **0xFFFF/0x3039/0x8311 files may be partial.** Only the single `BGM.bnk` prefetch entry is.

## Decode results (status column of the index)

| Codec | ok | silent | blocked | prefetch |
| --- | ---: | ---: | ---: | ---: |
| Wwise Vorbis | 10,985 | 0 | 0 | 0 |
| Wwise Opus | 716 | 9 | 0 | 1 |
| PCM | 64 | 0 | 0 | 0 |
| tag 0x8311 | 0 | 0 | 195 | 0 |
| **Total** | **11,765** | **9** | **195** | **1** |

11,774 of 11,970 indexed media (98.4 %) decode to 16-bit WAV (a decoded file is a WAV on disk, *not* a proof that it sounds right). The 9 "silent" Opus files are the layered boss/VS tracks (`PLAY_BGM_VS_BOSS_01/02/03/05/MID/CMN`, `PLAY_BGM_VS_NORA`; two have no recovered name) whose every Opus packet is an empty frame followed by zero padding (checked: 3,170 of 3,170 packets in the longest one), so the digital silence is in the source, not a decoder failure.

## Verified (without listening)

* **Duration**: every decoded file has a length within 1 % of the declared sample count (the decoder truncates to the declared count, so a shorter result would have been flagged; none was). Example: `UI.bnk` media 0 declares 192,000 frames at 48 kHz and decodes to exactly 192,000.
* **Signal sanity** over the 11,765 `ok` files: 0 non-finite samples; 33 files peak between 1.0 and 1.144 of full scale (their WAV is peak-normalised to 0.99, recorded in the `reason` column; no file is hard-clipped); median RMS per category between 0.0015 (environment) and 0.062 (other SFX); spectral centroid medians 2.1 to 5.6 kHz (voices 2.5 kHz, BGM 2.6 kHz). Category durations look right: Pokémon voices median 0.79 s (0.20 to 15.7 s), BGM median 24 s (2.3 to 260 s).
* **Independent decoder cross-check** (`--crosscheck 200`, random sample): our Vorbis output written as standard Ogg Vorbis decodes in `ffmpeg` with no warnings; on the common prefix our float PCM matches `ffmpeg`'s with a minimum SNR of 134.7 dB over 200 files (identical to float precision). Frame counts: equal in 131 of 200; `ffmpeg` is 128 or 256 samples shorter or longer in the rest (its own end-of-stream trimming; ours equals the declared count). For Opus (48 kHz files), our libopus decode vs `ffmpeg`'s own Opus decoder: min 38.3 dB, 10th percentile 63.9 dB, median 82 dB over 200 files, equal frame counts. `ffprobe` reports duration 4.000000 s for the first `UI` file.
* **Library correctness**: 598/598 packed books parse and have the length given by the pointer table; all 6,332 + 4,653 Vorbis setups parse to the end (at most 7 padding bits left).
* **HIRC layouts** used for the event map: sound (2) = `plugin u32, stream-type u8, source id u32, size u32, bits u8` + node parameters (parent id at offset 21 when no effects); event (4) = action count + `u32` ids; action (3) = `u16` type + `u32` target; track (11) = `u8, u32 count, 14-byte sources...`. Checked by the parent ids of every sound hitting an existing mixer id and by the extracted source ids being present in `DIDX` / `Default.pck`.
* **Names**: event ids hash with 32-bit FNV-1 of the lower-case name (`PLAY_BGM_HA_FI_CMN_WORMHOLE` and 2,102 more game strings match). `PLAY_PV_%03d_%02d_%02d` (and the upper/lower-case variants) appears as a printf format in `main.elf`. With the game strings plus an expansion of that format over species 1 to 1100, forms 0 to 40 and variants 0 to 99, all 1,707 events of `PM_VOICE` are named; the species numbers in all `PLAY_PV_*` names are exactly the 242 active species of `data/pml_personal.tsv` plus one inactive row, and none outside the table.

## Name mapping coverage

| Measure | Value |
| --- | --- |
| Events (distinct ids) | 4,934 (of 5,192 event objects) |
| Named by a game string (`hash_string`) | 2,103 |
| Named by a template (`hash_template`: `PLAY_PV_...`, `PLAY_PM_[N_]<nnn>_<suffix>` expansions) | 1,515 |
| Events named | 3,618 (73.3 %) |
| Events that resolve to media | 3,963 of 4,934 (the rest only set states, stop sounds or have no media); 223 music events use a child-list scan (heuristic) |
| Media rows with a name | 9,066 of 11,970 (75.7 %): 5,735 `hash_string`, 3,331 `hash_template` |
| Media rows with an event but no name (`bank_only`) | 2,904 |
| Pokémon voice rows (`category=pokemon_voice`) | 3,865, 244 distinct species numbers, 242/242 active species covered |
| BGM rows (`category=bgm`, named) | 214 |

Confidence tags (`name_confidence` column): `hash_string` = the event hash equals the FNV of a string found in game data (a 32-bit hash match: collision probability is negligible for found strings); `hash_template` = the event hash equals the FNV of a generated name (about 10 million candidates were tried, so about a dozen false matches are expected among the 1,515; the 242/242 species check above bounds the PV family, the `PLAY_PM_*_<suffix>` family is unchecked); `bank_only` = category guessed from the bank name only; the `category` column of such rows is a guess.

## Assumed

* `PV` means "Pokémon voice" and the fields after the species are form and variant; the meaning of the variant (cry, damage, faint, ...) is not proven. Some PV events carry textual suffixes (`PLAY_PV_059_DAMAGE`, `..._FIND_RND`, `..._THREAT_RND`) which hint at moods.
* An event lists *every* media reachable below its target (all random/switch variants); the mapping ignores switch, state and RTPC conditions, so a media can be listed for an event it only plays under some state.
* Music segments/playlists/switches: children found by a scan for plausible id lists (223 events flagged heuristic).
* The Opus pre-skip is set to 0 in the Ogg Opus rewrap (the real value is unknown; the declared sample count trims the end).
* `category` for named rows comes from the name prefix (`PLAY_BGM` = bgm, `PLAY_UI` = ui, `PLAY_PV`/`VOICE` = voice, `PLAY_PM` = pokemon_sfx, `PLAY_BA` = battle, `PLAY_PL` = player, `PLAY_ENV` = environment): a naming-convention reading.

## Unknown

* **Codec tag 0x8311 (195 files, all in `Default.pck`, mono, 48 kHz in 184 of them).** Evidence: `fmt` is 24 bytes (tag 0x8311, 1 channel, `avg_bytes_per_sec` 27,000 at 48 kHz, `block_align` 36, 4 bits per sample, 6 extension bytes `00 00 01 41 00 00`); every `data` size is a multiple of 36 (195/195); 27,000 / 36 = 750 blocks per second, so 64 samples per block at 48 kHz; each 36-byte block starts with a 4-byte header of two 16-bit-like values that stay close to each other, followed by 32 bytes of nibbles; 160 of 195 files start with an all-zero header block. That is the layout of a 2-tap 4-bit ADPCM with a 64-sample block, and the tag matches Platinum-style ADPCM from memory of other tools, but the prediction coefficients and the header bit layout could not be proven from the data, so **no decoder was written** and the codec stays *unidentified*. 159 of the 195 rows carry a hash-matched name (e.g. `PLAY_EW0071_02`, field/ambient emotes).
* Loop points (`smpl` / `cue ` chunks in WEM files) are not parsed; BGM layering and transitions (`bin/sound/table/bgm_layer.bin`, `bgm_transition_*.bin`) are not decoded; area-to-BGM binding is unknown.
* 1,316 events (26.7 %) have no recovered name; the HIRC objects other than sound, action, event, containers, segments and tracks (buses, states, switches, RTPCs) are not read.
* Whether the 33 slightly-over-full-scale files (peak up to 1.144) are over-driven in the source or a decoder scale is a few per cent off is unknown (the decoder matches `ffmpeg` to 134.7 dB on the 200 sampled files, so a decoder-wide scale error is excluded, but those 33 files were not individually cross-checked).
* How the Switch plays these (voice limits, streaming from the package) is out of scope.

## Playback in the Bevy port

No audio crate (`bevy_audio`, `rodio`, `cpal`, `lewton`, `hound`) exists offline. Plan: load `work/cache/audio/<bank>/<wem_id>.wav` (16-bit PCM; Bevy's WAV loader, no extra crate needed once `bevy_audio` is available) through the index: look up `entry` (`<bank>/<wem_id>`) or an event name in `data/audio_index.tsv` (`name`, `category`, `species`). For big music tracks the `.ogg` next to the WAV (Vorbis-source media only) is 23 times smaller (220 MB vs 5.1 GB overall); Opus-source tracks (716 + 9 files, mostly music in `Default.pck`) have WAV only because Bevy cannot read Opus. The maintainer should listen to a few: a cry (`PLAY_PV_025_00_00`), a UI sound and one BGM track, and check loops by ear (loop points are not applied).

## Tests

`cargo test --offline -p pla wwise` (unit: bit reader/writer, packed-book grammar, `lookup1_values`, IMDCT against the direct formula, float32 unpack, Ogg CRC table, TOC durations, WAV header, spectral centroid, bank/HIRC parsers) and `crates/pla/tests/wwise_audio.rs` (real files, `[skip]` when the game files are missing: 598 books, the `UI` bank census, 40 `UI` Vorbis files decoding to their declared length without NaN, the single-mode rule, Opus packet counts against the seek table).
