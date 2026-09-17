#!/usr/bin/env bash

FLENV_ANDROID_CLI_URL="https://dl.google.com/android/cli/latest/linux_x86_64/android"
FLENV_ANDROID_API="36"
FLENV_ANDROID_BUILD_TOOLS="36.0.0"

flenv_android_cli() {
	printf '%s/state/android-home/bin/android\n' "$1"
}

flenv_provision_android() {
	local environment="$1"
	local sdk="$environment/android-sdk"
	local android
	android="$(flenv_android_cli "$environment")"

	if flenv_component_ready "$environment" android &&
		[[ -x "$android" && -x "$sdk/platform-tools/adb" && -x "$sdk/cmdline-tools/latest/bin/sdkmanager" ]]; then
		flenv_info "Android SDK is already provisioned"
		return 0
	fi

	[[ "$(uname -m)" == "x86_64" || "$(uname -m)" == "amd64" ]] ||
		flenv_die "Android CLI v0.1 currently supports Linux x86_64 only"

	flenv_require_command java

	local cli_home staging launcher
	cli_home="$environment/state/android-home"
	staging="$(flenv_staging_dir "$environment")/android"
	launcher="$staging/android"
	mkdir -p -- "$staging" "$cli_home/bin" "$sdk"

	flenv_download "$FLENV_ANDROID_CLI_URL" "$launcher" "Android CLI"
	install -m 0755 -- "$launcher" "$android" || flenv_die "cannot install Android CLI"

	HOME="$cli_home" "$android" --version || flenv_die "Android CLI validation failed"
	HOME="$cli_home" "$android" --sdk="$sdk" sdk install \
		"platform-tools" \
		"platforms/android-${FLENV_ANDROID_API}" \
		"build-tools/${FLENV_ANDROID_BUILD_TOOLS}" \
		"cmdline-tools/latest" || flenv_die "Android SDK package installation failed"

	[[ -x "$sdk/platform-tools/adb" ]] || flenv_die "Android platform-tools validation failed"
	[[ -x "$sdk/cmdline-tools/latest/bin/sdkmanager" ]] || flenv_die "Android command-line tools validation failed"
	"$sdk/platform-tools/adb" version || flenv_die "adb validation failed"
	HOME="$cli_home" "$android" --sdk="$sdk" sdk list --all >/dev/null || flenv_die "Android SDK validation failed"

	flenv_mark_component_ready "$environment" android
	flenv_info "Android SDK provisioning complete"
}

flenv_android_licenses() {
	local environment="$1"
	local sdk="$environment/android-sdk"
	local sdkmanager="$sdk/cmdline-tools/latest/bin/sdkmanager"
	[[ -x "$sdkmanager" ]] || flenv_die "Android command-line tools are not installed"
	flenv_info "Android licenses require your review and acceptance"
	"$sdkmanager" --sdk_root="$sdk" --licenses
}
