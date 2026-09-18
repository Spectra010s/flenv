#!/usr/bin/env bash

flenv_doctor_version_line() {
	local command="$1"
	shift
	"$command" "$@" 2>/dev/null | head -n 1
}

flenv_doctor() {
	local verbose="false"
	local name="default"

	while (($#)); do
		case "$1" in
		-v | --verbose)
			verbose="true"
			shift
			;;
		--name)
			(($# >= 2)) || flenv_die "--name requires a value"
			name="$2"
			shift 2
			;;
		*) flenv_die "unknown doctor option: $1" ;;
		esac
	done

	flenv_validate_name "$name"

	local record path isolated
	record="$(flenv_record_path "$name")"
	[[ -f "$record" ]] || flenv_die "environment not found: $name"
	path="$(flenv_read_record_value "$record" path || true)"
	isolated="$(flenv_read_record_value "$record" isolated || true)"
	[[ -n "$path" && -d "$path" ]] || flenv_die "environment is missing: $name"

	local flutter="$path/flutter/bin/flutter"
	local dart="$path/flutter/bin/dart"
	local sdk="$path/android-sdk"
	local adb="$sdk/platform-tools/adb"
	local java_home=""
	[[ -f "$path/state/java-home" ]] && java_home="$(<"$path/state/java-home")"

	local failed=0
	local host arch java_version flutter_version dart_version adb_version
	host="$(uname -s)"
	arch="$(uname -m)"

	printf 'Host\n'
	if [[ "$host" == "Linux" ]]; then
		printf '  ✓ Linux %s\n' "$arch"
	else
		printf '  ✗ Unsupported host: %s %s\n' "$host" "$arch"
		failed=1
	fi
	printf '  ✓ Bash %s\n' "${BASH_VERSION%%(*}"

	if [[ -n "$java_home" && -x "$java_home/bin/java" ]]; then
		java_version="$(flenv_java_major_version "$java_home/bin/java")"
		printf '  ✓ Java %s\n' "$java_version"
	else
		printf '  ✗ Java\n'
		failed=1
	fi

	printf '\nEnvironment: %s\n' "$name"
	if [[ -x "$flutter" ]]; then
		flutter_version="$("$flutter" --version --machine 2>/dev/null | sed -n 's/.*"frameworkVersion":"\([^"]*\)".*/\1/p' | head -n 1)"
		[[ -n "$flutter_version" ]] || flutter_version="installed"
		printf '  ✓ Flutter %s\n' "$flutter_version"
	else
		printf '  ✗ Flutter\n'
		failed=1
	fi

	if [[ -x "$dart" ]]; then
		dart_version="$("$dart" --version 2>&1 | sed -n 's/^Dart SDK version: \([^ ]*\).*/\1/p')"
		printf '  ✓ Dart %s\n' "${dart_version:-installed}"
	else
		printf '  ✗ Dart\n'
		failed=1
	fi

	if [[ -d "$sdk" ]]; then
		printf '  ✓ Android SDK\n'
	else
		printf '  ✗ Android SDK\n'
		failed=1
	fi

	local platform="" build_tools=""
	if [[ -d "$sdk/platforms" ]]; then
		platform="$(find "$sdk/platforms" -mindepth 1 -maxdepth 1 -type d -printf '%f\n' 2>/dev/null | sort -V | tail -n 1)"
	fi
	if [[ -n "$platform" ]]; then
		printf '  ✓ Android API %s\n' "${platform#android-}"
	else
		printf '  ✗ Android API\n'
		failed=1
	fi

	if [[ -d "$sdk/build-tools" ]]; then
		build_tools="$(find "$sdk/build-tools" -mindepth 1 -maxdepth 1 -type d -printf '%f\n' 2>/dev/null | sort -V | tail -n 1)"
	fi
	if [[ -n "$build_tools" ]]; then
		printf '  ✓ Build Tools %s\n' "$build_tools"
	else
		printf '  ✗ Build Tools\n'
		failed=1
	fi

	if [[ -x "$adb" ]]; then
		adb_version="$("$adb" version 2>/dev/null | sed -n 's/^Version //p' | head -n 1)"
		printf '  ✓ ADB %s\n' "${adb_version:-installed}"
	else
		printf '  ✗ ADB\n'
		failed=1
	fi

	if [[ "$isolated" == "true" ]]; then
		if [[ -d "$path/workspace" ]]; then
			printf '  ✓ Workspace\n'
		else
			printf '  ✗ Workspace\n'
			failed=1
		fi
		if [[ -d "$path/cache/pub" && -d "$path/cache/gradle" ]]; then
			printf '  ✓ Isolated caches\n'
		else
			printf '  ✗ Isolated caches\n'
			failed=1
		fi
	fi

	if [[ -f "$path/state/environment.ready" ]]; then
		printf '  ✓ Flutter Android toolchain\n'
	else
		printf '  ✗ Flutter Android toolchain\n'
		failed=1
	fi

	if [[ "$verbose" == "true" ]]; then
		printf '\nPaths\n'
		printf '  FLENV_HOME: %s\n' "$FLENV_HOME"
		printf '  Environment: %s\n' "$path"
		printf '  FLUTTER_ROOT: %s/flutter\n' "$path"
		printf '  ANDROID_HOME: %s\n' "$sdk"
		printf '  JAVA_HOME: %s\n' "${java_home:-missing}"
		printf '  Workspace: %s/workspace\n' "$path"
		if [[ "$isolated" == "true" ]]; then
			printf '  PUB_CACHE: %s/cache/pub\n' "$path"
			printf '  GRADLE_USER_HOME: %s/cache/gradle\n' "$path"
		fi
		printf '\nState\n'
		printf '  Isolated: %s\n' "$isolated"
		printf '  Flutter: %s\n' "$([[ -f "$path/state/flutter.ready" ]] && printf ready || printf incomplete)"
		printf '  Android: %s\n' "$([[ -f "$path/state/android.ready" ]] && printf ready || printf incomplete)"
		printf '  Environment: %s\n' "$([[ -f "$path/state/environment.ready" ]] && printf ready || printf incomplete)"
	fi

	printf '\n'
	if ((failed)); then
		printf 'Environment has problems.\n'
		return 1
	fi
	printf 'Environment is ready.\n'
}
