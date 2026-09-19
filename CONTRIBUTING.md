# Contributing to flenv

Thanks for contributing to flenv.

flenv is a Bash-first, Linux-first project. Development tooling such as ShellCheck and shfmt is required for contributors, but is not a runtime dependency for flenv users.

## Development requirements

You need:

- Bash
- ShellCheck
- shfmt 3.12.0

On Debian/Ubuntu, ShellCheck can be installed with:

```sh
sudo apt-get update
sudo apt-get install -y shellcheck
```

Install shfmt 3.12.0 using the appropriate release binary for your system from the shfmt releases.

## Run the checks

Before opening a pull request, run the same checks used by CI from the repository root.

### Bash syntax

```sh
bash -n bin/flenv
find lib shell test -type f -name '*.sh' -print0 | xargs -0 -n1 bash -n
```

### Formatting

Format changed shell files with:

```sh
shfmt -w bin/flenv lib shell test
```

Then verify that the repository is formatted:

```sh
shfmt -d bin/flenv lib shell test
```

### ShellCheck

```sh
shellcheck -e SC1091 bin/flenv lib/*.sh lib/commands/*.sh shell/*.sh test/*.sh
```

### Tests

```sh
bash test/cli.sh
```

## Workflow

1. Pick an existing issue or open a focused issue for the change.
2. Create a focused branch from `main`.
3. Keep the change scoped to that issue.
4. Run the checks above.
5. Open a pull request against `main` and link the issue it addresses.

Use conventional commit messages in this form:

```text
type(scope): message
```

Examples:

```text
feat(env): add environment activation
fix(install): preserve partial downloads
docs(contributing): document local checks
```

Keep pull request descriptions focused on the change itself, its behavior, and relevant verification.
