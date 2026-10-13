#!/usr/bin/env bash
# One-command installer: extract YOUR OWN copy of the game and build every local cache.
#
#   ./install.sh --base /path/to/pk1.nsz --update /path/to/pk2.nsz --keys /path/to/prod.keys
#   ./install.sh            # later runs: the paths are remembered; only missing/stale steps run
#   ./install.sh --check    # change nothing; print the exact steps that would run
#   ./install.sh --help     # every option
#
# Everything is written under work/ (ignored by git). This repository ships no game data.
# Step-by-step guide: docs/INSTALL.md (English) / docs/INSTALL.es.md (Español).
set -euo pipefail
cd "$(dirname "$0")"
if ! command -v python3 >/dev/null 2>&1; then
  echo "error: python3 is not installed (Python 3.10 or newer is needed; 3.13 recommended)" >&2
  exit 2
fi
if ! python3 -c 'import sys; sys.exit(0 if sys.version_info >= (3, 10) else 1)'; then
  echo "error: python3 is $(python3 -V 2>&1 | cut -d' ' -f2); Python 3.10 or newer is needed" >&2
  exit 2
fi
exec python3 .tools/setup_cache.py "$@"
