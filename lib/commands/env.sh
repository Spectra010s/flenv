#!/usr/bin/env bash

flenv_env() {
	local name="${1:-}"
	[[ -n "$name" ]] || flenv_die "env requires an environment name"
	(($# == 1)) || flenv_die "env accepts one environment name"

	local path record isolated java_home
	path="$(flenv_resolve_environment "$name")"
	record="$(flenv_record_path "$name")"
	isolated="$(flenv_read_record_value "$record" isolated || printf 'false')"

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

	printf 'export PATH=%q:"$PATH"\n' "$path/flutter/bin:$path/android-sdk/platform-tools:$path/android-sdk/cmdline-tools/latest/bin"
}

flenv_use() {
	local name="${1:-}"
	[[ -n "$name" ]] || flenv_die "use requires an environment name"
	(($# == 1)) || flenv_die "use accepts one environment name"

	flenv_prepare_home
	flenv_resolve_environment "$name" >/dev/null
	printf '%s\n' "$name" >"$(flenv_selected_path)" || flenv_die "cannot save selected environment"
	printf 'Selected environment: %s\n' "$name"
	printf 'Activate it with: eval "$(flenv env %s)"\n' "$name"
}
