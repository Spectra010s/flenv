# Contributing to flenv

Thanks for contributing to flenv.

flenv is a Bash-first, Linux-first project. Contributor tooling such as ShellCheck and shfmt is used during development, but is not required by people using flenv.

## Getting started

Fork or clone the repository and create a branch from the latest `main` for your change.

Use a short, descriptive branch name that reflects the kind of work being done:

```text
feat/install-progress
fix/environment-activation
docs/contributor-workflow
```

Keep each branch focused on one issue or change.

## Making changes

Before starting, check the issue you are working on and keep the implementation within its scope.

Follow the existing Bash style and project structure. Avoid introducing runtime dependencies unless the change genuinely requires them.

When behavior changes, add or update tests that cover the change, including relevant failure cases.

## Testing

The shell test suite is run through:

```sh
bash test/cli.sh
```

Add tests for new behavior and bug fixes rather than relying only on existing coverage. Run the suite before opening or updating a pull request.

You can also check Bash syntax with:

```sh
bash -n bin/flenv
find lib shell test -type f -name '*.sh' -print0 | xargs -0 -n1 bash -n
```

## Formatting and linting

flenv uses shfmt and ShellCheck.

Format the repository with:

```sh
shfmt -w .
```

Run ShellCheck with:

```sh
shellcheck -e SC1091 bin/flenv lib/*.sh lib/commands/*.sh shell/*.sh test/*.sh
```

CI checks formatting, Bash syntax, ShellCheck, and the test suite on pushes to `main` and on pull requests.

## Commits

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

Keep commits focused and use a scope that describes the part of flenv being changed.

## Pull requests

Open pull requests against `main` and link the issue they address.

Keep pull request descriptions focused on the change itself, its behavior, and relevant verification.
