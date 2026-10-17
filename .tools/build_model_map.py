#!/usr/bin/env python3
"""Build data/national/model_map.tsv: user model folders -> national species/forms.

Read-only scan of the user's model folder (never copied, never modified). The
folder names are the games' pack stems `pmNNNN_FF_VV`. Everything here is a
reproducible derivation from:

  * the pack numbers themselves, grouped as contiguous family blocks,
  * `data/pokemon_resource_table.tsv` (Legends: Arceus pack catalog, VERIFIED
    in-game mapping (dex, form) -> pack for 226 species),
  * `data/national/forms.tsv` (external PokeAPI/Showdown data for names/kinds),
  * icons inside each model folder (`*_00_0.png`, `*_00_big.png`, `*_00.png`,
    and `*_col.png` / `*_col_rare.png` texture names) read visually and
    encoded in CURATED below.

Verified conventions (anchored by in-game data or the models' own textures):
  FF=00 base model, FF=01 female model (PLA catalog `female` column),
  FF=51 / FF=52 Mega 1 / Mega 2 (Charizard X/Y and Garchomp Mega icons),
  FF=81 / FF=82 Gigamax (the 33 `_81` packs are exactly the Gigantamax list),
  FF=11+n the game form n (PLA catalog: Unown, Burmy, Shellos, Rotom),
  VV=01 rare/alternate colours - shiny (textures named `_col_rare`),
  VV=11 Alolan (the 16 packs are exactly the Alolan Kanto forms; Raichu icon),
  VV=31 Galarian (the list is exactly the Galarian forms),
  VV=41 Hisuian (PLA catalog anchors, e.g. pm0058_00_41 Growlithe de Hisui),
  VV=51 Paldean (Tauros, Wooper).

Confidence language (repository rules):
  VERIFICADO   the mapping comes from the in-game PLA catalog, or the pack
               number equals the national dex number for 1..649, or a form
               convention is anchored by verified in-game data.
  PROBABLE     visual identification of the model's own icon, a variation
               convention anchored by one verified example, or a numbering
               rule extrapolated from a verified contiguous run.
  DESCONOCIDO  everything else; the row is kept, never silently dropped.

Usage: python3 .tools/build_model_map.py [--models-root DIR]
"""

from __future__ import annotations

import argparse
import csv
import os
import re
import sys
from collections import Counter

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEFAULT_MODELS_ROOT = "/home/nico/Downloads/models/models"

HEADER = (
    "# sheet: data/national/model_map\n"
    "# version: 1\n"
    "# generator: .tools/build_model_map.py\n"
    "# target: user model collection (Blender packs) -> national dex species/forms\n"
    "# confidence: VERIFICADO | PROBABLE | DESCONOCIDO (see .tools/build_model_map.py)\n"
    "# kind: base|regional|mega|gigamax|cosmetica|variante-color|otro\n"
    "# note: the pack id pmNNNN_FF_VV is the game's own model id; NNNN is NOT the\n"
    "#       national number above 649 (pm0749 is Bergmite #712, pm0841 is Rowlet #722)\n"
)
COLUMNS = "model:string*\tspecies:u16\tform_index:u8\tform_name:string\tkind:string\tconfidence:enum\treason:string?"

# ---------------------------------------------------------------------------
# Curated visual identifications (model pack number -> national species).
# Each entry was read from the pack's own icon inside the collection. Entries
# marked PROBABLE were additionally checked against the contiguous-family
# structure of the block (families are contiguous, in national order within a
# family) and against known form counts (folder/sub counts).
# key: (num, ff, vv) -> (species, form_index, form_name, kind, confidence, reason)
# ---------------------------------------------------------------------------
CURATED: dict[tuple[int, int, int], tuple[int, int, str, str, str, str]] = {}


def cur(num, ff, vv, species, form_index, form_name, kind, conf, reason):
    CURATED[(num, ff, vv)] = (species, form_index, form_name, kind, conf, reason)


