#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd -P -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
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

# Load the CLI modules directly so these foundation tests can replace the
# heavyweight network provisioning functions with local stubs. Provisioning
# itself is tested separately; these assertions cover CLI/layout behaviour.
# shellcheck source=../lib/core.sh
source "$ROOT/lib/core.sh"
# shellcheck source=../lib/provision.sh
source "$ROOT/lib/provision.sh"
# shellcheck source=../lib/flutter.sh
source "$ROOT/lib/flutter.sh"
# shellcheck source=../lib/java.sh
source "$ROOT/lib/java.sh"
# shellcheck source=../lib/android.sh
source "$ROOT/lib/android.sh"
# shellcheck source=../lib/commands/install.sh
source "$ROOT/lib/commands/install.sh"
# shellcheck source=../lib/commands/list.sh
source "$ROOT/lib/commands/list.sh"
# shellcheck source=../lib/commands/doctor.sh
source "$ROOT/lib/commands/doctor.sh"

flenv_provision_flutter() {
	local environment="$1"
	mkdir -p -- "$environment/flutter"
	flenv_mark_component_ready "$environment" flutter
}

flenv_provision_android() {
	local environment="$1"
	mkdir -p -- "$environment/android-sdk"
	flenv_mark_component_ready "$environment" android
}

flenv_android_licenses() {
	return 0
}

run_install() {
	flenv_install "$@"
}

version="$("$ROOT/bin/flenv" --version)"
assert_contains "$version" "flenv 0.1.0-dev"

help="$("$ROOT/bin/flenv" --help)"
assert_contains "$help" "flenv install"

run_install --name local >/dev/null
assert_dir "$FLENV_HOME/environments/local/flutter"
assert_dir "$FLENV_HOME/environments/local/android-sdk"
[[ ! -e "$FLENV_HOME/environments/local/workspace" ]] || fail "non-isolated install created workspace"

EXTERNAL="$TEST_ROOT/storage"
run_install --root "$EXTERNAL" --name isolated --isolated >/dev/null
ENV="$EXTERNAL/flenv/environments/isolated"
assert_dir "$ENV/flutter"
assert_dir "$ENV/android-sdk"
assert_dir "$ENV/cache/pub"
assert_dir "$ENV/cache/gradle"
assert_dir "$ENV/workspace"
assert_dir "$ENV/state"

list="$(flenv_list)"
assert_contains "$list" "local"
assert_contains "$list" "isolated"
assert_contains "$list" "$ENV"

# flenv_die intentionally exits. Run expected-failure cases in subshells so
# their exit status can be asserted without terminating this test process.
if (run_install --name '../escape' >/dev/null 2>&1); then
	fail "path traversal name was accepted"
fi

if (run_install --name 'bad/name' >/dev/null 2>&1); then
	fail "slash in environment name was accepted"
fi

JAVA_FIXTURE="$TEST_ROOT/jdk"
mkdir -p "$JAVA_FIXTURE/bin"
cat >"$JAVA_FIXTURE/bin/java" <<'EOF'
#!/usr/bin/env bash
printf 'openjdk version "21.0.1" 2023-10-17\n' >&2
EOF
chmod +x "$JAVA_FIXTURE/bin/java"
java_home="$(JAVA_HOME="$JAVA_FIXTURE" flenv_detect_java)"
[[ "$java_home" == "$JAVA_FIXTURE" ]] || fail "JAVA_HOME JDK was not selected"

cat >"$JAVA_FIXTURE/bin/java" <<'EOF'
#!/usr/bin/env bash
printf 'openjdk version "11.0.24" 2024-07-16\n' >&2
EOF
chmod +x "$JAVA_FIXTURE/bin/java"
if (JAVA_HOME="$JAVA_FIXTURE" flenv_detect_java >/dev/null 2>&1); then
	fail "Java older than 17 was accepted"
fi

flenv_doctor >/dev/null

printf 'PASS: CLI foundation\n'
