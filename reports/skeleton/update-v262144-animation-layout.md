# Read named animation records, not heuristic float runs

`TrAnm::parse_tracks` now decodes bounded FlatBuffer timing and named skeletal
scale/rotation/translation channels. It returns typed records with declared
frame indices, preserving zeros and duplicates. Rotation values are **three
unsigned packed 16-bit words**, not Euler triples or reconstructed quaternions.
This is record decoding, NOT native interpolation, playback or runtime skinning.

The old `parse` remains a documented compatibility alias for `parse_heuristic`;
its longest plausible-float run has no verified frame/channel identity. New
validated decoding never routes through that heuristic. This corrects the old
`.tranm` header-size and float-triple claims in dec043/dec047 and source docs;
historical findings are preserved with correction markers.

## Proven record lane

The first word is a root-table uoffset, not a header size. The observed root
contains Info and BoneAnimation only. Info carries loop flag, frame count and
integer frame rate. BoneAnimation contains named tracks; each track has separate
S/R/T union tags and table references.

| Tag | Encoding | Frame indices | Values |
|---:|---|---|---|
| 1 | Fixed | one implicit record at 0 | inline 3×f32 vector or 3×u16 packed rotation |
| 2 | Dense | implicit 0..frame_count−1 | contiguous struct vector |
| 3 | Framed16 | explicit u16 vector | matching contiguous struct vector |
| 4 | Framed8 | explicit u8 vector | matching contiguous struct vector |

All four record formats occur in the corpus. “Fixed at 0” describes stored
records, NOT hold/extrapolation behavior. Dense lengths exactly equal declared
frame count in all 1565 dense channels. Sparse/intermediate-frame evaluation,
loop endpoints and missing-channel/default semantics are not implemented.

## Independent primary references