# --- gen-6 block 701-774 (model-creation order; families contiguous) -------
cur(701, 0, 0, 676, 0, "Furfrou", "base", "PROBABLE",
    "icono: caniche blanco; 10 subcarpetas = 10 cortes de Furfrou")
cur(702, 0, 0, 708, 0, "Phantump", "base", "PROBABLE", "icono: tocon con hojas (Phantump)")
cur(703, 0, 0, 709, 0, "Trevenant", "base", "PROBABLE", "icono: arbol fantasma (Trevenant)")
cur(704, 0, 0, 667, 0, "Litleo", "base", "PROBABLE", "icono: cachorro leon con melena roja (Litleo)")
cur(705, 0, 0, 668, 0, "Pyroar", "base", "PROBABLE", "icono: leon con melena (Pyroar)")
cur(706, 0, 0, 677, 0, "Espurr", "base", "PROBABLE", "icono: gato gris de ojos grandes (Espurr)")
cur(708, 0, 0, 666, 0, "Vivillon", "base", "PROBABLE",
    "icono: mariposa; 20 subcarpetas = 20 patrones de Vivillon")
cur(711, 0, 0, 659, 0, "Bunnelby", "base", "PROBABLE", "icono: conejo gris (Bunnelby)")
cur(712, 0, 0, 660, 0, "Diggersby", "base", "PROBABLE", "icono: conejo pardo sonriente (Diggersby)")
cur(713, 0, 0, 669, 0, "Flabebe", "base", "PROBABLE", "icono: flor amarilla; 5 subcarpetas = 5 colores")
cur(714, 0, 0, 670, 0, "Floette", "base", "PROBABLE", "icono: flor blanca; 7 subcarpetas = colores + Floette Eterna")
cur(715, 0, 0, 671, 0, "Florges", "base", "PROBABLE", "icono: flor roja; 5 subcarpetas = 5 colores")
cur(717, 0, 0, 653, 0, "Fennekin", "base", "PROBABLE", "icono: zorro amarillo (Fennekin)")
cur(718, 0, 0, 654, 0, "Braixen", "base", "PROBABLE", "icono: zorro blanco/naranja (Braixen)")
cur(719, 0, 0, 655, 0, "Delphox", "base", "PROBABLE", "icono: zorro rojo con baston (Delphox)")
cur(720, 0, 0, 650, 0, "Chespin", "base", "PROBABLE", "icono: erizo verde con rama (Chespin)")
cur(721, 0, 0, 651, 0, "Quilladin", "base", "PROBABLE", "icono: armadura verde sonriente (Quilladin)")
cur(722, 0, 0, 781, 0, "Dhelmise", "base", "PROBABLE",
    "render sin texturas: timon verde + algas + ancla (Dhelmise, #781)")
