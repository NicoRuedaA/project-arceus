#!/usr/bin/env bash
# Juega con tus texturas mejoradas (work/texturas_mejoradas) por encima de las originales.
# Uso: ./play.sh --area 1 [otras opciones del visor]      |      ./play.sh --list
set -euo pipefail
cd "$(dirname "$0")"
if [ "${1:-}" = "--list" ]; then exec python3 .tools/texture_overrides.py --list; fi
overlay="$(python3 .tools/texture_overrides.py | tail -n 1)"
export PLA_CACHE_ROOT="$overlay"
exec cargo run --release -p pla --example port_viewer -- "$@"
