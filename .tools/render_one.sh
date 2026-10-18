#!/bin/bash
# render_one.sh <asset-root-relative-to-cwd> <out-dir> <size> <yaw,yaw,...> label=rel.gltf ...   (CPU, lavapipe; takes a render slot)
root=$1; out=$2; size=$3; yaws=$4; shift 4
mkdir -p "$out"
for n in 1 2 3; do mkdir /home/nico/work/decompilacion/work/progress/render-$n.lock 2>/dev/null && slot=$n && break; done
while [ -z "$slot" ]; do sleep 30; for n in 1 2 3; do mkdir /home/nico/work/decompilacion/work/progress/render-$n.lock 2>/dev/null && slot=$n && break; done; done
for y in ${yaws//,/ }; do
  args=(); for a in "$@"; do args+=("${a%%=*}_y$y=${a#*=}"); done
  VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json WGPU_BACKEND=vulkan /home/nico/work/cargo-target-fixes/debug/examples/render_gltf_headless --root "$root" --out "$out" --size "$size" --yaw "$y" "${args[@]}" >/dev/null 2>&1
done
rmdir /home/nico/work/decompilacion/work/progress/render-$slot.lock