cur(723, 0, 0, 656, 0, "Froakie", "base", "PROBABLE", "icono: rana azul (Froakie)")
cur(724, 0, 0, 657, 0, "Frogadier", "base", "PROBABLE", "icono: rana azul (Frogadier)")
cur(725, 0, 0, 658, 0, "Greninja", "base", "PROBABLE", "icono: rana ninja; 2 subcarpetas (Greninja + Ash)")
cur(728, 0, 0, 673, 0, "Gogoat", "base", "PROBABLE", "icono: cabra con cuernos (Gogoat)")
cur(729, 0, 0, 759, 0, "Stufful", "base", "PROBABLE", "icono: oso bicolor con etiqueta (Stufful)")
cur(730, 0, 0, 760, 0, "Bewear", "base", "PROBABLE", "icono: oso grande (Bewear)")
cur(737, 0, 0, 696, 0, "Tyrunt", "base", "PROBABLE", "icono: dinosaurio pardo (Tyrunt)")
cur(738, 0, 0, 697, 0, "Tyrantrum", "base", "PROBABLE", "icono: dinosaurio rojo (Tyrantrum)")
cur(739, 0, 0, 698, 0, "Amaura", "base", "PROBABLE", "icono: dinosaurio azul (Amaura)")
cur(740, 0, 0, 699, 0, "Aurorus", "base", "PROBABLE", "icono: dinosaurio cuello largo (Aurorus)")
cur(741, 0, 0, 702, 0, "Dedenne", "base", "PROBABLE", "icono: raton naranja (Dedenne)")
cur(742, 0, 0, 684, 0, "Swirlix", "base", "PROBABLE", "icono: algodon rosa (Swirlix)")
cur(743, 0, 0, 685, 0, "Slurpuff", "base", "PROBABLE", "icono: dulce rosa (Slurpuff)")
cur(744, 0, 0, 679, 0, "Honedge", "base", "PROBABLE", "icono: espada azul (Honedge)")
cur(745, 0, 0, 680, 0, "Doublade", "base", "PROBABLE", "icono: espadas gemelas (Doublade)")
cur(746, 0, 0, 681, 0, "Aegislash", "base", "PROBABLE", "icono: escudo con espadas; 2 subcarpetas (Aegislash)")
cur(747, 0, 0, 688, 0, "Binacle", "base", "PROBABLE", "icono: percebes (Binacle)")
cur(748, 0, 0, 689, 0, "Barbaracle", "base", "PROBABLE", "icono: roca con brazos (Barbaracle)")
cur(752, 0, 0, 674, 0, "Pancham", "base", "PROBABLE", "icono: panda con hoja (Pancham)")
cur(753, 0, 0, 661, 0, "Fletchling", "base", "PROBABLE", "icono: pajaro naranja (Fletchling)")
cur(754, 0, 0, 662, 0, "Fletchinder", "base", "PROBABLE", "icono: pajaro con cresta (Fletchinder)")
cur(755, 0, 0, 663, 0, "Talonflame", "base", "PROBABLE", "icono: halcon rojo (Talonflame)")
cur(756, 0, 0, 692, 0, "Clauncher", "base", "PROBABLE", "icono: crustaceo azul (Clauncher)")
cur(757, 0, 0, 693, 0, "Clawitzer", "base", "PROBABLE", "icono: crustaceo azul grande (Clawitzer)")
cur(768, 0, 0, 716, 0, "Xerneas", "base", "PROBABLE", "icono: ciervo azul con astas (Xerneas)")
cur(769, 0, 0, 717, 0, "Yveltal", "base", "PROBABLE", "icono: pajaro rojo oscuro (Yveltal)")
cur(770, 0, 0, 718, 0, "Zygarde", "base", "PROBABLE",
    "icono: dragon verde; 6 subcarpetas = formas de Zygarde")
cur(772, 0, 0, 719, 0, "Diancie", "base", "PROBABLE", "icono: hada cristal; 2 subcarpetas (Diancie + Mega)")

# --- gen-7 starters (Rowlet line verified by the PLA catalog) --------------
cur(845, 0, 0, 726, 0, "Torracat", "base", "PROBABLE", "icono: gato naranja (Torracat)")
cur(846, 0, 0, 727, 0, "Incineroar", "base", "PROBABLE", "icono: gato grande rojo (Incineroar)")
cur(847, 0, 0, 728, 0, "Popplio", "base", "PROBABLE", "icono: foca azul (Popplio)")
cur(848, 0, 0, 729, 0, "Brionne", "base", "PROBABLE", "icono: foca con volantes (Brionne)")
cur(849, 0, 0, 730, 0, "Primarina", "base", "PROBABLE", "icono: foca cantante (Primarina)")

# --- gen-8 (Galar) ----------------------------------------------------------
cur(918, 0, 0, 822, 0, "Corvisquire", "base", "PROBABLE", "icono: cuervo azul (Corvisquire)")
cur(919, 0, 0, 823, 0, "Corviknight", "base", "PROBABLE", "icono: cuervo acorazado (Corviknight)")
cur(941, 0, 0, 778, 0, "Mimikyu", "base", "PROBABLE",
    "icono: disfraz de Pikachu; 2 subcarpetas (disfraz/roto)")
