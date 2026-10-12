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

The side `.tbl` has 725 keys and 724 message records, including 149 `questname_sub` keys (115 IDs in 1–122 and 34 above 122). **28** of those stored IDs occur somewhere in `quest_list_sub.bin`; a bounded byte-level check found all 28 at target offset `+60` for root-vector indices 94–121, and none at `+60` for indices 0–93. This is a locator only: side target layouts are not validated, so those bytes do not establish side-row IDs. The local vectors contain 27 main and 122 side entries, while message inventory suggests 28 main and 94 request IDs; `122 - 94 = 28` and the tail-byte hits do not prove which 94 entries are requests or identify any side row. The older `28 + 94` claim is therefore not reconciled to row identities. The only proven quest-row identities are the 27 main title-key joins.

**Falsifiable checks:** every rerun must produce 27 distinct raw-index-04-to-AHTB-ID joins; a missing, duplicate, or changed key falsifies the field mapping. Re-scan the side binary and verify exactly 28 `questname_sub` AHTB IDs at target `+60` for indices 94–121 and zero at that offset for 0–93; any difference falsifies this locator observation, but even a repeat does not validate record layouts. Any side-list semantic promotion requires a decoded title/message ID at a specific validated vector target, not merely elsewhere in the file.

### Encounters and outbreaks

- `poke_encount.bin`: **322/322** F00 values are in the active species-ID set from `data/pml_personal.tsv` (**242 distinct IDs**). This validates the species-ID interpretation in `poke_encount_fields.tsv`. F01 is absent in all records; because 26 species have multiple active forms, only **303/322** rows match `(species, form=0)`, so the absent form/default semantics remain unknown. F02–F13 are float32 slots (observed −1.0 to 1.5); exact time/weather meaning and ordering remain unknown, and the community schema's asset binding is unproven. Falsify the species candidate by rechecking every F00 against the active ID set; a non-match disproves it.
- `poke_event_encount.bin`: **251** outer records have F00/F01/F03 relative-offset fields; F02 is absent. All 251 F00 strings are unique encounter identifiers; F01 is empty throughout. F03 points to `EventEncountPoke` vectors containing **298** Pokémon records. Schema slots F00/F01/F04 decode as Species/Form/Level; all **298/298** `(species, form)` pairs match active `pml_personal.tsv` rows (**89 distinct pairs**), and levels span **12–100** (**23 distinct values**). Reparse those exact slots and require zero unmatched pairs and the same 23-value level set to falsify/reconfirm. `Field_01`'s meaning and human-readable encounter/area labels remain unknown.
- `huge_outbreak.bin`: all **5/5** F00 hashes match corresponding rows in `new_huge_outbreak_lottery.bin`; **0/5** match the 450 event-flag hashes directly. This supports cross-table identity only, not the hash's label or gameplay meaning. Repeat both joins to falsify/reconfirm. Signedness, target pointers, and nested values remain unknown.
- New-outbreak group exports contain 166 parent/480 child records and 7 group-lottery parent/501 child records; species/move meanings remain unknown. The five mass-outbreak `WorkValueName` strings are `WSYS_MASS_GANERATION_AREA01` through `AREA05`; these are area/work-value identifiers, not verified human-readable region names. Re-decode the five string targets to falsify the exact identifier set. `field_huge_outbreak.bin` remains contradicted/unusable for semantic assignment because the first table has an invalid vtable delta (−99,472).

### Items, item hashes, shops, and crafting

- English `itemname.dat` decodes to **1,829 entries** (indices 0–1,828; 1,183 unique visible strings). The hash table has **1,830** `<u64,u64>` records. An expanded candidate pass generated 38,411 visible-name case/path/prefix/suffix/encoding variants and tested FNV-1/FNV-1a at 32 and 64 bits: **0 hash intersections**. It also tested FNV-1a32/64 over all 1,830 AHTB item-name keys and 39,492 numeric/internal-ID spellings such as `item_001` and `pml/item/item_001`: **0 intersections and 0 exact `(hash,index)` joins**. These falsify the tested direct-name/key/path and numeric-ID hypotheses; the actual key derivation remains unknown. Re-run the same candidate generator against all 1,830 stored hashes to falsify this result.
- The hash table's second u64 is zero in **1,023** records. The other 807 rows contain 806 distinct values in 1–1,828; ID 1,828 occurs twice, and **1,022** candidate name indices in 1–1,828 are absent. This contradicts a simple one-to-one index map over the full name table. Hash/key semantics remain unknown; do not rename based on the numeric range.
- `item.dat` exports 807 records; crafting has 43 recipe records (IDs 1–43, result quantity 1; ingredient slots 1/2/3/4 populated in 43/41/15/7 records); shop data has 143 rows. Shop F02 candidate item IDs span 17–1,754 (105 distinct), and 24 distinct values overlap recipe result IDs. Those range/overlap counts alone do not prove item-name joins or recipe/shop semantics. No field in item, shop, or craft tables was promoted in this pass beyond existing structural evidence.

## Schema changes

- Main quest `raw_index_04` remains verified as `QuestNameMessageKeyID`; `poke_encount` F00 remains verified as a species ID; three nested event-encounter fields (`Species`, `Form`, `Level`) are now verified against the pinned schema and independent PML checks, with exact counts and falsifiers above.
- The mapping summary now totals **94 verified, 68 unverified, and 5 contradicted** across 22 TSVs / 167 fields. Rename proposals retain the two existing evidence-backed candidates; no item-hash or side-quest rename is justified.
- Side-list record semantics and the 28+94 identity split, trigger fields, `poke_encount` form/weather meanings, encounter area labels, outbreak meanings beyond existing identity joins, item-hash key derivation, and most item/shop/craft semantics remain unknown. Event Pokémon Species/Form/Level and their independent PML pair/range checks are now recorded as verified schema-field mappings, not behavioral verification. No function evidence ledger/profile was regenerated or modified.
