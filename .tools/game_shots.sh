#!/bin/bash
# game_shots.sh <out-dir> <spawn-dist> <pitch> <cam-distance> N[:form][@clip] ...
# One fresh CPU viewer per species (`--spawn-species N:form`, no other model on screen), one 1600x900 shot each.
# Env: BIN (viewer binary), AUTOMATION_DIR (own directory, never shared). Slow (about 25 s per species).
out=$1; sd=$2; pitch=$3; cd_=$4; shift 4
BIN=${BIN:-/home/nico/work/cargo-target-fixes/debug/examples/port_viewer}
D=${AUTOMATION_DIR:-work/automation-gs}
G="python3 .tools/gamectl.py --dir $D"
mkdir -p "$out"
for spec in "$@"; do
  sp=${spec%%@*}; clip=""; [[ $spec == *@* ]] && clip=${spec#*@}
  name=${sp/:/_f}
  extra=(); [ -n "$clip" ] && extra=(--spawn-clip "$clip")
  $G start --cpu --no-build --bin "$BIN" --size 960x540 -- --area 0 --spawn-species "$sp" --spawn-dist "$sd" "${extra[@]}" >/dev/null 2>&1
  $G step 150 --fast >/dev/null
  $G shot "$out/s$(printf %s "$name" | awk -F_ '{printf "%03d%s",$1,($2?"_"$2:"")}')${clip:+_$clip}.png" --pitch "$pitch" --distance "$cd_" --width 1600 --height 900 | cut -c1-40
  $G stop >/dev/null 2>&1
done