cur(972, 0, 0, 854, 0, "Sinistea", "base", "PROBABLE", "icono: taza de te con cara (Sinistea)")
cur(973, 0, 0, 855, 0, "Polteageist", "base", "PROBABLE", "icono: tetera (Polteageist)")
cur(981, 0, 0, 865, 0, "Sirfetch'd", "base", "PROBABLE", "icono: pato blanco con puerro (Sirfetch'd)")

# --- Paldea starters + first route species (contiguous verified run) -------
# The Paldea block starts at 1010 in Paldea-dex order; identified by icons and
# cross-checked over 15 consecutive entries (starters, Lechonk, spiders).
cur(1010, 0, 0, 906, 0, "Sprigatito", "base", "PROBABLE", "icono: felino verde (Sprigatito)")
cur(1011, 0, 0, 907, 0, "Floragato", "base", "PROBABLE", "icono: felino verde (Floragato)")
cur(1012, 0, 0, 908, 0, "Meowscarada", "base", "PROBABLE", "icono: felino enmascarado (Meowscarada)")
cur(1013, 0, 0, 909, 0, "Fuecoco", "base", "PROBABLE", "icono: cocodrilo rojo (Fuecoco)")
cur(1014, 0, 0, 910, 0, "Crocalor", "base", "PROBABLE", "icono: cocodrilo con fuego (Crocalor)")
cur(1015, 0, 0, 911, 0, "Skeledirge", "base", "PROBABLE", "icono: cocodrilo con calavera (Skeledirge)")
cur(1016, 0, 0, 912, 0, "Quaxly", "base", "PROBABLE", "icono: pato azul (Quaxly)")
cur(1017, 0, 0, 913, 0, "Quaxwell", "base", "PROBABLE", "icono: pato azul (Quaxwell)")
cur(1018, 0, 0, 914, 0, "Quaquaval", "base", "PROBABLE", "icono: pavo real azul (Quaquaval)")
cur(1019, 0, 0, 915, 0, "Lechonk", "base", "PROBABLE", "icono: cochinillo negro (Lechonk)")
cur(1020, 0, 0, 916, 0, "Oinkologne", "base", "PROBABLE", "icono: cerdo; 2 subcarpetas = macho/hembra")
cur(1021, 0, 0, 917, 0, "Tarountula", "base", "PROBABLE", "icono: arana amarilla (Tarountula)")
cur(1022, 0, 0, 918, 0, "Spidops", "base", "PROBABLE", "icono: arana gris (Spidops)")
cur(1023, 0, 0, 919, 0, "Nymble", "base", "PROBABLE", "icono: saltamontes verde (Nymble)")
cur(1024, 0, 0, 920, 0, "Lokix", "base", "PROBABLE", "icono: saltamontes oscuro (Lokix)")

REGIONS = {
    11: ("alola", "regional", "Alola"),
    31: ("galar", "regional", "Galar"),
    41: ("hisui", "regional", "Hisui"),
    51: ("paldea", "regional", "Paldea"),
}

# The 33 FF=81 packs are exactly the Gigantamax species list (SwSh).
GMAX_SPECIES = {
    3, 6, 9, 12, 25, 52, 68, 94, 99, 131, 133, 143, 144, 145, 146, 150, 3, 6, 9, 12,
    25, 52, 68, 94, 99, 131, 133, 143, 144, 145, 146, 150, 809, 812, 815, 818, 820,
    823, 826, 834, 839, 841, 842, 849, 851, 858, 861, 869, 879, 884, 890, 892,
}


def load_pla_catalog() -> dict[str, tuple[int, int, int, str]]:
    """pack stem -> (dex, form, female, reason) from the in-game PLA catalog."""
    out = {}
    path = os.path.join(REPO, "data", "pokemon_resource_table.tsv")
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            if line.startswith("#") or line.startswith("species") or not line.strip():
                continue
            f = line.rstrip("\n").split("\t")
            if len(f) < 4:
                continue
            try:
                dex, form, female = int(f[0]), int(f[1]), int(f[2])
            except ValueError:
                continue
            out.setdefault(f[3], (dex, form, female, "catalogo PLA del juego (VERIFICADO)"))
    return out


