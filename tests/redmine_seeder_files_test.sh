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

assert_file scripts/seed-redmine-test-data.sh
assert_file docker/redmine/seed_test_data.rb

bash -n scripts/seed-redmine-test-data.sh

assert_contains scripts/seed-redmine-test-data.sh "docker compose -f compose.redmine.yml exec -T redmine"
assert_contains scripts/seed-redmine-test-data.sh "bundle exec rails runner -"

assert_contains docker/redmine/seed_test_data.rb "redmine-tui-sandbox"
assert_contains docker/redmine/seed_test_data.rb "find_or_initialize_by"
assert_contains docker/redmine/seed_test_data.rb "Issue.find_or_initialize_by"
assert_contains docker/redmine/seed_test_data.rb "TimeEntry.find_or_initialize_by"
assert_contains docker/redmine/seed_test_data.rb "User.current"
assert_contains docker/redmine/seed_test_data.rb "Setting.notified_events = []"
assert_contains docker/redmine/seed_test_data.rb "issue.reload"
