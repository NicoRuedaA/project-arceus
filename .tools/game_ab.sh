#!/bin/bash
# game_ab.sh <stem> <regex> <out.png> <cam-dist> <focus-h> [species]
# A/B in the real game: temporarily swaps work/cache/pokemon_batch/<stem>/<stem>_idle.gltf for a copy without the meshes
# matching <regex>, photographs species with the close-up camera, restores the file (always).
stem=$1; rx=$2; out=$3; cam=$4; fh=$5; sp=${6:-$((10#${stem:2:4}))}
C=/home/nico/work/decompilacion/work/cache/pokemon_batch/$stem; f=$C/${stem}_idle.gltf
cp "$f" "$f.ab_orig"
trap 'mv -f "$f.ab_orig" "$f"' EXIT
python3 .tools/gltf_drop_nodes.py "$f.ab_orig" "$C/tmp_ab.gltf" "$rx" >/dev/null && mv "$C/tmp_ab.gltf" "$f"
.tools/game_closeup.sh "$(dirname "$out")" "$cam" "$fh" "$sp" >/dev/null 2>&1
mv "$(dirname "$out")/c$(printf %03d "$sp").png" "$out"
