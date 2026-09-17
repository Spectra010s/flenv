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
assert_dir() { [[ -d "$1" ]] || fail "expected directory: $1"; }
assert_contains() { [[ "$1" == *"$2"* ]] || fail "expected output to contain: $2"; }

source "$ROOT/lib/core.sh"
source "$ROOT/lib/provision.sh"
source "$ROOT/lib/flutter.sh"
source "$ROOT/lib/java.sh"
source "$ROOT/lib/android.sh"
source "$ROOT/lib/commands/install.sh"
source "$ROOT/lib/commands/list.sh"
source "$ROOT/lib/commands/doctor.sh"

flenv_provision_flutter() {
	mkdir -p -- "$1/flutter"
	flenv_mark_component_ready "$1" flutter
}
flenv_provision_android() {
	mkdir -p -- "$1/android-sdk"
	flenv_mark_component_ready "$1" android
}
flenv_validate_environment() { flenv_mark_component_ready "$1" environment; }
run_install() { flenv_install "$@"; }

version="$("$ROOT/bin/flenv" --version)"
assert_contains "$version" "flenv 0.1.0-dev"
help="$("$ROOT/bin/flenv" --help)"
assert_contains "$help" "flenv install"
assert_contains "$help" "flenv doctor"

run_install --name local >/dev/null
LOCAL="$FLENV_HOME/environments/local"
assert_dir "$LOCAL/flutter"
assert_dir "$LOCAL/android-sdk"
[[ -f "$LOCAL/state/environment.ready" ]] || fail "validated environment was not marked ready"
[[ ! -e "$LOCAL/workspace" ]] || fail "non-isolated install created workspace"

EXTERNAL="$TEST_ROOT/storage"
run_install --root "$EXTERNAL" --name isolated --isolated >/dev/null
ENV="$EXTERNAL/flenv/environments/isolated"
assert_dir "$ENV/flutter"
assert_dir "$ENV/android-sdk"
assert_dir "$ENV/cache/pub"
assert_dir "$ENV/cache/gradle"
assert_dir "$ENV/workspace"
assert_dir "$ENV/state"
[[ -f "$ENV/state/environment.ready" ]] || fail "isolated environment was not marked ready"

list="$(flenv_list)"
assert_contains "$list" "local"
assert_contains "$list" "isolated"
assert_contains "$list" "$ENV"
assert_contains "$list" "ready"
rm -f -- "$ENV/state/environment.ready"
list="$(flenv_list)"
assert_contains "$list" "incomplete"
touch "$ENV/state/environment.ready"

if (run_install --name '../escape' >/dev/null 2>&1); then fail "path traversal name was accepted"; fi
if (run_install --name 'bad/name' >/dev/null 2>&1); then fail "slash in environment name was accepted"; fi

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
if (JAVA_HOME="$JAVA_FIXTURE" flenv_detect_java >/dev/null 2>&1); then fail "Java older than 17 was accepted"; fi

mkdir -p "$ENV/flutter/bin" "$ENV/android-sdk/platform-tools" "$ENV/android-sdk/platforms/android-36" "$ENV/android-sdk/build-tools/36.0.0"
cat >"$ENV/flutter/bin/flutter" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' '{"frameworkVersion":"3.47.4"}'
EOF
cat >"$ENV/flutter/bin/dart" <<'EOF'
#!/usr/bin/env bash
printf 'Dart SDK version: 3.13.3 (stable)\n' >&2
EOF
cat >"$ENV/android-sdk/platform-tools/adb" <<'EOF'
#!/usr/bin/env bash
printf 'Android Debug Bridge version 1.0.41\nVersion 37.0.1-15733141\n'
EOF
chmod +x "$ENV/flutter/bin/flutter" "$ENV/flutter/bin/dart" "$ENV/android-sdk/platform-tools/adb"
cat >"$JAVA_FIXTURE/bin/java" <<'EOF'
#!/usr/bin/env bash
printf 'openjdk version "21.0.1" 2023-10-17\n' >&2
EOF
chmod +x "$JAVA_FIXTURE/bin/java"
printf '%s\n' "$JAVA_FIXTURE" >"$ENV/state/java-home"

doctor="$(flenv_doctor --name isolated)"
assert_contains "$doctor" "Environment: isolated"
assert_contains "$doctor" "Java 21"
assert_contains "$doctor" "Flutter 3.47.4"
assert_contains "$doctor" "Dart 3.13.3"
assert_contains "$doctor" "Android API 36"
assert_contains "$doctor" "Build Tools 36.0.0"
assert_contains "$doctor" "ADB 37.0.1-15733141"
assert_contains "$doctor" "Workspace"
assert_contains "$doctor" "Isolated caches"
assert_contains "$doctor" "Environment is ready."

verbose="$(flenv_doctor --name isolated --verbose)"
assert_contains "$verbose" "FLENV_HOME: $FLENV_HOME"
assert_contains "$verbose" "Environment: $ENV"
assert_contains "$verbose" "JAVA_HOME: $JAVA_FIXTURE"
assert_contains "$verbose" "PUB_CACHE: $ENV/cache/pub"
assert_contains "$verbose" "Environment: ready"

rm -f -- "$ENV/state/environment.ready"
if incomplete="$(flenv_doctor --name isolated 2>&1)"; then fail "doctor accepted an incomplete environment"; fi
assert_contains "$incomplete" "Flutter Android toolchain"
assert_contains "$incomplete" "Environment has problems."

if missing="$(flenv_doctor --name missing 2>&1)"; then fail "doctor accepted a missing environment"; fi
assert_contains "$missing" "environment not found: missing"

printf 'PASS: CLI foundation\n'
