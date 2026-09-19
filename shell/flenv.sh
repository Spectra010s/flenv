#!/usr/bin/env bash

# Source this file from your shell to make `flenv use` activate environments:
#   source /path/to/flenv/shell/flenv.sh

flenv() {
	local command="${1:-}"

	if [[ "$command" == "use" ]]; then
		shift
		local name="${1:-}"
		[[ -n "$name" ]] || {
			printf 'flenv: error: use requires an environment name\n' >&2
			return 1
		}
		(($# == 1)) || {
			printf 'flenv: error: use accepts one environment name\n' >&2
			return 1
		}

		# The CLI prints shell assignments; eval applies them to this shell so `use` can activate the environment.\n		local activation
		activation="$(command flenv env "$name")" || return
		command flenv use "$name" || return
		eval "$activation"
		return
	fi

	command flenv "$@"
}
