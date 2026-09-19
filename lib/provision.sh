#!/usr/bin/env bash

flenv_require_command() {
	command -v "$1" >/dev/null 2>&1 || flenv_die "required command not found: $1"
}

flenv_staging_dir() {
	printf '%s/state/staging\n' "$1"
}

flenv_download() {
	local url="$1"
	local destination="$2"
	local component="${3:-download}"
	local part="${destination}.part"

	flenv_require_command curl
	mkdir -p -- "$(dirname -- "$destination")"

	# Download into a .part file so interrupted transfers can resume without replacing a valid artifact.
	if ! curl --fail --location --silent --show-error --retry 3 --retry-delay 2 --continue-at - --output "$part" "$url"; then
		flenv_die "$component download failed; partial file kept at $part for retry"
	fi

	mv -f -- "$part" "$destination" || flenv_die "cannot finalize $component download"
}

flenv_mark_component_ready() {
	local environment="$1"
	local component="$2"
	mkdir -p -- "$environment/state"
	# Readiness markers are written only after the component has passed its validation.
	: >"$environment/state/${component}.ready"
}

flenv_component_ready() {
	[[ -f "$1/state/$2.ready" ]]
}
