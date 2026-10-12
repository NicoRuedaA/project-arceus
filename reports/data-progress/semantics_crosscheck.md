# Data Semantics Cross-check — Update v262144

**Scope:** quest lists/triggers, encounter and outbreak tables, and item/hash/shop/craft tables. Cross-checks use decoded English messages, exported data, and independent local tables only. No emulator or Ghidra process was used. Community schema labels remain hints unless an independent check supports them.

## Results

### Main and side quest lists

`quest_catalog.tsv` contains 27 main and 122 side vector entries. The 27 main rows each have an 8-byte `raw_index_04_value_hex` at object offset `+60`. Interpreting it as a little-endian u64 yields **27/27 unique direct matches** to stored AHTB IDs for `msg_questlist_main_questname_*` keys in `English/common/questlist_main.tbl`. None of raw indices 00–03 or 05–12 joins these IDs. This proves raw index 04 is the main row's quest-name message-key ID; it does not prove a native reader binding.

Reproduce the join from the repository root (set `ROM` to the supplied `romfs_patched` directory):

```python
import csv, struct
from pathlib import Path

root = Path("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin")
b = (root / "message/English/common/questlist_main.tbl").read_bytes()
n, pos = struct.unpack_from("<I", b, 4)[0], 8
by_id = {}
for _ in range(n):
    key_id = struct.unpack_from("<Q", b, pos)[0]
    size = struct.unpack_from("<H", b, pos + 8)[0]
    pos += 10
    by_id[key_id] = b[pos:pos + size - 1].decode()
    pos += size

with open("data/quest/quest_catalog.tsv") as f:
    rows = list(csv.DictReader((x for x in f if not x.startswith("#")), delimiter="\t"))
main = [r for r in rows if r["collection:enum"] == "main"]
matches = [by_id.get(int.from_bytes(bytes.fromhex(r["raw_index_04_value_hex:string?"]), "little")) for r in main]
assert len(matches) == 27 and all(matches) and len(set(matches)) == 27
print(matches)
```

The 27 joined titles are *In an Unfamiliar Land*, *The Galaxy Team's Entry Trial*, *The Basics of Crafting*, *Getting to Work on Research Tasks*, *A Request from Mai*, *Summoned by Commander Kamado*, *The Frenzy of the Lord of the Woods*, *Arezu's Predicament*, *A New Mission*, *The Lordless Island*, *Scaling Perilous Heights*, *The Slumbering Lord of the Tundra*, *Disaster Looming*, *The Trial of Lake Verity*, *The Trial of Lake Valor*, *The Trial of Lake Acuity*, *Atop Mount Coronet*, *The Counterpart*, *A New Day Dawns*, *The Researcher of Myths*, *The Plate of the Lakes*, *The Trial of Firespit Island*, *The Trial of Moonview Arena*, *The Trial of Snowpoint Temple*, *The Trial of Prelude Beach*, *Seeking the Remaining Plates*, and *The Deified Pokémon*. English message text can be decoded with `python3 .tools/message_cipher.py "$ROM/bin/message/English" --index <index> --dump`.

**Count discrepancy:** the main `.tbl` has 31 quest-name keys, of which 27 join to list rows. Four are unreferenced: `0_1`, `1_2`, `1_6`, and `7_2`. `7_2` decodes as *Seeking the Red Chain*, a plausible additional main mission but **not a row in the 27-entry vector**; the other three remain placeholder/control-like and unclassified. Thus 27 main rows are proven, while 28 main missions is only a plausible inventory count.

The side `.tbl` has 725 keys and 724 message records, including 149 `questname_sub` keys (115 IDs in 1–122 and 34 above 122). **28** of those stored IDs occur somewhere in `quest_list_sub.bin`, but none are at any of the 122 root-vector targets. The target records remain opaque. The numeric `122 - 94 = 28` difference and the 28 embedded IDs do not prove which 94 entries are requests or identify any side row. Therefore the older `28 + 94` claim is not reconciled to row identities. The only proven quest-row identities are the 27 main title-key joins.

**Falsifiable checks:** every rerun must produce 27 distinct raw-index-04-to-AHTB-ID joins; a missing, duplicate, or changed key falsifies the field mapping. Any side-list promotion requires a title/message ID at a specific vector target, not merely elsewhere in the file.

### Encounters and outbreaks

- `poke_encount.bin`: **322/322** F00 values are in the active species-ID set from `data/pml_personal.tsv` (**242 distinct IDs**). This validates the `Species`/species-ID interpretation in `poke_encount_fields.tsv`. Recheck membership against the active-ID set; a non-match falsifies it. F01 is absent in all records. F02–F13 are float32 slots (observed −1.0 to 1.5); exact time/weather meaning and ordering remain unknown, and the community schema's asset binding is unproven.
- `poke_event_encount.bin`: **251 records** have four-byte relative-offset slots F00, F01, and F03; F02 is absent throughout. Pointed-to strings/vector contents were not independently decoded, so nested species, level, and event semantics remain unknown.
- `huge_outbreak.bin`: all **5/5** F00 hashes match corresponding rows in `new_huge_outbreak_lottery.bin`; **0/5** match the 450 event-flag hashes directly. This supports cross-table identity only, not the hash's label or gameplay meaning. Repeat both joins to falsify/reconfirm. Signedness, target pointers, and nested values remain unknown.
- New-outbreak group exports contain 166 parent/480 child records and 7 group-lottery parent/501 child records; this pass did not establish species or move field semantics. `field_huge_outbreak.bin` remains contradicted/unusable for semantic assignment because the first table has an invalid vtable delta (−99,472).

### Items, item hashes, shops, and crafting

- English `itemname.dat` decodes to **1,829 entries** (indices 0–1,828; index 0 is `None`). The hash table has **1,830** `<u64,u64>` records. FNV-1a64 of visible English names yielded **0 hash-only and 0 `(hash,index)` exact matches** for UTF-8, lowercase/uppercase UTF-8, UTF-16LE, and lowercase UTF-16LE. This falsifies direct hashing of those visible names; it does not establish what the hash represents.
- The hash table's second u64 is zero in **1,023** records. The other 807 rows contain 806 distinct values in 1–1,828; ID 1,828 occurs twice, and **1,022** candidate name indices in 1–1,828 are absent. This contradicts a simple one-to-one index map over the full name table. Hash/key semantics remain unknown; do not rename based on the numeric range.
- `item.dat` exports 807 records; crafting has 43 recipe records (IDs 1–43, result quantity 1; ingredient slots 1/2/3/4 populated in 43/41/15/7 records); shop data has 143 rows. Shop F02 candidate item IDs span 17–1,754 (105 distinct), and 24 distinct values overlap recipe result IDs. Those range/overlap counts alone do not prove item-name joins or recipe/shop semantics. No field in item, shop, or craft tables was promoted in this pass beyond existing structural evidence.

## Schema changes

- Promoted only main quest `raw_index_04` to verified `QuestNameMessageKeyID` and `poke_encount` F00 to verified `Species`, with the exact counts and falsifiers above.
- The mapping summary now totals **91 verified, 68 unverified, and 5 contradicted** across 22 TSVs / 164 fields. Rename proposals add the two evidence-backed candidates.
- Side-list fields, trigger fields, event-encounter nested values, outbreak meanings beyond the existing identity join, visible-name item hashes, and most item/shop/craft semantics remain unknown. No function evidence ledger/profile was regenerated or modified.