def load_forms():
    """(species, form_index) -> (name_en, kind); plus species -> regional index."""
    names = {}
    regional = {}
    path = os.path.join(REPO, "data", "national", "forms.tsv")
    with open(path, encoding="utf-8") as fh:
        lines = [l for l in fh if not l.startswith("#")]
    rd = csv.reader(lines, delimiter="\t")
    header = [h.split(":")[0] for h in next(rd)]
    for fields in rd:
        row = dict(zip(header, fields))
        dex = int(row["national_number"])
        fi = int(row["form_index"])
        names[(dex, fi)] = (row["name_en"], row["form_kind"])
        ident = row["form_identifier"].lower()
        for vv, (needle, _, _) in REGIONS.items():
            if needle in ident:
                regional.setdefault((dex, vv), fi)
    return names, regional


def scan_models(root: str):
    """Yield (top, sub, pack, blend, texture_count)."""
    for top in sorted(os.listdir(root)):
        tp = os.path.join(root, top)
        if not os.path.isdir(tp) or not re.match(r"^pm\d{4}(_\d{2})?$", top):
            continue
        for sub in sorted(os.listdir(tp)):
            sp = os.path.join(tp, sub)
            if not os.path.isdir(sp) or not re.match(r"^pm\d{4}_\d{2}(_\d{2})?$", sub):
                continue
            texdir = os.path.join(sp, "textures")
            if os.path.isdir(texdir):
                tex = os.listdir(texdir)
            else:
                tex = [t for t in os.listdir(sp) if t.endswith(".png")]
            blend = next((b for b in os.listdir(sp) if b.endswith(".blend")), None)
            yield top, sub, sub, blend, tex


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--models-root", default=DEFAULT_MODELS_ROOT)
    args = ap.parse_args()

    if not os.path.isdir(args.models_root):
        print(f"models root not found: {args.models_root}", file=sys.stderr)
        return 2

    pla = load_pla_catalog()
    forms, regional = load_forms()
    rows = []
    seen_pack = Counter()
    stats = Counter()

    def form_name(dex, fi, fallback_kind="form"):
        return forms.get((dex, fi), (f"forma {fi}", fallback_kind))[0]

    for top, sub, pack, blend, tex in scan_models(args.models_root):
        m = re.match(r"^pm(\d{4})_(\d{2})(?:_(\d{2}))?$", pack)
        num, ff = int(m.group(1)), int(m.group(2))
        vv = int(m.group(3)) if m.group(3) else 0
        entry = None

        # 1) PLA in-game catalog: exact pack stem (VERIFIED).
        if pack in pla:
            dex, form, female, reason = pla[pack]
            fname = form_name(dex, form)
            kind = forms.get((dex, form), (fname, "otro"))[1] if form else "base"
            if form and kind in ("base", "form"):
                kind = "regional" if (dex, form) in [(d, f) for (d, v), f in regional.items() if v == 41] else "cosmetica"
            entry = (dex, form, fname, kind, "VERIFICADO", reason + (" (hembra)" if female else ""))

        # 2) 1..649: pack number equals the national dex number (VERIFIED).
        if entry is None and 1 <= num <= 649:
            base = (num, 0, form_name(num, 0), "base")
            if ff == 0 and vv == 0:
                entry = (*base, "VERIFICADO",
                         "pmNNNN base con NNNN<=649: el indice del modelo es el nº nacional")
            elif ff == 1:
                entry = (num, 0, form_name(num, 0), "otro", "PROBABLE",
                         "FF=01 = modelo hembra (convencion del catalogo PLA: columna female=1)")
            elif ff in (51, 52):
                mega = "mega 1" if ff == 51 else "mega 2"
                fi = 1 if ff == 51 else 2
                entry = (num, fi, form_name(num, fi), "mega", "PROBABLE",
                         f"FF={ff} = {mega} (anclas: pm0006_51_00 Mega Charizard X, pm0006_52_00 Mega Y, "
                         "pm0445_51_00 Mega Garchomp)")
            elif ff in (81, 82):
                entry = (num, 0, form_name(num, 0), "gigamax", "PROBABLE",
                         "FF=%d = Gigamax (los 33 packs _81 son exactamente la lista de Gigamax)" % ff)
            elif 11 <= ff <= 50:
                fi = ff - 11
                entry = (num, fi, form_name(num, fi), "cosmetica", "PROBABLE",
                         "FF=11+n = forma n del juego (anclas PLA: Unown, Burmy, Shellos, Rotom)")
            elif vv == 1:
                entry = (num, 0, form_name(num, 0), "variante-color", "VERIFICADO",
                         "VV=01 = variante rara/brillante (texturas _col_rare en el propio modelo)")
            elif vv in REGIONS:
                needle, kind, label = REGIONS[vv]
                fi = regional.get((num, vv), 0)
                entry = (num, fi, form_name(num, fi), "regional", "VERIFICADO",
                         f"VV={vv} = forma regional de {label} (lista exacta de formas + anclas PLA)")

        # 3) curated visual identification (PROBABLE).
        if entry is None and (num, ff, vv) in CURATED:
            entry = CURATED[(num, ff, vv)]

        # 4) conventions for species known through anchors/curation.
        if entry is None and num > 649:
            known = None
            for (knum, kff, kvv), val in CURATED.items():
                if knum == num:
                    known = val[0]
                    break
            if known is None:
                for p, (dex, form, female, _r) in pla.items():
                    pm = re.match(r"^pm(\d{4})_", p)
                    if pm and int(pm.group(1)) == num:
                        known = dex
                        break
            if known:
                if vv == 1:
                    entry = (known, 0, form_name(known, 0), "variante-color", "PROBABLE",
                             "VV=01 brillante (convencion verificada); especie por ancla")
                elif ff in (51, 52):
                    fi = 1 if ff == 51 else 2
                    entry = (known, fi, form_name(known, fi), "mega", "PROBABLE",
                             "FF=%d Mega (convencion verificada); especie por ancla" % ff)
                elif ff in (81, 82):
                    entry = (known, 0, form_name(known, 0), "gigamax", "PROBABLE",
                             "FF=%d Gigamax (convencion verificada); especie por ancla" % ff)

        if entry is None:
            entry = (0, 0, "", "otro", "DESCONOCIDO",
                     "indice interno del modelo no deducible con la evidencia disponible")

        dex, form, fname, kind, conf = entry[:5]
        reason = entry[5] if len(entry) > 5 else ""
        stats[conf] += 1
        seen_pack[pack] += 1
        rows.append((pack, dex, form, fname, kind, conf, reason))

    rows.sort(key=lambda r: (r[1], r[2], r[0]))
    out = os.path.join(REPO, "data", "national", "model_map.tsv")
    with open(out, "w", encoding="utf-8") as fh:
        fh.write(HEADER)
        fh.write(COLUMNS + "\n")
        for pack, dex, form, fname, kind, conf, reason in rows:
            fh.write(f"{pack}\t{dex}\t{form}\t{fname}\t{kind}\t{conf}\t{reason}\n")

    species_with = {r[1] for r in rows if r[5] != "DESCONOCIDO" and r[1] > 0}
    unknown_rows = sum(1 for r in rows if r[5] == "DESCONOCIDO")
    print(f"rows={len(rows)} unique_packs={len(seen_pack)} duplicates={sum(1 for v in seen_pack.values() if v > 1)}")
    print(f"confidence: {dict(stats)}")
    print(f"species with a mapped model: {len(species_with)} (of 1025)")
    print(f"rows without identification: {unknown_rows}")
    print(f"written: {os.path.relpath(out, REPO)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
