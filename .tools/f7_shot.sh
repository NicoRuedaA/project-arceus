#!/bin/bash
# f7_shot.sh <species> <out.png> [form-right-presses] [pitch] [distance]
# Places species N with the F7 spawner in a running gamectl session (--dir work/automation-texturas-151) and photographs it.
G="python3 .tools/gamectl.py --dir ${AUTOMATION_DIR:-work/automation-texturas-151}"
n=$1; out=$2; forms=${3:-0}; pitch=${4:-8}; dist=${5:-3}
$G key F7 --frames 2 >/dev/null; $G step 3 --fast >/dev/null
for _ in 1 2 3 4; do $G key Backspace --frames 1 >/dev/null; done
for ((i=0;i<${#n};i++)); do $G key "${n:$i:1}" --frames 2 >/dev/null; $G step 2 --fast >/dev/null; done
for ((i=0;i<forms;i++)); do $G key Right --frames 2 >/dev/null; $G step 2 --fast >/dev/null; done
$G key Enter --frames 2 >/dev/null; $G step 90 --fast >/dev/null
$G key Escape --frames 2 >/dev/null; $G step 20 --fast >/dev/null
$G shot "$out" --pitch "$pitch" --distance "$dist" --width 1600 --height 900 | cut -c1-60
