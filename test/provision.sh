#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd -P -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_ROOT="$(mktemp -d)"
trap 'rm -rf -- "$TEST_ROOT"' EXIT
export FLENV_HOME="$TEST_ROOT/metadata"
export FIXTURES="$TEST_ROOT/fixtures"
export JAVA_HOME="$FIXTURES/jdk"
export PATH="$FIXTURES/bin:$PATH"
mkdir -p "$FIXTURES/bin" "$JAVA_HOME/bin" "$FIXTURES/sdk/flutter/bin"

fail() {
	printf 'FAIL: %s\n' "$*" >&2
	exit 1
}
contains() { [[ "$1" == *"$2"* ]] || fail "expected: $2"; }
log_path() { sed -n 's/^Install log: //p' "$1"; }

cat >"$JAVA_HOME/bin/java" <<'JAVA'
#!/usr/bin/env bash
printf 'openjdk version "21.0.1"\n' >&2
JAVA
cat >"$FIXTURES/sdk/flutter/bin/flutter" <<'FLUTTER'
#!/usr/bin/env bash
case "$1" in
--version) printf 'Flutter fixture ready\n' ;;
config) printf 'Flutter config diagnostic: %s\n' "$*" ;;
doctor)
  printf 'Flutter doctor diagnostic\n'
  [[ "${FAIL_STAGE:-}" != validation ]] || exit 9
  printf '[✓] Android toolchain\n'
  ;;
esac
FLUTTER
cat >"$FIXTURES/android" <<'ANDROID'
#!/usr/bin/env bash
set -eu
printf 'Android native output\r\033[2Knative renderer\n'
[[ "$1" != --version ]] || exit 0
sdk="${1#--sdk=}"
if [[ "$3" == install ]]; then
  [[ "${FAIL_STAGE:-}" != android ]] || { printf 'package diagnostic\n' >&2; exit 7; }
  mkdir -p "$sdk/platform-tools" "$sdk/cmdline-tools/latest/bin"
  printf '#!/usr/bin/env bash\nprintf "adb diagnostic\\n"\n' >"$sdk/platform-tools/adb"
  cp "$sdk/platform-tools/adb" "$sdk/cmdline-tools/latest/bin/sdkmanager"
  chmod +x "$sdk/platform-tools/adb" "$sdk/cmdline-tools/latest/bin/sdkmanager"
fi
ANDROID
cat >"$FIXTURES/bin/curl" <<'CURL'
#!/usr/bin/env bash
set -eu
printf 'curl diagnostic: %s\n' "$*"
out=""; url=""; resume=false; retries=false
while (($#)); do
  case "$1" in
    --output) out="$2"; shift 2 ;;
    --continue-at) [[ "$2" != - ]] || resume=true; shift 2 ;;
    --retry) [[ "$2" != 3 ]] || retries=true; shift 2 ;;
    --retry-delay|--write-out) shift 2 ;;
    --*) shift ;;
    *) url="$1"; shift ;;
  esac
done
[[ "$resume" == true && "$retries" == true ]] || exit 99
case "$url" in
  */releases_linux.json)
    printf '{"current_release":{"stable":"fixture"},"releases":[{"hash":"fixture","channel":"stable","dart_sdk_arch":"x64","archive":"fixture.tar"},{"hash":"fixture","channel":"stable","dart_sdk_arch":"arm64","archive":"fixture.tar"}]}' >"$out"
    ;;
  */fixture.tar)
    if [[ "${FAIL_STAGE:-}" == download ]]; then
      printf 'partial' >"$out"; printf 'network diagnostic\n' >&2; exit 18
    fi
    if [[ -f "$out" ]]; then printf 'Resuming from %s bytes\n' "$(stat -c %s "$out")"; fi
    cp "$FIXTURES/flutter.tar" "$out"
    ;;
  */android) cp "$FIXTURES/android" "$out" ;;
  *) exit 98 ;;
esac
CURL
chmod +x "$JAVA_HOME/bin/java" "$FIXTURES/sdk/flutter/bin/flutter" "$FIXTURES/android" "$FIXTURES/bin/curl"
tar -cf "$FIXTURES/flutter.tar" -C "$FIXTURES/sdk" flutter

