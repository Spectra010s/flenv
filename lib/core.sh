#!/usr/bin/env bash

: "${FLENV_HOME:=${HOME:?HOME must be set}/.flenv}"

flenv_die() {
  printf 'flenv: error: %s\n' "$*" >&2
  exit 1
}

flenv_info() {
  printf 'flenv: %s\n' "$*" >&2
}

flenv_require_linux() {
  [[ "$(uname -s)" == "Linux" ]] || flenv_die "v0.1 currently supports Linux only"
}

flenv_validate_name() {
  local name="$1"
  [[ -n "$name" ]] || flenv_die "environment name cannot be empty"
  [[ "$name" != "." && "$name" != ".." ]] || flenv_die "invalid environment name: $name"
  [[ "$name" =~ ^[A-Za-z0-9._-]+$ ]] || flenv_die "invalid environment name: $name"
}

flenv_environment_path() {
  local name="$1"
  local root="${2:-}"

  if [[ -n "$root" ]]; then
    printf '%s/flenv/environments/%s\n' "${root%/}" "$name"
  else
    printf '%s/environments/%s\n' "${FLENV_HOME%/}" "$name"
  fi
}

flenv_record_path() {
  printf '%s/records/%s\n' "${FLENV_HOME%/}" "$1"
}

flenv_prepare_home() {
  mkdir -p -- "$FLENV_HOME/records" "$FLENV_HOME/environments" || flenv_die "cannot create FLENV_HOME: $FLENV_HOME"
  [[ -w "$FLENV_HOME" ]] || flenv_die "FLENV_HOME is not writable: $FLENV_HOME"
}

flenv_write_record() {
  local name="$1"
  local path="$2"
  local isolated="$3"
  local record
  record="$(flenv_record_path "$name")"

  {
    printf 'path=%s\n' "$path"
    printf 'isolated=%s\n' "$isolated"
  } > "$record" || flenv_die "cannot write environment record: $record"
}

flenv_read_record_value() {
  local record="$1"
  local key="$2"
  local line

  while IFS= read -r line; do
    if [[ "$line" == "$key="* ]]; then
      printf '%s\n' "${line#*=}"
      return 0
    fi
  done < "$record"
  return 1
}
