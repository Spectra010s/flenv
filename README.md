# flenv

A lightweight Flutter environment manager for headless, minimal, constrained, and ephemeral Linux environments.

`flenv` is for cases where a normal Flutter/Android setup is awkward or undesirable: cloud shells, small home partitions, mounted disks, temporary filesystems, remote servers, CI-like machines, and development without Android Studio.

The key idea is simple: **the heavy Flutter/Android environment should live where you choose, not necessarily in `$HOME`.**

## Status

Early development. The repository is private while the first usable version is being built.

## Goals

- Install Flutter and the Android SDK without Android Studio.
- Let users choose where the environment is stored.
- Keep Flutter, Android SDK, Pub cache, Gradle cache, and related state scoped to an environment when isolation is requested.
- Support multiple named environments.
- Make activation predictable from any shell.
- Provide diagnostics for broken or incomplete environments.
- Work well on headless/minimal Linux systems.

## Proposed CLI

```sh
# Interactive/default installation
flenv install

# Put the heavy environment on a specific filesystem
flenv install --root /tmp
flenv install --root /mnt --name stable

# Explicitly isolate SDKs and caches inside the environment
flenv install --root /tmp --name stable --isolated

# Environment management
flenv list
flenv use stable
eval "$(flenv env stable)"
flenv doctor
```

`/tmp` is never the implicit default. Ephemeral storage must be an explicit choice.

## Environment model

flenv separates lightweight metadata from environment data. `FLENV_HOME` defaults to `~/.flenv`, while the heavy environment can live elsewhere.

By default:

```text
~/.flenv/
├── config
├── active
├── records/
└── environments/
    └── default/
```

`--root` selects a **storage base**, not an arbitrary final environment path. flenv creates its own namespace beneath external roots. For example:

```sh
flenv install --root /tmp --name stable --isolated
```

uses:

```text
/tmp/flenv/environments/stable/
├── flutter/
├── android-sdk/
├── cache/
│   ├── pub/
│   └── gradle/
└── state/
```

User projects are not placed inside or owned by flenv environments.

The full v0.1 storage, isolation, activation, ephemeral-root, and removal semantics are documented in [`docs/environment-model.md`](docs/environment-model.md).

## Why flenv?

Flutter's normal setup assumes a fairly conventional development machine. On a constrained cloud shell, `$HOME` may be tiny while `/tmp`, `/mnt`, or another mounted filesystem has tens of gigabytes available. Installing Flutter in one place while Android SDK components, Pub packages, Gradle caches, and NDK downloads quietly fill `$HOME` defeats the point of moving Flutter at all.

`flenv` aims to make the storage boundary explicit and reproducible.

## v0.1 direction

The first version will be written in Bash and focus on Linux. It should prove one thing well: starting from a minimal environment, flenv can create a working Flutter + Android command-line development environment in a user-selected location and make it easy to activate again.

Initial scope:

- `flenv install`
- named environments
- configurable environment root
- isolated cache/SDK paths
- shell activation
- `flenv list`
- `flenv doctor`
- safe removal
- automated shell tests where practical

## Non-goals for v0.1

- Replacing Flutter's own version/channel tooling.
- Installing Android Studio.
- Managing iOS/macOS development environments.
- Hiding whether storage is ephemeral or persistent.

## Development

Development is issue-driven. `main` describes the current usable state; implementation work happens on focused branches and lands through pull requests.
