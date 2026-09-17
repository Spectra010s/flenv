#!/usr/bin/env bash

# Pinned for reproducible v0.1 installs. Update deliberately when Google publishes a new stable build.
FLENV_ANDROID_CLI_BUILD="15859902"
FLENV_ANDROID_CLI_SHA256="4e4c464f145a7512b57d088ac6c278c03c9eea610886b35a5e0804e74eedf583"
FLENV_ANDROID_API="36"
FLENV_ANDROID_BUILD_TOOLS="36.0.0"

flenv_provision_android() {
	local environment="$1"
	flenv_component_ready "$environment" android && {
		flenv_info "Android SDK is already provisioned"
		return 0
	}

	[[ "$(uname -m)" == "x86_64" || "$(uname -m)" == "amd64" ]] ||
		flenv_die "Android command-line tools v0.1 currently support Linux x86_64 only"

	flenv_require_command unzip
	flenv_require_command sha256sum
	flenv_require_command java

	local sdk staging archive unpacked cli_url sdkmanager
	sdk="$environment/android-sdk"
	staging="$(flenv_staging_dir "$environment")/android"
	archive="$staging/commandlinetools-linux-${FLENV_ANDROID_CLI_BUILD}_latest.zip"
	cli_url="https://dl.google.com/android/repository/$(basename -- "$archive")"
	mkdir -p -- "$staging"

	flenv_download "$cli_url" "$archive" "Android command-line tools"
	printf '%s  %s\n' "$FLENV_ANDROID_CLI_SHA256" "$archive" | sha256sum --check --status ||
		flenv_die "Android command-line tools checksum verification failed"

	unpacked="$staging/unpacked"
	rm -rf -- "$unpacked"
	mkdir -p -- "$unpacked"
	unzip -q "$archive" -d "$unpacked" || flenv_die "Android command-line tools extraction failed"
	[[ -x "$unpacked/cmdline-tools/bin/sdkmanager" ]] || flenv_die "Android command-line tools archive is invalid"

	rm -rf -- "$sdk/cmdline-tools/latest"
	mkdir -p -- "$sdk/cmdline-tools/latest"
	cp -a -- "$unpacked/cmdline-tools/." "$sdk/cmdline-tools/latest/" || flenv_die "cannot install Android command-line tools"
	sdkmanager="$sdk/cmdline-tools/latest/bin/sdkmanager"

	"$sdkmanager" --sdk_root="$sdk" \
		"platform-tools" \
		"platforms;android-${FLENV_ANDROID_API}" \
		"build-tools;${FLENV_ANDROID_BUILD_TOOLS}" || flenv_die "Android SDK package installation failed"

	[[ -x "$sdk/platform-tools/adb" ]] || flenv_die "Android platform-tools validation failed"
	"$sdk/platform-tools/adb" version || flenv_die "adb validation failed"
	"$sdkmanager" --sdk_root="$sdk" --list_installed >/dev/null || flenv_die "Android SDK validation failed"

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
