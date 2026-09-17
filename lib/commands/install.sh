#!/usr/bin/env bash

flenv_install_help() {
	cat <<'EOF'
Usage: flenv install [--root DIR] [--name NAME] [--isolated] [--skip-licenses]

Creates a flenv environment and provisions Flutter plus the Android command-line SDK.
Android license acceptance is interactive unless --skip-licenses is supplied.
EOF
}

flenv_install() {
	local root=""
	local name="default"
	local isolated="false"
	local skip_licenses="false"

	while (($#)); do
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
		--skip-licenses)
			skip_licenses="true"
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

	flenv_provision_flutter "$environment"
	flenv_provision_android "$environment"

	if [[ "$skip_licenses" == "false" ]]; then
		flenv_android_licenses "$environment"
	else
		flenv_info "Android license review skipped; run provisioning again without --skip-licenses before building"
	fi

	printf 'Environment %s is provisioned.\n' "$name"
	printf 'Path: %s\n' "$environment"
	printf 'Isolated: %s\n' "$isolated"
	[[ "$isolated" == "true" ]] && printf 'Workspace: %s/workspace\n' "$environment"
}
