#!/bin/bash
# game_orbit.sh <out-dir> <species[:form]> <cam-dist> <focus-h> <pitch> yaw [yaw ...]
# One CPU viewer, one close-up per orbit yaw (degrees) around the F7/--spawn-species model. Env: BIN, AUTOMATION_DIR.
out=$1; spec=$2; cam=$3; fh=$4; pitch=$5; shift 5
BIN=${BIN:-/home/nico/work/cargo-target-fixes/debug/examples/port_viewer}
G="python3 .tools/gamectl.py --dir ${AUTOMATION_DIR:-work/automation-orbit}"
mkdir -p "$out"; case "$out" in /*) ABS="";; *) ABS="$PWD/";; esac
$G start --cpu --no-build --bin "$BIN" --size 960x540 -- --area 0 --spawn-species "$spec" --spawn-dist -2 >/dev/null 2>&1
$G step 150 --fast >/dev/null
for y in "$@"; do
  $G cmd "{\"cmd\":\"screenshot\",\"path\":\"$ABS$out/${spec/:/_f}_y$y.png\",\"width\":1600,\"height\":900,\"camera\":{\"yaw\":$y,\"pitch\":$pitch,\"distance\":$cam,\"focus\":[402.32,$fh,374.7],\"degrees\":true}}" >/dev/null
done
$G stop >/dev/null 2>&1
