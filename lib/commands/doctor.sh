#!/usr/bin/env bash

flenv_doctor_check_command() {
	local command="$1"
	if command -v "$command" >/dev/null 2>&1; then
		printf '[ok]   %s\n' "$command"
		return 0
	fi

	printf '[fail] %s not found\n' "$command"
	return 1
}

flenv_doctor() {
	local failed=0

	if [[ "$(uname -s)" == "Linux" ]]; then
		printf '[ok]   Linux host\n'
	else
		printf '[fail] unsupported host: %s\n' "$(uname -s)"
		failed=1
	fi

	if ((BASH_VERSINFO[0] >= 4)); then
		printf '[ok]   Bash %s\n' "$BASH_VERSION"
	else
		printf '[fail] Bash 4 or newer is required\n'
		failed=1
	fi

	local command
	for command in mkdir uname; do
		flenv_doctor_check_command "$command" || failed=1
	done

	if mkdir -p -- "$FLENV_HOME/records" "$FLENV_HOME/environments" 2>/dev/null && [[ -w "$FLENV_HOME" ]]; then
		printf '[ok]   FLENV_HOME writable: %s\n' "$FLENV_HOME"
	else
		printf '[fail] FLENV_HOME not writable: %s\n' "$FLENV_HOME"
		failed=1
	fi

	if ((failed)); then
		printf '\nflenv doctor found problems.\n'
		return 1
	fi

	printf '\nflenv doctor found no problems in the CLI foundation.\n'
}
