#!/usr/bin/env bash

flenv_env() {
	local name="${1:-}"
	[[ -n "$name" ]] || flenv_die "env requires an environment name"
	(($# == 1)) || flenv_die "env accepts one environment name"

	local path record isolated java_home
	path="$(flenv_resolve_environment "$name")" || return 1
	record="$(flenv_record_path "$name")"
	isolated="$(flenv_read_record_value "$record" isolated || printf 'false')"

	# Restore the user's cache/Java settings before applying the next environment.
	local variable saved
	for variable in JAVA_HOME PUB_CACHE GRADLE_USER_HOME; do
		saved="FLENV_SAVED_$variable"
		if [[ -z "${FLENV_ACTIVE_ROOT:-}" ]]; then
			# Bash -v tests whether a variable is set, even when its value is empty.\n		if [[ -v $variable ]]; then
				printf 'export %s=%q\n' "$saved" "${!variable}"
			else
				printf 'unset %s\n' "$saved"
			fi
		elif [[ -v $saved ]]; then
			printf 'export %s=%q\n' "$variable" "${!saved}"
		else
			printf 'unset %s\n' "$variable"
		fi
	done

	# Remove only paths added by the previous activation, keeping other edits.
	local remaining="${PATH-}:" entry
	local -a paths=("$path/flutter/bin" "$path/android-sdk/platform-tools" "$path/android-sdk/cmdline-tools/latest/bin")
	while [[ "$remaining" == *:* ]]; do
		entry="${remaining%%:*}"
		remaining="${remaining#*:}"
		if [[ -n "${FLENV_ACTIVE_ROOT:-}" ]]; then
			case "$entry" in
			"$FLENV_ACTIVE_ROOT/flutter/bin" | "$FLENV_ACTIVE_ROOT/android-sdk/platform-tools" | "$FLENV_ACTIVE_ROOT/android-sdk/cmdline-tools/latest/bin") continue ;;
			esac
		fi
		paths+=("$entry")
	done
	printf 'export FLENV_ACTIVE_ROOT=%q\n' "$path"

	printf 'export FLENV_ENV=%q\n' "$name"
	printf 'export FLUTTER_ROOT=%q\n' "$path/flutter"
	printf 'export ANDROID_HOME=%q\n' "$path/android-sdk"
	printf 'export ANDROID_SDK_ROOT=%q\n' "$path/android-sdk"

	if [[ -f "$path/state/java-home" ]]; then
		java_home="$(<"$path/state/java-home")"
		[[ -d "$java_home" ]] && printf 'export JAVA_HOME=%q\n' "$java_home"
	fi

	if [[ "$isolated" == "true" ]]; then
		printf 'export PUB_CACHE=%q\n' "$path/cache/pub"
		printf 'export GRADLE_USER_HOME=%q\n' "$path/cache/gradle"
	fi

	# Joining an array with ${array[*]} uses the first IFS character, producing a PATH string.\n	local IFS=:
	printf 'export PATH=%q\n' "${paths[*]}"
}

flenv_use() {
	local name="${1:-}"
	[[ -n "$name" ]] || flenv_die "use requires an environment name"
	(($# == 1)) || flenv_die "use accepts one environment name"

	flenv_prepare_home
	flenv_resolve_environment "$name" >/dev/null || return 1
	printf '%s\n' "$name" >"$(flenv_selected_path)" || flenv_die "cannot save selected environment"
	printf 'Selected environment: %s\n' "$name"
}