# Full provisioning uses only tiny local fixtures; command output stays in logs.
"$ROOT/bin/flenv" install --name success --root "$TEST_ROOT/external storage" >"$TEST_ROOT/success" 2>&1
log="$(log_path "$TEST_ROOT/success")"
[[ -f "$log" ]] || fail 'success log missing'
[[ "$(stat -c %a "$log")" == 600 ]] || fail 'log is not private'
output="$(<"$TEST_ROOT/success")"
details="$(<"$log")"
for expected in 'curl diagnostic' 'flutter/bin/flutter' 'Java detection:' 'openjdk version' 'Android native output' 'adb diagnostic' 'Flutter config diagnostic' 'Flutter doctor diagnostic' 'exit 0'; do
	contains "$details" "$expected"
done
contains "$output" 'bytes saved'
[[ "$output" != *'native renderer'* && "$output" != *'curl diagnostic'* && "$output" != *$'\r'* && "$output" != *$'\033'* ]] || fail 'raw output leaked into non-TTY UI'
[[ ! -e "$log.output" ]] || fail 'command capture was not cleaned up'
cp "$log" "$TEST_ROOT/original-log"

# A second run creates a new log and leaves the first untouched.
"$ROOT/bin/flenv" install --name success --root "$TEST_ROOT/external storage" >"$TEST_ROOT/repeat" 2>&1
[[ "$(log_path "$TEST_ROOT/repeat")" != "$log" ]] || fail 'reused run log'
cmp "$log" "$TEST_ROOT/original-log" || fail 'overwrote first log'
rm -rf "$TEST_ROOT/external storage"
[[ -f "$log" ]] || fail 'environment removal lost log'

for stage in download android validation; do
	if FAIL_STAGE="$stage" "$ROOT/bin/flenv" install --name "$stage" >"$TEST_ROOT/$stage" 2>&1; then fail "$stage failure succeeded"; fi
	log="$(log_path "$TEST_ROOT/$stage")"
	[[ -f "$log" && ! -e "$log.output" ]] || fail "$stage log not preserved"
	contains "$(<"$TEST_ROOT/$stage")" "Detailed log: $log"
	contains "$(<"$TEST_ROOT/$stage")" 'Installation failed during'
	case "$stage" in
	download)
		contains "$(<"$log")" 'network diagnostic'
		contains "$(<"$TEST_ROOT/$stage")" 'Flutter: Downloading Flutter SDK'
		;;
	android)
		contains "$(<"$log")" 'package diagnostic'
		contains "$(<"$TEST_ROOT/$stage")" 'Android: Installing SDK packages'
		;;
	validation)
		contains "$(<"$log")" 'Flutter doctor diagnostic'
		contains "$(<"$TEST_ROOT/$stage")" 'Validation: Checking Flutter Android toolchain'
		;;
	esac
	[[ ! -e "$FLENV_HOME/environments/$stage/state/environment.ready" ]] || fail 'failure marked ready'
done
part="$FLENV_HOME/environments/download/state/staging/flutter/fixture.tar.part"
[[ "$(<"$part")" == partial ]] || fail 'download failure lost resumable data'
"$ROOT/bin/flenv" install --name download >"$TEST_ROOT/resume" 2>&1
contains "$(cat "$(log_path "$TEST_ROOT/resume")")" 'Resuming from 7 bytes'
[[ ! -e "$part" ]] || fail 'successful download left partial file'

# Render progress directly at known elapsed times, without sleeping or a network.
(
	source "$ROOT/lib/core.sh"
	source "$ROOT/lib/provision.sh"
	printf '1234567' >"$TEST_ROOT/bytes"
	FLENV_PROGRESS_FILE="$TEST_ROOT/bytes"
	exec 3>"$TEST_ROOT/progress"
	flenv_progress 'Flutter SDK' 10
	unset FLENV_PROGRESS_FILE
	flenv_progress 'SDK packages' 20
)
output="$(<"$TEST_ROOT/progress")"
contains "$output" '7 bytes saved'
contains "$output" '20s elapsed'
[[ "$output" != *'%'* && "$output" != *$'\033'* && "$output" != *$'\r'* ]] || fail 'non-TTY progress is animated or invented'
printf 'PASS: provisioning logs and progress\n'
