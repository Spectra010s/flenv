#!/usr/bin/env bash

FLENV_FLUTTER_RELEASES_URL="https://storage.googleapis.com/flutter_infra_release/releases/releases_linux.json"
FLENV_FLUTTER_STORAGE_URL="https://storage.googleapis.com/flutter_infra_release/releases"

flenv_flutter_arch() {
	case "$(uname -m)" in
	x86_64 | amd64) printf 'x64\n' ;;
	aarch64 | arm64) printf 'arm64\n' ;;
	*) flenv_die "unsupported Flutter Linux architecture: $(uname -m)" ;;
	esac
}

flenv_flutter_release_archive() {
	local metadata="$1"
	local arch="$2"
	flenv_require_command python3

	python3 - "$metadata" "$arch" <<'PY'
import json, sys
path, arch = sys.argv[1:]
with open(path, encoding="utf-8") as f:
    data = json.load(f)
stable_hash = data.get("current_release", {}).get("stable")
for release in data.get("releases", []):
    if release.get("hash") == stable_hash and release.get("channel") == "stable" and release.get("dart_sdk_arch") in (None, arch):
        print(release["archive"])
        break
else:
    raise SystemExit(f"no stable Linux Flutter release found for {arch}")
PY
}

flenv_provision_flutter() {
	local environment="$1"
	if flenv_component_ready "$environment" flutter; then
		flenv_success "Flutter already provisioned"
		return 0
	fi

	flenv_require_command tar
	local staging metadata arch archive bundle extract_dir version
	staging="$(flenv_staging_dir "$environment")/flutter"
	metadata="$staging/releases_linux.json"
	mkdir -p -- "$staging"

	flenv_step "Resolving stable release"
	flenv_download "$FLENV_FLUTTER_RELEASES_URL" "$metadata" "Flutter release metadata"
	arch="$(flenv_flutter_arch)"
	archive="$(flenv_flutter_release_archive "$metadata" "$arch")" || flenv_die "cannot resolve stable Flutter release"
	bundle="$staging/$(basename -- "$archive")"
	flenv_step "Downloading Flutter SDK"
	flenv_download "$FLENV_FLUTTER_STORAGE_URL/$archive" "$bundle" "Flutter SDK"

	flenv_step "Extracting"
	extract_dir="$staging/extracted"
	rm -rf -- "$extract_dir"
	mkdir -p -- "$extract_dir"
	tar --no-same-owner -xf "$bundle" -C "$extract_dir" || flenv_die "Flutter SDK extraction failed"
	[[ -x "$extract_dir/flutter/bin/flutter" ]] || flenv_die "Flutter archive did not contain a valid SDK"

	rm -rf -- "$environment/flutter"
	mv -- "$extract_dir/flutter" "$environment/flutter" || flenv_die "cannot install Flutter SDK"

	if ! version="$("$environment/flutter/bin/flutter" --version 2>/dev/null | head -n1)"; then
		rm -f -- "$environment/state/flutter.ready"
		flenv_die "Flutter validation failed"
	fi

	flenv_mark_component_ready "$environment" flutter
	flenv_success "${version:-Flutter ready}"
}
