# Environment model

This document defines the storage and activation semantics for flenv v0.1.

## Two kinds of storage

flenv separates **metadata** from **environment data**.

### Metadata home

`FLENV_HOME` defaults to:

```text
~/.flenv
```

It is intentionally lightweight and may contain configuration, the active environment name, and environment records. Setting `FLENV_HOME` allows this location to be changed.

### Environment data

An environment contains the heavy development toolchain and, when isolated, its caches and workspace. Environment data does not have to live under `FLENV_HOME`.

The default environment base is:

```text
$FLENV_HOME/environments
```

A user can choose another base with `--root`.

## Names and roots

Every environment has a name. The default name is `default`.

`--root` is a **base directory**, not the final environment directory. flenv owns a `flenv/environments` namespace below an external root so it does not scatter generic names such as `stable` directly into `/tmp` or `/mnt`.

For example:

```sh
flenv install --name stable
```

uses:

```text
~/.flenv/environments/stable
```

while:

```sh
flenv install --root /tmp --name stable
```

uses:

```text
/tmp/flenv/environments/stable
```

and:

```sh
flenv install --root /mnt/fast-disk --name work
```

uses:

```text
/mnt/fast-disk/flenv/environments/work
```

`--root` never defaults to `/tmp`. Using an ephemeral filesystem is an explicit user decision.

Environment names are identifiers, not paths. v0.1 names should be restricted to letters, numbers, `.`, `_`, and `-`, and must not contain `/`, `..` path traversal, or shell control characters.

## Environment layout

An isolated v0.1 environment uses this layout:

```text
<environment>/
├── flutter/
├── android-sdk/
├── cache/
│   ├── pub/
│   └── gradle/
├── workspace/
└── state/
```

`state/` is reserved for environment-local flenv state that belongs with the toolchain.

`workspace/` is the environment's development workspace. It exists so users who deliberately move an isolated Flutter environment onto a larger or temporary filesystem can keep project working trees there too instead of consuming a constrained home partition.

Multiple projects can live inside the single workspace:

```text
workspace/
├── app-one/
├── app-two/
└── package-one/
```

flenv owns and creates the `workspace/` directory as part of an isolated environment, but **the projects inside it are user data**. That distinction affects destructive operations: SDKs and caches are reproducible environment data, while workspace contents must be protected.

## Metadata records

For environments stored outside the default environment base, flenv keeps a small record under `FLENV_HOME` so a name can be resolved later.

Conceptually:

```text
~/.flenv/
├── config
├── active
└── records/
    └── stable
```

A record contains only flenv metadata such as the environment's canonical path and isolation mode. It must not contain SDK payloads or large caches.

If an external or ephemeral environment disappears, the record may remain. Commands must detect that the target no longer exists and report the environment as missing/stale rather than claiming it is installed.

## Isolation

`--isolated` means storage-heavy state controlled by the Flutter/Android command-line workflow is redirected into the selected environment wherever the upstream tools provide a supported mechanism. It also creates the environment-local `workspace/` directory.

Activation for an isolated environment sets at least:

```text
FLUTTER_ROOT=<environment>/flutter
ANDROID_HOME=<environment>/android-sdk
ANDROID_SDK_ROOT=<environment>/android-sdk
PUB_CACHE=<environment>/cache/pub
GRADLE_USER_HOME=<environment>/cache/gradle
```

and prepends the relevant Flutter and Android tool directories to `PATH`.

flenv should not invent unsupported environment variables merely to claim perfect isolation. If an upstream tool stores unavoidable state elsewhere, `flenv doctor` should make that limitation visible.

Without `--isolated`, Flutter and Android SDK payloads still live in the selected environment, but normal user-level caches may use their upstream defaults and flenv does not require projects to use an environment-local workspace. This mode is useful when users want multiple SDK locations without duplicating every cache.

## Activation semantics

A child process cannot permanently modify its parent shell. Therefore `flenv use <name>` must not pretend that executing a normal program can rewrite the current shell environment.

v0.1 should expose shell-safe activation output, for example:

```sh
eval "$(flenv env stable)"
```

`flenv env <name>` prints shell assignments/exports only. Human-readable status belongs on stderr so command substitution remains safe.

A later convenience integration may provide shell functions/hooks, but installing flenv must not silently edit `.bashrc`, `.zshrc`, or equivalent files.

`flenv use <name>` may persist the selected default name in `FLENV_HOME/active`; it does not by itself mutate the already-running shell. `flenv env` with no name can then resolve that selected default.

## Persistence and ephemeral roots

flenv treats a path according to what it is, not according to a guessed filesystem lifetime. `/tmp` receives no special persistence promise.

When a user explicitly installs under `/tmp` and that directory later disappears:

- `flenv list` can show the recorded environment as missing;
- `flenv env <name>` must fail instead of exporting broken paths;
- `flenv doctor` should explain that the recorded environment root no longer exists;
- reinstalling the same named environment may recreate it after explicit confirmation or according to non-interactive flags.

For isolated environments on ephemeral storage, the workspace is ephemeral too. flenv must make this clear because project files in that workspace disappear with the underlying filesystem.

## Removal safety

flenv may recursively remove only directories that it can prove are managed environment directories. Removal must validate both the metadata record and the expected flenv environment layout before deleting recursively.

`flenv remove` must never recursively delete the value supplied to `--root` itself. For example, removing an environment installed with `--root /mnt` may operate on `/mnt/flenv/environments/stable`; it must never delete `/mnt`.

For an isolated environment:

- an empty `workspace/` may be removed with the environment;
- a non-empty `workspace/` must block normal destructive removal;
- deleting a non-empty workspace requires a separate explicit destructive confirmation/force path;
- flenv should offer or support preserving the workspace while removing reproducible SDK/cache state.

This prevents a normal environment cleanup from being equivalent to deleting source repositories.

## v0.1 invariants

1. Heavy SDK data is never forced into `$HOME` when the user explicitly selected another root.
2. `--root` means a storage base; flenv creates its own namespace beneath it.
3. Environment names never act as arbitrary filesystem paths.
4. `/tmp` and other ephemeral storage are opt-in.
5. `--isolated` redirects Pub and Gradle caches as well as SDK payloads and creates an environment-local `workspace/`.
6. flenv owns the workspace directory structure, but projects inside it are protected user data.
7. Activation is explicit and shell-correct; flenv does not silently edit shell startup files.
8. Missing external environments are detected rather than hidden.
9. Normal environment removal must not delete a non-empty workspace.