#!/usr/bin/env bash

FLENV_JAVA_MIN_VERSION="17"

flenv_java_major_version() {
	local java="$1"
	local version
	version="$("$java" -version 2>&1 | sed -n '1{s/.*version "\([0-9][0-9]*\).*/\1/p;q;}')"
	[[ "$version" =~ ^[0-9]+$ ]] || flenv_die "cannot determine Java version from: $java"
	printf '%s\n' "$version"
}

flenv_java_home() {
	local java="$1"
	local resolved
	resolved="$(readlink -f -- "$java" 2>/dev/null || true)"
	[[ -n "$resolved" && -x "$resolved" ]] || resolved="$java"
	cd -P -- "$(dirname -- "$resolved")/.." && pwd
}

flenv_detect_java() {
	local java=""

	if [[ -n "${JAVA_HOME:-}" && -x "$JAVA_HOME/bin/java" ]]; then
		java="$JAVA_HOME/bin/java"
	elif command -v java >/dev/null 2>&1; then
		java="$(command -v java)"
	else
		flenv_die "Java ${FLENV_JAVA_MIN_VERSION} or newer is required; no Java installation was found"
	fi

	local version home
	version="$(flenv_java_major_version "$java")"
	((version >= FLENV_JAVA_MIN_VERSION)) ||
		flenv_die "Java ${FLENV_JAVA_MIN_VERSION} or newer is required; found Java $version at $java"

	home="$(flenv_java_home "$java")" || flenv_die "cannot determine Java home from: $java"
	[[ -x "$home/bin/java" ]] || flenv_die "invalid Java home: $home"
	printf '%s\n' "$home"
}
