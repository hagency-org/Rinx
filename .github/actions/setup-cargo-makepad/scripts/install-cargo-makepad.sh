#!/usr/bin/env bash
# Install cargo-makepad from the Makepad checkout that Cargo.lock selects.
#
# `cargo install --git <makepad> cargo-makepad` no longer works: it loads every
# manifest in the repository, and Makepad's `libs/ai/services` is a member of
# the root workspace while sitting under the separate `libs/ai` workspace
# ("is a member of the wrong workspace"). Building inside the root workspace
# fails too, because its [patch] names a sibling `../octoscript-makepad`.
#
# cargo-makepad itself only needs Makepad's own path crates. So build it from
# Cargo's checkout of the pinned revision as a workspace of its own. That is
# the same checkout the old command built from, so cargo-makepad still keeps
# its SDKs (tools/cargo_makepad/android_*, ios-deploy) where
# tools/package-octos.py looks for them.
set -euo pipefail

lockfile="${1:-Cargo.lock}"
expected_rev="${2:-}"
manifest="$(dirname "$lockfile")/Cargo.toml"

makepad="$(
  cargo metadata --locked --format-version 1 --manifest-path "$manifest" |
    python3 -c '
import json, pathlib, sys
packages = [p for p in json.load(sys.stdin)["packages"] if p["name"] == "makepad-platform"]
if len(packages) != 1:
    sys.exit("expected exactly one makepad-platform in the lockfile")
print(pathlib.Path(packages[0]["manifest_path"]).parent.parent.as_posix())
'
)"
if [[ -n "$expected_rev" ]] && ! grep -q "#${expected_rev}\"" <<<"$(grep -A2 '^name = "makepad-platform"' "$lockfile")"; then
  echo "error: makepad-platform in $lockfile is not at $expected_rev" >&2
  exit 1
fi

tool="$makepad/tools/cargo_makepad"
backup="$(mktemp)"
cp "$tool/Cargo.toml" "$backup"
had_lock=0
[[ -e "$tool/Cargo.lock" ]] && had_lock=1
# Leave Cargo's checkout exactly as it was once cargo-makepad is installed.
restore() {
  cp "$backup" "$tool/Cargo.toml"
  rm -f "$backup"
  [[ "$had_lock" == 1 ]] || rm -f "$tool/Cargo.lock"
}
trap restore EXIT
printf '\n[workspace]\n' >> "$tool/Cargo.toml"

echo "Installing cargo-makepad from $tool"
CARGO_TARGET_DIR="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/cargo-makepad-target" \
  cargo install --force --path "$tool" "${@:3}"
