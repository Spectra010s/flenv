#!/usr/bin/env bash

flenv_install_help() {
	cat <<'EOF'
Usage: flenv install [--root DIR] [--name NAME] [--isolated]

Creates a flenv environment and provisions Flutter plus the Android command-line SDK.
EOF
}

flenv_validate_environment() {
	local environment="$1"
	local flutter="$environment/flutter/bin/flutter"
	local sdk="$environment/android-sdk"
	local java_home flutter_home doctor_output

	# A failed re-validation must never leave a stale ready marker behind.
	rm -f -- "$environment/state/environment.ready"

	[[ -x "$flutter" ]] || flenv_die "Flutter is not installed in the environment"
	[[ -x "$sdk/platform-tools/adb" ]] || flenv_die "Android platform-tools are not installed in the environment"

	if [[ -f "$environment/state/java-home" ]]; then
		java_home="$(<"$environment/state/java-home")"
	fi
	if [[ -z "${java_home:-}" || ! -x "$java_home/bin/java" ]]; then
		java_home="$(flenv_detect_java)"
	fi

	# Flutter gets an environment-local HOME so configuration does not leak into the host home.
	flutter_home="$environment/state/flutter-home"
	mkdir -p -- "$flutter_home"

	HOME="$flutter_home" JAVA_HOME="$java_home" ANDROID_HOME="$sdk" ANDROID_SDK_ROOT="$sdk" \
		"$flutter" config --android-sdk "$sdk" >/dev/null || flenv_die "cannot configure Flutter Android SDK"
	HOME="$flutter_home" JAVA_HOME="$java_home" ANDROID_HOME="$sdk" ANDROID_SDK_ROOT="$sdk" \
		"$flutter" config --jdk-dir "$java_home" >/dev/null || flenv_die "cannot configure Flutter JDK"

	if ! doctor_output="$(HOME="$flutter_home" JAVA_HOME="$java_home" ANDROID_HOME="$sdk" ANDROID_SDK_ROOT="$sdk" \
		"$flutter" doctor -v 2>&1)"; then
		printf '%s\n' "$doctor_output" >&2
		flenv_die "Flutter doctor failed"
	fi

	if ! grep -Eq '^\[✓\] Android toolchain' <<<"$doctor_output"; then
		printf '%s\n' "$doctor_output" >&2
		flenv_die "Flutter Android toolchain validation failed"
	fi

	printf '%s\n' "$java_home" >"$environment/state/java-home"
	flenv_mark_component_ready "$environment" environment
}

flenv_install() {
	local root=""
	local name="default"
	local isolated="false"

	# Walk through command-line arguments until every install option has been consumed.\n	while (($#)); do
		case "$1" in
		--root)
			(($# >= 2)) || flenv_die "--root requires a directory"
			root="$2"
			shift 2
			;;
		--name)
			(($# >= 2)) || flenv_die "--name requires a value"
			name="$2"
			shift 2
			;;
		--isolated)
			isolated="true"
			shift
			;;
		-h | --help)
			flenv_install_help
			return 0
			;;
		*)
			flenv_die "unknown install option: $1"
			;;
		esac
	done

	flenv_require_linux
	flenv_validate_name "$name"
	flenv_prepare_home

	if [[ -n "$root" ]]; then
		mkdir -p -- "$root" || flenv_die "cannot create root: $root"
		[[ -d "$root" && -w "$root" ]] || flenv_die "root is not a writable directory: $root"
		root="$(cd -P -- "$root" && pwd)"
	fi

	local environment
	environment="$(flenv_environment_path "$name" "$root")"

	# Existing directories are resumed so interrupted provisioning can be retried safely.
	if [[ ! -e "$environment" ]]; then
		mkdir -p -- "$environment/flutter" "$environment/android-sdk" "$environment/state" ||
			flenv_die "cannot create environment: $environment"

		if [[ "$isolated" == "true" ]]; then
			mkdir -p -- \
				"$environment/cache/pub" \
				"$environment/cache/gradle" \
				"$environment/workspace" || flenv_die "cannot create isolated environment layout"
		fi
		flenv_write_record "$name" "$environment" "$isolated"
	else
		flenv_info "resuming existing environment: $environment"
	fi

	flenv_section 1 4 "Flutter"
	flenv_provision_flutter "$environment"

	flenv_section 2 4 "Java"
	flenv_step "Checking installed JDK"
	local java_home java_major
	java_home="$(flenv_detect_java)"
	java_major="$(flenv_java_major_version "$java_home/bin/java")"
	flenv_success "Java $java_major"

	flenv_section 3 4 "Android"
	flenv_provision_android "$environment"

	flenv_section 4 4 "Validation"
	flenv_step "Checking Flutter Android toolchain"
	flenv_validate_environment "$environment"
	flenv_success "Environment ready"

	printf '\nEnvironment %s is provisioned.\n' "$name"
	printf 'Path: %s\n' "$environment"
	printf 'Isolated: %s\n' "$isolated"
	if [[ "$isolated" == "true" ]]; then
		printf 'Workspace: %s/workspace\n' "$environment"
	fi
}
