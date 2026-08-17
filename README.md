# redmine-tui

A draft terminal UI for browsing and editing Redmine issue data.

The current application is a local Ratatui prototype. It reads fixture-like YAML data from `datas/` and renders issue detail screens while the UI and domain model are being developed.

## Requirements

- Rust toolchain compatible with edition 2024
- Docker and Docker Compose v2, for the local Redmine test server
- Nix, optional, for the provided development shell

## Development Shell

If you use Nix:

```sh
nix develop
```

This shell provides Rust, Cargo, Clippy, `cargo-insta`, and LLVM coverage tools.

## Run the TUI

```sh
cargo run
```

Key bindings shown in the application:

- `Left` / `Right`: shrink or expand the rendered width
- `Up` / `Down`: shrink or expand the rendered height
- `q`: quit

## Test

```sh
cargo test
```

Snapshot tests use `cargo-insta`:

```sh
cargo insta test
```

Seeder file checks:

```sh
bash tests/redmine_seeder_files_test.sh
```

## Local Redmine With Docker

This repository includes a Docker Compose setup for a local Redmine instance used during development and testing.

Start Redmine:

```sh
docker compose -f compose.redmine.yml up -d
```

Open:

```text
http://localhost:8080
```

Default Redmine login:

```text
admin / admin
```

Stop Redmine:

```sh
docker compose -f compose.redmine.yml down
```

Reset all Redmine data, including the MySQL database and uploaded files:

```sh
docker compose -f compose.redmine.yml down -v
```

## Redmine Configuration

The Compose file is intended for local testing only. It uses Docker named volumes for MySQL data and Redmine uploaded files.

Common environment variables:

- `REDMINE_IMAGE`, default `redmine:6.1`
- `REDMINE_DB_IMAGE`, default `mysql:8.0`
- `REDMINE_PORT`, default `8080`
- `REDMINE_DB_DATABASE`, default `redmine`
- `REDMINE_DB_USERNAME`, default `redmine`
- `REDMINE_DB_PASSWORD`, default `redmine`
- `REDMINE_DB_ROOT_PASSWORD`, default `redmine-root`
- `REDMINE_SECRET_KEY_BASE`, default `redmine-tui-local-test-secret`

Example using a different host port:

```sh
REDMINE_PORT=18080 docker compose -f compose.redmine.yml up -d
```

## Seed Redmine Test Data

Start Redmine first, then run:

```sh
cargo xtask seed-redmine
```

This command generates SQL from `datas/` and pipes it into the MySQL service in `compose.redmine.yml`. It first applies `docker/redmine/fresh_test_data.sql`, so the local test data is reset before seeding.

To inspect the generated SQL without touching the database:

```sh
cargo xtask seed-redmine --dry-run
```

The seeder currently inserts the fixture data present in `datas/`:

- projects from `datas/projects.yml`
- users from `datas/users.yml`
- trackers, statuses, priorities, versions, categories, and time entry activities
- issues from `datas/issues/*.yml`, preserving issue IDs
- journals and journal details from `datas/journals/*.yml`

The compatibility wrapper remains available:

```sh
scripts/seed-redmine-test-data.sh
```

## Redmine References

- Redmine install guide: https://www.redmine.org/projects/redmine/wiki/redmineinstall
- Docker official Redmine image: https://hub.docker.com/_/redmine

## Repository Layout

- `src/`: Rust TUI source
- `xtask/`: Cargo development tasks, including Redmine YAML seeding
- `datas/`: local YAML fixture data
- `compose.redmine.yml`: local Redmine Docker Compose setup
- `docker/redmine/fresh_test_data.sql`: Redmine test-data reset SQL used before YAML seeding
- `scripts/seed-redmine-test-data.sh`: seeder execution wrapper
- `docs/redmine-test.md`: detailed local Redmine notes
