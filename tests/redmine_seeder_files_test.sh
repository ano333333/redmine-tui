#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

assert_file() {
  local path="$1"
  if [[ ! -f "$path" ]]; then
    echo "missing required file: $path" >&2
    exit 1
  fi
}

assert_contains() {
  local path="$1"
  local expected="$2"
  if ! grep -Fq "$expected" "$path"; then
    echo "expected $path to contain: $expected" >&2
    exit 1
  fi
}

assert_file .cargo/config.toml
assert_file xtask/Cargo.toml
assert_file xtask/src/main.rs
assert_file docker/redmine/fresh_test_data.sql

seed_sql="$(cargo xtask seed-redmine --dry-run)"

assert_contains .cargo/config.toml 'xtask = "run --package xtask --"'

assert_contains docker/redmine/fresh_test_data.sql "DELETE FROM issues;"
assert_contains docker/redmine/fresh_test_data.sql "ALTER TABLE issues AUTO_INCREMENT = 1;"

if [[ "$seed_sql" != *"INSERT INTO issues"* ]]; then
  echo "expected seed SQL to insert issues" >&2
  exit 1
fi

if [[ "$seed_sql" != *"INSERT INTO journals"* ]]; then
  echo "expected seed SQL to insert journals" >&2
  exit 1
fi

if [[ "$seed_sql" != *"UPDATE issues SET parent_id = 3"* ]]; then
  echo "expected seed SQL to set parent-child issue relations" >&2
  exit 1
fi
