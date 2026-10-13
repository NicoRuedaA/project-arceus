#!/usr/bin/env bash
# Juega con tus texturas mejoradas (work/texturas_mejoradas) por encima de las originales.
# Uso: ./play.sh --area 1 [otras opciones del visor]      |      ./play.sh --list
# Antes hay que instalar una vez con ./install.sh (ver docs/INSTALL.es.md): si faltan los datos
# del juego, este guion lo dice con la orden exacta y no arranca el visor.
# Si has mejorado texturas del suelo (work/texturas_mejoradas/terrain_layers/*.png), antes de
# arrancar se re-hornea el suelo de las zonas afectadas (solo escribe en work/cache/terrain_albedo).
set -euo pipefail
cd "$(dirname "$0")"
if [ "${1:-}" = "--list" ]; then exec python3 .tools/texture_overrides.py --list; fi
# 0) comprobar la instalación (no escribe nada): sin datos del juego no se arranca
python3 .tools/setup_cache.py --preflight || exit 1
# datos del juego extraídos por ./install.sh (se respeta un PLA_BIN_ROOT ya exportado)
export PLA_BIN_ROOT="${PLA_BIN_ROOT:-$PWD/work/pla/pk2/romfs_patched/bin}"
# zona pedida (--area N / --area=N): solo se re-hornea esa; sin ella, todas las afectadas
area=""
prev=""
for a in "$@"; do
  case "$a" in --area=*) area="${a#--area=}";; esac
  if [ "$prev" = "--area" ]; then area="$a"; fi
  prev="$a"
done
case "$area" in ''|*[!0-9]*) rebake_args=();; *) rebake_args=(--areas "$area");; esac
# 1) re-hornear el suelo ANTES de montar la capa de texturas (la capa enlaza los PNG ya horneados)
python3 .tools/terrain_rebake.py "${rebake_args[@]}" \
  || echo "AVISO: no se pudo re-hornear el suelo; se usa el que ya habia" >&2
# 2) montar la capa de texturas mejoradas y lanzar el visor
overlay="$(python3 .tools/texture_overrides.py | tail -n 1)"
export PLA_CACHE_ROOT="$overlay"
exec cargo run --release -p pla --example port_viewer -- "$@"
