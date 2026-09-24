# flenv vision — Rust rewrite

`flenv` is a Flutter environment manager for developers who do not use
Android Studio. It works with any editor, on a laptop, on a VPS, in a
cloud shell, and in CI.

Status: the Bash v0.1 implementation is frozen. Active work moves to a
portable Rust CLI described here and tracked in issues #36–#45.

## What flenv promises

- Start Flutter development without installing Android Studio.
- Install Flutter and the Android command-line SDK into a location you
  choose (`--root`), with pinned versions and checksum verification.
- Keep Pub and Gradle caches isolated per environment when requested.
- Work the same way on Linux, Windows, and macOS.
- Provide a GitHub Action for CI setup.

## Build everywhere, run where acceleration exists

- **Everywhere (no KVM needed):** `flenv setup` installs the SDKs,
  accepts Android licenses, and `flenv doctor` verifies the toolchain.
  This covers cloud shells, Codespaces, CI runners, and VPS instances.
  Emulator execution is not promised here.
- **Where hardware acceleration exists:** `flenv emulator create/list/start`
  plus `flenv view` provide a working emulator. Full speed on a laptop or
  a VPS with KVM. Slow software-rendered preview only where KVM is absent.

`flenv doctor` reports a missing `/dev/kvm` (or equivalent on Windows/macOS)
as a warning — "emulator will be slow" — rather than failing silently.

## Viewing

`flenv view` resolves an environment, checks `adb devices`, and explains how
to see the running emulator or device.

- **Default backend (no Docker required):** uses the standard emulator with a
  software-rendering fallback where KVM is absent, and prints the viewer
  addresses plus an SSH forwarding template for remote access from a laptop
  browser, a VNC client, or a phone.
- **Optional container backend:** a Redroid/Docker-based Android target for
  hosts without KVM where Docker and `binderfs` are available, exposed as
  `flenv view --backend redroid`. The default backend ships first.

## Why Rust

A single static binary per OS simplifies distribution via package managers
and `cargo install`, avoids Windows shell shims, and makes the GitHub Action
(`uses: flenv/setup@v1`) a thin wrapper around the same CLI. See issues
#36–#45 for the implementation breakdown.