Pinned public [animation reader directory](https://github.com/ChicoEevee/Pokemon-Switch-Model-Importer-Blender/tree/b0c98d9fcaab85a04ad35e2d111bae4cad6c1e04/GFLib/Anim)
provides generated field getters, union tags and struct widths. The independent
[importer](https://github.com/ChicoEevee/Pokemon-Switch-Model-Importer-Blender/blob/b0c98d9fcaab85a04ad35e2d111bae4cad6c1e04/gfbanm_importer.py)
uses these named channels and explicit frame indices; the
[exporter](https://github.com/ChicoEevee/Pokemon-Switch-Model-Importer-Blender/blob/b0c98d9fcaab85a04ad35e2d111bae4cad6c1e04/gfbanm_exporter.py#L54-L56)
corroborates count/rate/loop roles. The census JSON records SHA-256 for all 18
selected primary files, including:

| Source | SHA-256 |
|---|---|
| `Animation.py` | `324ea935e9dbef290eb217acc12c37d1f34ff322c75ab4b36aa7d9a7be94a575` |
| `Info.py` | `6429e64f365b6ec1a0e32d93fe55fde522742bf6842017285b947feba6d6608e` |
| `BoneTrack.py` | `9ee0c0a91ed235a74e2d7b512a5af6850e7f50556bfe3d9a7a5c28399fb23e1a` |
| `gfbanm_importer.py` | `b836fe617f3123a7ff5158a4f5ebab7e44186905ea3b698e19de136a3a1899fd` |

The pinned pkNX tree has no animation schema; it is NOT claimed as corroboration
for these animation fields. Generated reference modules were inspected, not
executed: their FlatBuffers Python runtime is unavailable and was not installed.
The independent stdlib Python census separately traverses/validates the files;
Rust is compared against its canonical typed-record checksums. Neither that
agreement nor public importer code proves native packed-quaternion reconstruction.

## Actual coverage, not inventory inference

All **1522 standalone inputs** were actually traversed, hashed and record-decoded,
then compared with Rust: **25394 named tracks / 1868293 records**, 19544648 input
bytes. There are zero unsupported record variants among these inputs, NOT proof
that archive-embedded/unseen formats or all animation semantics are supported.
All frame rates are 60; frame counts range 21–1461; loop flags are 0 in 1337
files and 1 in 185. 1518 files contain varying
records, four contain only fixed/static records.

| Channel | Fixed channels / records | Dense channels / records | Framed16 channels / records | Framed8 channels / records |
|---|---:|---:|---:|---:|
| Scale | 25195 / 25195 | 0 / 0 | 157 / 3464 | 42 / 620 |
| Packed rotation | 10897 / 10897 | 1200 / 203338 | 6674 / 736829 | 6623 / 313079 |
| Translation | 16848 / 16848 | 365 / 77401 | 3756 / 306712 | 4425 / 173910 |

**All 21677 framed channels repeat identical first/last records** at indices
0 and frame_count−1, with two duplicate adjacencies each. No descending or
out-of-range indices were observed. Every repeated record is retained. The
public importer selects a first occurrence; that does NOT establish native
interpolation or justify deleting duplicates.

| Nontrivial independent input | Frames / tracks | Demonstrated varying lane | SHA-256 |
|---|---:|---|---|
| `d020_item_224_c002.tranm` | 241 / 3 | dense rotation and translation | `1c9c8e855af7cf32aeab7fd36a2b88c2052d7cb6e680027739925d5ae00cc12a` |
| `d020_item_224_c011.tranm` | 211 / 5 | framed8 rotation and translation | `8b4ed07a2679cdfb38ade1069663dd2915f40beb0415567f003dfbb1106c5f82` |
| `sd9150_misterygift_anim.tranm` | 481 / 10 | framed16 rotation and translation | `c7107bcac30767b131ce4671d27c371eedca3a43960995bb8db2eed862631f15` |

Name subsets yield **6 unique candidates within the 199-file skeleton census**,
27 ambiguous and 1489 unresolved. This is name compatibility, not a native asset
link. Explicit `skeleton_nodes` resolution validates each name against a selected
skeleton and fails on ambiguity/missing names; it never guesses rig indices.
The mysterygift case maps all ten names to the corresponding skeleton (SHA-256
`327dd5a826a82356a6f77fa08859780d4d3afbb068ab8c4c3524cd502061bc54`).
Four locator cases and one broken-building case are the other unique candidates;
all paths/hashes are in the metadata census.

## Boundaries, reproduction and checks

Inputs are capped at 32 MiB, tracks at 8192, names at 1024 bytes and aggregate key
records/frame count at one million. Integer frame rates are bounded to 1..1000;
only 60 is observed. Signed shared vtables, relative offsets, field/vector
extents and alignment are checked, including two-byte packed rotation structs.
Dense lengths, framed lengths, finite vectors, ordered/in-range indices and
unique track names are checked before output. Tags 0/NONE or unknown tags,
material/visibility/event chunks and InitData are explicitly unsupported.

```sh
python3 .tools/tranm_census.py "$PRIVATE_ROMFS" \
  --output reports/skeleton/update-v262144-animation-census.json
PLA_ANIMATION_CORPUS="$PRIVATE_ROMFS" cargo test -p pla --test tr_animation -- --nocapture
./.tools/ci.sh
```

JSON/TSV contain paths/hashes, timing, counts and metadata fingerprints only;
no game keyframe values, packed words, quaternions or sample bytes. Canonical
FNV-1a-64 typed-record checksums are differential metadata, not cryptographic
security or behavioral parity. Exact input SHA-256 hashes are reconciled
separately. Optional tests without the private environment skip and supply no
corpus evidence. Build identity derives from prior verified extraction evidence,
not directory naming.

Five tests cover author-created fixed/dense/framed-u16/framed-u8 buffers,
zero values, duplicate records, frames above 255, explicit name mapping,
malformed timing/tags/lengths/indices/values, extra chunks, truncations, byte
mutations and allocation limits. The opted-in full-corpus test compares every
record checksum, all union counts and six unique name mappings. After source
normalization, explicit private animation/matrix/hierarchy/skin suites passed
**20/20, EXIT 0** (animation 5, matrix 5, hierarchy 4, skin 6). Independent
metadata census **EXIT 0**; all 1522 animation input hashes, six selected mapping
references and 18 pinned public source hashes reconciled. Final `./.tools/ci.sh`:
**EXIT 0**, 89 passed, 2 ignored; preflight 0 errors/0 warnings. Its unconfigured
optional corpus tests skip and are not the explicit corpus proof above. Python
syntax, check-only formatting and `git diff --check` passed. Final single
function-progress regeneration is checked independently against exact build,
archive, current Rust fingerprint and all output hashes. Native/game playback runtime: **not
exercised**; only parsers/differential checks execute. No direct native function
address is mapped, and the function evidence ledger receives no state promotion.

## Rollback and next step

Rollback dec098's animation module/re-export, explicit tests/census/reports and
sheet/PLAN projections; retain dec094–097 and matrix counterexample evidence.
Regenerate function-progress reports after source fingerprint changes.

Next bounded unit: independently corroborate packed-rotation reconstruction and
framed evaluation, retaining duplicate-record evidence and explicit first
mismatch. Native timeline/loop semantics, item_230 scaled binds, nonzero pivots,
asset links, animation application and Bevy skinning remain unresolved. Priority
2 and the full decompilation goal remain incomplete.
