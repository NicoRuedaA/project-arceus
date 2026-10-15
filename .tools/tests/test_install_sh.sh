#!/usr/bin/env bash
# Shell-level checks of ./install.sh and the play.sh pre-flight. Uses only a throw-away
# folder and tiny synthetic files; never reads real game files and never starts the viewer.
#   bash .tools/tests/test_install_sh.sh
set -uo pipefail
cd "$(dirname "$0")/../.."
tmp="$(mktemp -d)"
trap 'rm -rf -- "$tmp"' EXIT
fails=0
pass() { printf 'ok   %s\n' "$1"; }
fail() { printf 'FAIL %s\n' "$1"; fails=$((fails + 1)); }
expect() { # name, wanted exit code, wanted text, command...
  local name="$1" code="$2" text="$3"; shift 3
  local out rc
  out="$("$@" 2>&1)"; rc=$?
  if [ "$rc" -ne "$code" ]; then fail "$name (exit $rc, wanted $code)"; printf '%s\n' "$out" | tail -5; return; fi
  if ! grep -qF -- "$text" <<<"$out"; then fail "$name (missing text: $text)"; printf '%s\n' "$out" | tail -5; return; fi
  pass "$name"
}

w="$tmp/work"; mkdir -p "$w"
expect "check names the missing base file" 2 "missing --base" ./install.sh --check --work "$w"
[ -z "$(ls -A "$w")" ] && pass "check wrote nothing" || fail "check wrote files: $(ls -A "$w")"

truncate -s 1234 "$tmp/pk1.nsz"; truncate -s 52657467 "$tmp/pk2.nsz"
printf 'header_key = %s\n' "$(printf 'ab%.0s' $(seq 32))" > "$tmp/prod.keys"   # synthetic value
expect "wrong base size is explained" 2 "expected 2,334,586,382 bytes, got 1,234" \
  ./install.sh --check --work "$w" --base "$tmp/pk1.nsz" --update "$tmp/pk2.nsz" --keys "$tmp/prod.keys"
expect "swapped inputs are detected" 2 "did you swap --base and --update" \
  ./install.sh --check --work "$w" --base "$tmp/pk2.nsz" --update "$tmp/pk2.nsz" --keys "$tmp/prod.keys"
expect "missing input file" 2 "no such file" \
  ./install.sh --check --work "$w" --base "$tmp/none.nsz" --update "$tmp/pk2.nsz" --keys "$tmp/prod.keys"
echo "title_key = 00" > "$tmp/title.keys"
expect "keys without header_key" 2 "no 'header_key' line" \
  ./install.sh --check --work "$w" --base "$tmp/pk1.nsz" --update "$tmp/pk2.nsz" --keys "$tmp/title.keys"
expect "unknown step" 1 "unknown step 'bogus'" ./install.sh --check --work "$w" --only bogus
expect "bad area" 2 "--areas takes numbers 0..6" ./install.sh --check --work "$w" --areas 9
expect "real run refuses bad inputs before any step" 2 "Cannot start, please fix:" \
  ./install.sh --work "$w" --base "$tmp/pk1.nsz" --update "$tmp/pk2.nsz" --keys "$tmp/prod.keys" --no-memory-limit
[ ! -e "$w/install-state/config.json" ] && pass "bad inputs are not remembered" || fail "bad inputs were saved"

# play.sh pre-flight (called directly: play.sh itself would start the viewer when data exists)
expect "preflight without data names the install command" 1 "./install.sh --base <your pk1.nsz>" \
  python3 .tools/setup_cache.py --preflight --work "$tmp/empty"
mkdir -p "$tmp/w2/pla/pk2/romfs_patched/bin/field"
expect "preflight with data but no caches warns and passes" 0 "To build them run:" \
  python3 .tools/setup_cache.py --preflight --work "$tmp/w2"

# nothing game-derived is tracked by git, and work/ is ignored
bad="$(git ls-files | grep -E '(^|/)work/|\.(nsz|nsp|xci|nca|ncz|nso|elf|keys)$|romfs_patched|(^|/)prod\.keys$' || true)"
[ -z "$bad" ] && pass "no game containers, keys or work/ files are tracked" || fail "tracked game-like files: $bad"
git check-ignore -q work/cache/x.png && git check-ignore -q work/install-state/config.json \
  && pass "work/ (caches and installer state) is ignored by git" || fail "work/ is not ignored"
if LC_ALL=C grep -qE '(header|key_area|titlekek|master)_key[_0-9a-f]* *= *[0-9a-fA-F]{32}' \
    install.sh play.sh .tools/setup_cache.py .tools/safe_io.py docs/INSTALL*.md; then
  fail "an installer file contains a key value"
else
  pass "installer files and docs contain no key values (only pinned file hashes)"
fi

if [ "$fails" -eq 0 ]; then echo "all install.sh checks passed"; else echo "$fails install.sh check(s) failed"; exit 1; fi
