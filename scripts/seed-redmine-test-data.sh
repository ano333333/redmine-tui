#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

docker compose -f compose.redmine.yml exec -T redmine sh -lc 'export SECRET_KEY_BASE="${SECRET_KEY_BASE:-${REDMINE_SECRET_KEY_BASE:-redmine-tui-local-test-secret}}"; bundle exec rails runner -' < docker/redmine/seed_test_data.rb
