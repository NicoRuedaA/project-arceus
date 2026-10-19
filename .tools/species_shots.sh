#!/bin/bash
# species_shots.sh <out-dir> <species[:form]> <frames,frames,...> <yaw,yaw,...> [spawn-dist] [pitch] [cam-distance]
# One CPU viewer (no window), several real-game shots of ONE species at given idle frames and camera yaws.
# Env: AUTOMATION_DIR (own dir), BIN (viewer binary).
out=$1; sp=$2; frames=$3; yaws=$4; sd=${5:--2}; pitch=${6:-10}; cd_=${7:-1.6}
BIN=${BIN:-/home/nico/work/cargo-target-fixes/debug/examples/port_viewer}
D=${AUTOMATION_DIR:-work/automation-ss}; G="python3 .tools/gamectl.py --dir $D"; mkdir -p "$out"
$G start --cpu --no-build --bin "$BIN" --size 960x540 -- --area 0 --spawn-species "$sp" --spawn-dist "$sd" >/dev/null 2>&1
done_=0; name=${sp/:/_f}
for f in ${frames//,/ }; do
  $G step $((f-done_)) --fast >/dev/null; done_=$f
  for y in ${yaws//,/ }; do
    $G shot "$out/s${name}_f${f}_y${y}.png" --yaw "$y" --pitch "$pitch" --distance "$cd_" --width 1000 --height 800 >/dev/null
  done
done
$G stop >/dev/null 2>&1
