#!/usr/bin/env bash

flenv_require_command() {
	command -v "$1" >/dev/null 2>&1 || flenv_die "required command not found: $1"
}

flenv_staging_dir() {
	printf '%s/state/staging\n' "$1"
}

# The install runs in a subshell, so its descriptors and traps cannot leak.
flenv_install_log() {
	local name="$1" directory="$FLENV_HOME/logs"
	mkdir -p -- "$directory" || flenv_die "cannot create log directory: $directory"
	FLENV_LOG="$(mktemp "$directory/install-${name}-$(date -u +%Y%m%dT%H%M%SZ)-XXXXXX.log")" ||
		flenv_die "cannot create installation log"
	FLENV_OUTPUT="$FLENV_LOG.output"
	(
		umask 077
		: >"$FLENV_OUTPUT"
	)
	exec 3>&1
	# Used by flenv_message in core.sh.
	# shellcheck disable=SC2034
	FLENV_UI_FD=3
	exec >>"$FLENV_LOG" 2>&1
	trap 'flenv_install_finish "$?"' EXIT
	trap 'exit 130' INT
	trap 'exit 143' TERM
	trap 'printf "Command failed (%s): %s\n" "$?" "$BASH_COMMAND" >&2' ERR
	printf 'Started: %s\nEnvironment: %s\n' "$(date -u +%FT%TZ)" "$name"
	flenv_message 'Install log: %s\n' "$FLENV_LOG"
}

flenv_install_finish() {
	local status="$1"
	trap - EXIT INT TERM ERR
	if [[ -n "${FLENV_JOB_PID:-}" ]]; then
		kill "$FLENV_JOB_PID" 2>/dev/null || true
		wait "$FLENV_JOB_PID" 2>/dev/null || true
	fi
	if [[ -f "$FLENV_OUTPUT" ]]; then
		if [[ "${FLENV_OUTPUT_LOGGED:-false}" == false ]]; then
			cat -- "$FLENV_OUTPUT" >>"$FLENV_LOG"
		fi
		rm -f -- "$FLENV_OUTPUT"
	fi
	if [[ "${FLENV_PROGRESS_LINE:-false}" == true ]]; then
		printf '\n' >&3
	fi
	printf '\nFinished: %s (exit %s)\n' "$(date -u +%FT%TZ)" "$status"
	if ((status)); then
		flenv_message 'Installation failed during %s: %s (exit %s).\nDetailed log: %s\n' \
			"${FLENV_STAGE:-Setup}" "${FLENV_STEP:-Preparing environment}" "$status" "$FLENV_LOG"
	fi
	exit "$status"
}

# A byte count is the current on-disk partial size, including resumed bytes.
# Operations without a byte source report elapsed time, never invented percent.
flenv_progress() {
	local label="$1" elapsed="$2" final="${3:-false}" detail
	detail="${elapsed}s elapsed"
	if [[ -n "${FLENV_PROGRESS_FILE:-}" ]]; then
		local bytes=0
		[[ ! -f "$FLENV_PROGRESS_FILE" ]] || bytes="$(stat -c %s -- "$FLENV_PROGRESS_FILE")"
		detail="$bytes bytes saved, $detail"
	fi
	if [[ -t 3 && "${TERM:-dumb}" != dumb ]]; then
		printf '\r\033[2K  → %s ... %s' "$label" "$detail" >&3
		FLENV_PROGRESS_LINE=true
		if [[ "$final" == true ]]; then
			printf '\n' >&3
			FLENV_PROGRESS_LINE=false
		fi
	elif { [[ "$final" == true ]] && { [[ -n "${FLENV_PROGRESS_FILE:-}" ]] || ((elapsed >= 10)); }; } ||
		((elapsed > 0 && elapsed % 10 == 0)); then
		printf '  → %s ... %s\n' "$label" "$detail" >&3
	fi
}

flenv_run() {
	local label="$1" started=$SECONDS status=0
	shift
	if [[ -t 3 && "${TERM:-dumb}" != dumb ]]; then
		FLENV_STEP="$label"
		printf '  → %s\n' "$label" >>"$FLENV_LOG"
	else
		flenv_step "$label"
	fi
	{
		printf '\nCommand:'
		printf ' %q' "$@"
		printf '\n'
	} >>"$FLENV_LOG"
	FLENV_OUTPUT_LOGGED=false
	"$@" >"$FLENV_OUTPUT" 2>&1 &
	FLENV_JOB_PID=$!
	while kill -0 "$FLENV_JOB_PID" 2>/dev/null; do
		flenv_progress "$label" "$((SECONDS - started))"
		sleep 1
	done
	wait "$FLENV_JOB_PID" || status=$?
	FLENV_JOB_PID=""
	flenv_progress "$label" "$((SECONDS - started))" true
	cat -- "$FLENV_OUTPUT" >>"$FLENV_LOG"
	FLENV_OUTPUT_LOGGED=true
	printf 'Exit: %s\n' "$status" >>"$FLENV_LOG"
	return "$status"
}

flenv_download() {
	local url="$1" destination="$2" component="${3:-download}"
	local part="${destination}.part"
	local FLENV_PROGRESS_FILE="$part"

	flenv_require_command curl
	mkdir -p -- "$(dirname -- "$destination")"
	if ! flenv_run "Downloading $component" curl --fail --location --silent --show-error \
		--retry 3 --retry-delay 2 --continue-at - --output "$part" \
		--write-out 'HTTP %{http_code}; downloaded %{size_download} bytes; %{speed_download} bytes/s; %{time_total}s\n' "$url"; then
		flenv_die "$component download failed; partial file kept at $part for retry"
	fi
	mv -f -- "$part" "$destination" || flenv_die "cannot finalize $component download"
}

flenv_mark_component_ready() {
	local environment="$1"
	local component="$2"
	mkdir -p -- "$environment/state"
	: >"$environment/state/${component}.ready"
}

flenv_component_ready() {
	[[ -f "$1/state/$2.ready" ]]
}
