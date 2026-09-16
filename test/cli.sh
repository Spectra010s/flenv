#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd -P -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
FLENV="$ROOT/bin/flenv"
TEST_ROOT="$(mktemp -d)"
trap 'rm -rf -- "$TEST_ROOT"' EXIT

export HOME="$TEST_ROOT/home"
export FLENV_HOME="$HOME/.flenv"
mkdir -p "$HOME"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

assert_dir() {
  [[ -d "$1" ]] || fail "expected directory: $1"
}

assert_contains() {
  local haystack="$1"
  local needle="$2"
  [[ "$haystack" == *"$needle"* ]] || fail "expected output to contain: $needle"
}

version="$($FLENV --version)"
assert_contains "$version" "flenv 0.1.0-dev"

help="$($FLENV --help)"
assert_contains "$help" "flenv install"

$FLENV install --name local >/dev/null
assert_dir "$FLENV_HOME/environments/local/flutter"
assert_dir "$FLENV_HOME/environments/local/android-sdk"
[[ ! -e "$FLENV_HOME/environments/local/workspace" ]] || fail "non-isolated install created workspace"

EXTERNAL="$TEST_ROOT/storage"
$FLENV install --root "$EXTERNAL" --name isolated --isolated >/dev/null
ENV="$EXTERNAL/flenv/environments/isolated"
assert_dir "$ENV/flutter"
assert_dir "$ENV/android-sdk"
assert_dir "$ENV/cache/pub"
assert_dir "$ENV/cache/gradle"
assert_dir "$ENV/workspace"
assert_dir "$ENV/state"

list="$($FLENV list)"
assert_contains "$list" "local"
assert_contains "$list" "isolated"
assert_contains "$list" "$ENV"

if $FLENV install --name '../escape' >/dev/null 2>&1; then
  fail "path traversal name was accepted"
fi

if $FLENV install --name 'bad/name' >/dev/null 2>&1; then
  fail "slash in environment name was accepted"
fi

$FLENV doctor >/dev/null

printf 'PASS: CLI foundation\n'
