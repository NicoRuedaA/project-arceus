#!/bin/bash
# game_closeup.sh <out-dir> <cam-distance> <focus-height> N[:form][@cam,focus-height] ...
# Real-game close-ups: one fresh CPU viewer per species (--spawn-species, placed 2 m ahead of the spawn
# player), orbit camera focused on the model (player spawn of --area 0 is at 402.32,76.82,376.71),
# 960x540 viewer, 1600x900 shot cropped to the model. Env: BIN, AUTOMATION_DIR (own directory).
out=$1; cam=$2; fh=$3; shift 3
BIN=${BIN:-/home/nico/work/cargo-target-fixes/debug/examples/port_viewer}
D=${AUTOMATION_DIR:-work/automation-closeup}
G="python3 .tools/gamectl.py --dir $D"
mkdir -p "$out"; case "$out" in /*) ABS="";; *) ABS="$PWD/";; esac
for spec0 in "$@"; do
  # N[:form][@cam,focus-height] overrides the defaults for one species
  spec=${spec0%%@*}; c=$cam; h=$fh
  if [[ $spec0 == *@* ]]; then o=${spec0#*@}; c=${o%%,*}; [[ $o == *,* ]] && h=${o#*,}; fi
  name=${spec/:/_f}
  $G start --cpu --no-build --bin "$BIN" --size 960x540 -- --area 0 --spawn-species "$spec" --spawn-dist -2 >/dev/null 2>&1
  $G step 150 --fast >/dev/null
  f="$out/c$(printf %s "$name" | awk -F_ '{printf "%03d%s",$1,($2?"_"$2:"")}').png"
  $G cmd "{\"cmd\":\"screenshot\",\"path\":\"$ABS$f\",\"width\":1600,\"height\":900,\"camera\":{\"yaw\":0,\"pitch\":6,\"distance\":$cam,\"focus\":[402.32,$fh,374.7],\"degrees\":true}}" | cut -c1-30
  $G stop >/dev/null 2>&1
done
