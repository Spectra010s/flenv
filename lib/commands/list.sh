#!/usr/bin/env bash

flenv_list() {
	flenv_prepare_home

	local found="false"
	local record name path isolated status

	printf '%-20s %-10s %s\n' "NAME" "STATUS" "PATH"

	shopt -s nullglob
	for record in "$FLENV_HOME"/records/*; do
		found="true"
		name="${record##*/}"
		path="$(flenv_read_record_value "$record" path || true)"
		isolated="$(flenv_read_record_value "$record" isolated || printf 'false')"

		if [[ -z "$path" || ! -d "$path" ]]; then
			status="missing"
		elif flenv_component_ready "$path" environment; then
			status="ready"
		else
			status="incomplete"
		fi

		if [[ "$isolated" == "true" ]]; then
			name="$name (isolated)"
		fi

		printf '%-20s %-10s %s\n' "$name" "$status" "${path:--}"
	done
	shopt -u nullglob

	if [[ "$found" == "false" ]]; then
		printf 'No environments recorded.\n'
	fi
}
